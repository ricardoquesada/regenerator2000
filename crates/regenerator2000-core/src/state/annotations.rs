//! Sparse, address-keyed annotation storage and lexical scope indexing.
//!
//! This module defines [`AddressEntry`] and [`AnnotationManager`], which store per-address
//! comments, formatting overrides, bookmarks, enum usages, and lexical symbol scopes.
//! Empty or whitespace-only entries are automatically pruned from the underlying [`BTreeMap`]
//! to guarantee deterministic `.regen2000proj` serialization, while secondary `scopes` and
//! `scope_ends` indices provide `O(log S)` boundary lookups.

use crate::state::types::{Addr, ImmediateFormat};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Returns `true` if `s` is `None`, empty (`""`), or contains only ASCII/Unicode whitespace.
#[allow(clippy::unnecessary_map_or)]
fn is_str_none_or_empty(s: &Option<String>) -> bool {
    s.as_deref().map_or(true, |st| st.trim().is_empty())
}

/// Compact sparse annotation entry storing user annotations and formatting overrides for an [`Addr`].
///
/// Entries are managed via [`AnnotationManager::update`], which normalizes whitespace-only
/// strings to `None` and prunes completely empty records from the map.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressEntry {
    /// Auto-generated or platform system comment (loaded dynamically from system assets; not saved in `.regen2000proj` files).
    #[serde(default, skip_serializing)]
    pub system_comment: Option<String>,

    /// User-defined side (inline) comment rendered at the end of a disassembly line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_side_comment: Option<String>,

    /// User-defined full-line banner comment rendered above the instruction at this [`Addr`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_line_comment: Option<String>,

    /// Specific [`ImmediateFormat`] override for immediate operand values at this [`Addr`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immediate_format: Option<ImmediateFormat>,

    /// User bookmark title or description attached to this [`Addr`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,

    /// Inclusive end [`Addr`] of the lexical symbol scope (`start..=end`) whose `start` is this entry's key in [`AnnotationManager`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Addr>,

    /// Name of the enum definition applied to the operand at this [`Addr`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_usage: Option<String>,
}

impl AddressEntry {
    /// Normalizes empty or whitespace-only string fields to `None`.
    pub fn normalize(&mut self) {
        if self
            .system_comment
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        {
            self.system_comment = None;
        }
        if self
            .user_side_comment
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        {
            self.user_side_comment = None;
        }
        if self
            .user_line_comment
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        {
            self.user_line_comment = None;
        }
        if self
            .bookmark
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        {
            self.bookmark = None;
        }
        if self
            .enum_usage
            .as_deref()
            .is_some_and(|s| s.trim().is_empty())
        {
            self.enum_usage = None;
        }
    }

    /// Returns `true` if all metadata fields (including `system_comment`) are `None`, empty, or whitespace-only.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        is_str_none_or_empty(&self.system_comment)
            && is_str_none_or_empty(&self.user_side_comment)
            && is_str_none_or_empty(&self.user_line_comment)
            && self.immediate_format.is_none()
            && is_str_none_or_empty(&self.bookmark)
            && self.scope.is_none()
            && is_str_none_or_empty(&self.enum_usage)
    }

    /// Returns `true` if the entry contains user-defined annotations that must be persisted to `.regen2000proj` files.
    ///
    /// Unlike [`AddressEntry::is_empty`], this excludes `system_comment` because platform comments are reloaded dynamically from system assets.
    #[must_use]
    pub fn is_persistent(&self) -> bool {
        !is_str_none_or_empty(&self.user_side_comment)
            || !is_str_none_or_empty(&self.user_line_comment)
            || self.immediate_format.is_some()
            || !is_str_none_or_empty(&self.bookmark)
            || self.scope.is_some()
            || !is_str_none_or_empty(&self.enum_usage)
    }
}

/// Encapsulated manager guaranteeing deterministic [`BTreeMap`] node pruning on empty entries
/// and maintaining secondary `O(log S)` indices for lexical scopes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationManager {
    /// Sparse primary storage of non-empty [`AddressEntry`] records keyed by [`Addr`].
    map: BTreeMap<Addr, AddressEntry>,
    /// Secondary index mapping each lexical scope's inclusive `start` [`Addr`] to its inclusive `end` [`Addr`].
    scopes: BTreeMap<Addr, Addr>,
    /// Secondary index of exclusive scope end addresses (`end.wrapping_add(1)`), used for `O(log S)` virtual-splitter boundary checks.
    scope_ends: BTreeSet<Addr>,
}

impl serde::Serialize for AnnotationManager {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let persistent_entries: Vec<(&Addr, &AddressEntry)> = self
            .map
            .iter()
            .filter(|(_, entry)| entry.is_persistent())
            .collect();

        let mut map = serializer.serialize_map(Some(persistent_entries.len()))?;
        for (addr, entry) in persistent_entries {
            map.serialize_entry(addr, entry)?;
        }
        map.end()
    }
}

impl AnnotationManager {
    /// Fetches an immutable reference to the [`AddressEntry`] at `addr`, if present.
    #[must_use]
    pub fn get(&self, addr: Addr) -> Option<&AddressEntry> {
        self.map.get(&addr)
    }

    /// Mutates an [`AddressEntry`] at `addr` in-place, normalizes string fields, automatically
    /// prunes empty [`BTreeMap`] nodes, and maintains the secondary lexical scope indices.
    pub fn update<F>(&mut self, addr: Addr, f: F)
    where
        F: FnOnce(&mut AddressEntry),
    {
        use std::collections::btree_map::Entry;
        let (old_scope, new_scope) = match self.map.entry(addr) {
            Entry::Occupied(mut entry) => {
                let old_scope = entry.get().scope;
                f(entry.get_mut());
                entry.get_mut().normalize();
                let new_scope = entry.get().scope;
                if entry.get().is_empty() {
                    entry.remove();
                }
                (old_scope, new_scope)
            }
            Entry::Vacant(entry) => {
                let mut new_entry = AddressEntry::default();
                f(&mut new_entry);
                new_entry.normalize();
                let new_scope = new_entry.scope;
                if !new_entry.is_empty() {
                    entry.insert(new_entry);
                }
                (None, new_scope)
            }
        };

        if old_scope != new_scope {
            if let Some(end) = new_scope {
                self.scopes.insert(addr, end);
            } else {
                self.scopes.remove(&addr);
            }
            // Rebuild `scope_ends` from `self.scopes.values()` rather than calling
            // `self.scope_ends.remove(&old_end.wrapping_add(1))`: multiple scopes can
            // share the same inclusive end address, so removing one scope must not
            // delete the shared exclusive boundary while another scope still ends there.
            self.scope_ends = self.scopes.values().map(|e| e.wrapping_add(1)).collect();
        }
    }

    /// Rebuilds the secondary `scopes` and `scope_ends` indices from `self.map`.
    fn rebuild_scope_indices(&mut self) {
        self.scopes.clear();
        self.scope_ends.clear();
        for (&addr, entry) in &self.map {
            if let Some(end) = entry.scope {
                self.scopes.insert(addr, end);
                self.scope_ends.insert(end.wrapping_add(1));
            }
        }
    }

    /// Returns an iterator over all non-empty `(Addr, &AddressEntry)` pairs in ascending address order.
    pub fn iter(&self) -> impl Iterator<Item = (Addr, &AddressEntry)> {
        self.map.iter().map(|(&a, e)| (a, e))
    }

    /// Clears all annotations and resets the secondary `scopes` and `scope_ends` indices.
    pub fn clear(&mut self) {
        self.map.clear();
        self.scopes.clear();
        self.scope_ends.clear();
    }

    /// Returns `true` if there are no annotations stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the number of annotated addresses.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns an immutable reference to the indexed [`BTreeMap`] of active lexical scopes,
    /// mapping each scope's inclusive `start` [`Addr`] to its inclusive `end` [`Addr`].
    #[must_use]
    pub fn scopes(&self) -> &BTreeMap<Addr, Addr> {
        &self.scopes
    }

    /// Returns an immutable reference to the [`BTreeSet`] of exclusive scope end addresses (`end.wrapping_add(1)`).
    ///
    /// Note that a scope ending at `$FFFF` inserts `$0000` into this set via 16-bit wrapping addition.
    /// Prefer this zero-allocation accessor over [`AnnotationManager::scope_ends`] on hot paths.
    #[must_use]
    pub fn scope_ends_set(&self) -> &BTreeSet<Addr> {
        &self.scope_ends
    }

    /// Returns a cloned [`BTreeSet`] of exclusive scope end addresses (`end.wrapping_add(1)`).
    ///
    /// Prefer [`AnnotationManager::scope_ends_set`] when a borrowed reference suffices.
    #[must_use]
    pub fn scope_ends(&self) -> BTreeSet<Addr> {
        self.scope_ends.clone()
    }

    /// Returns `true` if a lexical scope starts at `addr` (`O(log S)`).
    #[must_use]
    pub fn is_scope_start(&self, addr: Addr) -> bool {
        self.scopes.contains_key(&addr)
    }

    /// Returns `true` if `addr` immediately follows the inclusive end of any lexical scope (`end.wrapping_add(1) == addr`, `O(log S)`).
    #[must_use]
    pub fn is_scope_end(&self, addr: Addr) -> bool {
        self.scope_ends.contains(&addr)
    }
}

impl<'de> serde::Deserialize<'de> for AnnotationManager {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Supports three JSON representations for backward compatibility:
        // 1. Flattened/direct map of `Addr -> AddressEntry` (`AnnotationsPayload::Direct`).
        // 2. Structured object with an `annotations` (or `map`) field (`AnnotationsPayload::Structured`).
        // 3. Legacy per-category maps (`user_side_comments`, `scopes`, etc.) inside `RawAnnotations`.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum AnnotationsPayload {
            Direct(BTreeMap<Addr, AddressEntry>),
            Structured(Box<RawAnnotations>),
        }

        #[derive(Deserialize)]
        struct RawAnnotations {
            #[serde(alias = "map")]
            annotations: Option<BTreeMap<Addr, AddressEntry>>,
            system_comments: Option<BTreeMap<Addr, String>>,
            user_side_comments: Option<BTreeMap<Addr, String>>,
            user_line_comments: Option<BTreeMap<Addr, String>>,
            immediate_value_formats: Option<BTreeMap<Addr, ImmediateFormat>>,
            bookmarks: Option<BTreeMap<Addr, String>>,
            scopes: Option<BTreeMap<Addr, Addr>>,
            enum_usages: Option<BTreeMap<Addr, String>>,
        }

        let payload = AnnotationsPayload::deserialize(deserializer)?;
        let mut manager = AnnotationManager::default();

        match payload {
            AnnotationsPayload::Direct(map) => {
                for (addr, mut entry) in map {
                    entry.normalize();
                    if !entry.is_empty() {
                        manager.map.insert(addr, entry);
                    }
                }
            }
            AnnotationsPayload::Structured(raw) => {
                if let Some(ann) = raw.annotations {
                    for (addr, mut entry) in ann {
                        entry.normalize();
                        if !entry.is_empty() {
                            manager.map.insert(addr, entry);
                        }
                    }
                }
                if let Some(sys) = raw.system_comments {
                    for (addr, comment) in sys {
                        manager.update(addr, |e| e.system_comment = Some(comment));
                    }
                }
                if let Some(side) = raw.user_side_comments {
                    for (addr, comment) in side {
                        manager.update(addr, |e| e.user_side_comment = Some(comment));
                    }
                }
                if let Some(line) = raw.user_line_comments {
                    for (addr, comment) in line {
                        manager.update(addr, |e| e.user_line_comment = Some(comment));
                    }
                }
                if let Some(imm) = raw.immediate_value_formats {
                    for (addr, fmt) in imm {
                        manager.update(addr, |e| e.immediate_format = Some(fmt));
                    }
                }
                if let Some(bm) = raw.bookmarks {
                    for (addr, mark) in bm {
                        manager.update(addr, |e| e.bookmark = Some(mark));
                    }
                }
                if let Some(sc) = raw.scopes {
                    for (addr, scope) in sc {
                        manager.update(addr, |e| e.scope = Some(scope));
                    }
                }
                if let Some(enums) = raw.enum_usages {
                    for (addr, en) in enums {
                        manager.update(addr, |e| e.enum_usage = Some(en));
                    }
                }
            }
        }

        // Rebuild secondary scope indices unconditionally because `Direct` and `raw.annotations`
        // insert directly into `manager.map` without calling `manager.update()`.
        manager.rebuild_scope_indices();
        Ok(manager)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_entry_normalize() {
        let mut entry = AddressEntry {
            system_comment: Some("".to_string()),
            user_side_comment: Some("comment".to_string()),
            user_line_comment: Some("".to_string()),
            bookmark: Some("".to_string()),
            enum_usage: Some("".to_string()),
            ..Default::default()
        };

        entry.normalize();

        assert_eq!(entry.system_comment, None);
        assert_eq!(entry.user_side_comment, Some("comment".to_string()));
        assert_eq!(entry.user_line_comment, None);
        assert_eq!(entry.bookmark, None);
        assert_eq!(entry.enum_usage, None);
        assert!(!entry.is_empty());
    }

    #[test]
    fn test_address_entry_is_empty() {
        let mut entry = AddressEntry {
            system_comment: Some("".to_string()),
            ..Default::default()
        };
        assert!(entry.is_empty());

        entry.normalize();
        assert!(entry.is_empty());

        entry.user_side_comment = Some("hello".to_string());
        assert!(!entry.is_empty());
    }

    #[test]
    fn test_annotation_manager_update_and_pruning() {
        let mut manager = AnnotationManager::default();
        let addr = Addr(0x1000);

        // Update with non-empty comment
        manager.update(addr, |e| {
            e.user_side_comment = Some("Side comment".to_string());
        });

        assert_eq!(manager.len(), 1);
        assert_eq!(
            manager
                .get(addr)
                .and_then(|e| e.user_side_comment.as_deref()),
            Some("Side comment")
        );

        // Update to empty string -> should normalize and prune node
        manager.update(addr, |e| {
            e.user_side_comment = Some("".to_string());
        });

        assert_eq!(manager.len(), 0);
        assert!(manager.get(addr).is_none());
    }

    #[test]
    fn test_legacy_json_deserialization() {
        let legacy_json = r#"{
            "user_side_comments": { "4096": "Legacy side comment" },
            "user_line_comments": { "4096": "Legacy line comment" },
            "bookmarks": { "8192": "Start of main" }
        }"#;

        let manager: AnnotationManager = serde_json::from_str(legacy_json).unwrap();

        assert_eq!(manager.len(), 2);
        let entry_4096 = manager.get(Addr(4096)).unwrap();
        assert_eq!(
            entry_4096.user_side_comment.as_deref(),
            Some("Legacy side comment")
        );
        assert_eq!(
            entry_4096.user_line_comment.as_deref(),
            Some("Legacy line comment")
        );

        let entry_8192 = manager.get(Addr(8192)).unwrap();
        assert_eq!(entry_8192.bookmark.as_deref(), Some("Start of main"));
    }

    #[test]
    fn test_modern_json_deserialization_with_alias() {
        let modern_json = r#"{
            "map": {
                "4096": {
                    "user_side_comment": "Modern comment"
                }
            }
        }"#;

        let manager: AnnotationManager = serde_json::from_str(modern_json).unwrap();

        assert_eq!(manager.len(), 1);
        let entry = manager.get(Addr(4096)).unwrap();
        assert_eq!(entry.user_side_comment.as_deref(), Some("Modern comment"));
    }

    #[test]
    fn test_whitespace_normalization() {
        let mut entry = AddressEntry {
            user_side_comment: Some("   ".to_string()),
            user_line_comment: Some("\t \n".to_string()),
            bookmark: Some("  valid  ".to_string()),
            ..Default::default()
        };

        entry.normalize();

        assert_eq!(entry.user_side_comment, None);
        assert_eq!(entry.user_line_comment, None);
        assert_eq!(entry.bookmark, Some("  valid  ".to_string()));
    }

    // T1.1
    #[test]
    fn test_annotation_manager_scope_index_lifecycle() {
        let mut manager = AnnotationManager::default();
        let start = Addr(0x1000);

        // 1. Insert scope $1000 -> $1020
        manager.update(start, |e| {
            e.scope = Some(Addr(0x1020));
        });
        assert!(manager.is_scope_start(start));
        assert!(!manager.is_scope_start(Addr(0x1001)));
        assert!(manager.is_scope_end(Addr(0x1021)));
        assert!(!manager.is_scope_end(Addr(0x1020)));
        assert_eq!(manager.scopes().get(&start), Some(&Addr(0x1020)));
        assert!(manager.scope_ends().contains(&Addr(0x1021)));
        assert!(manager.scope_ends_set().contains(&Addr(0x1021)));

        // 2. Mutate scope end to $1030
        manager.update(start, |e| {
            e.scope = Some(Addr(0x1030));
        });
        assert!(manager.is_scope_start(start));
        assert!(!manager.is_scope_end(Addr(0x1021)));
        assert!(manager.is_scope_end(Addr(0x1031)));
        assert_eq!(manager.scopes().get(&start), Some(&Addr(0x1030)));

        // 3. Remove scope
        manager.update(start, |e| {
            e.scope = None;
        });
        assert!(!manager.is_scope_start(start));
        assert!(!manager.is_scope_end(Addr(0x1031)));
        assert!(manager.scopes().is_empty());
        assert!(manager.scope_ends_set().is_empty());
        assert!(manager.is_empty());
    }

    // T1.2
    #[test]
    fn test_annotation_manager_shared_scope_end_landmine_a() {
        let mut manager = AnnotationManager::default();
        let s1 = Addr(0x1000);
        let s2 = Addr(0x1010);
        let shared_end = Addr(0x1050);
        let exclusive_end = Addr(0x1051);

        manager.update(s1, |e| e.scope = Some(shared_end));
        manager.update(s2, |e| e.scope = Some(shared_end));

        assert_eq!(manager.scopes().len(), 2);
        assert!(manager.is_scope_end(exclusive_end));

        // Remove first scope: $1051 must remain in scope_ends because s2 still ends at $1050
        manager.update(s1, |e| e.scope = None);
        assert!(!manager.is_scope_start(s1));
        assert!(manager.is_scope_start(s2));
        assert!(
            manager.is_scope_end(exclusive_end),
            "Shared scope end must remain active while second scope still references it"
        );

        // Remove second scope: $1051 must now be removed
        manager.update(s2, |e| e.scope = None);
        assert!(!manager.is_scope_start(s2));
        assert!(!manager.is_scope_end(exclusive_end));
        assert!(manager.scopes().is_empty());
        assert!(manager.scope_ends_set().is_empty());
    }

    // T1.3
    #[test]
    fn test_annotation_manager_scope_end_wrapping_ffff() {
        let mut manager = AnnotationManager::default();
        manager.update(Addr(0xFF00), |e| e.scope = Some(Addr(0xFFFF)));

        assert!(manager.is_scope_start(Addr(0xFF00)));
        assert!(manager.is_scope_end(Addr(0x0000)));
        assert!(manager.scope_ends_set().contains(&Addr(0x0000)));
    }

    // T1.4
    #[test]
    fn test_annotation_manager_clear_resets_scope_indices() {
        let mut manager = AnnotationManager::default();
        manager.update(Addr(0x1000), |e| {
            e.user_side_comment = Some("note".to_string());
            e.scope = Some(Addr(0x1020));
        });
        assert!(!manager.is_empty());
        assert!(!manager.scopes().is_empty());
        assert!(!manager.scope_ends_set().is_empty());

        manager.clear();

        assert!(manager.is_empty());
        assert!(manager.scopes().is_empty());
        assert!(manager.scope_ends_set().is_empty());
    }

    // T1.5
    #[test]
    fn test_annotation_manager_deserialize_populates_scopes_landmine_b() {
        // 1. Direct payload
        let direct_json = r#"{
            "8192": {
                "user_side_comment": "scope start",
                "scope": 8208
            }
        }"#;
        let direct_mgr: AnnotationManager = serde_json::from_str(direct_json).unwrap();
        assert!(direct_mgr.is_scope_start(Addr(0x2000)));
        assert!(direct_mgr.is_scope_end(Addr(0x2011)));
        assert_eq!(direct_mgr.scopes().get(&Addr(0x2000)), Some(&Addr(0x2010)));

        // 2. Structured payload with "map" field
        let structured_map_json = r#"{
            "map": {
                "8192": {
                    "user_side_comment": "scope start",
                    "scope": 8208
                }
            }
        }"#;
        let structured_mgr: AnnotationManager = serde_json::from_str(structured_map_json).unwrap();
        assert!(structured_mgr.is_scope_start(Addr(0x2000)));
        assert!(structured_mgr.is_scope_end(Addr(0x2011)));
        assert_eq!(direct_mgr, structured_mgr);

        // 3. Structured payload with legacy "scopes" field
        let legacy_scopes_json = r#"{
            "scopes": {
                "4096": 4112
            }
        }"#;
        let legacy_mgr: AnnotationManager = serde_json::from_str(legacy_scopes_json).unwrap();
        assert!(legacy_mgr.is_scope_start(Addr(4096)));
        assert!(legacy_mgr.is_scope_end(Addr(4113)));
        assert_eq!(legacy_mgr.scopes().get(&Addr(4096)), Some(&Addr(4112)));

        // 4. Round-trip serialize -> deserialize equality
        let mut expected = AnnotationManager::default();
        expected.update(Addr(0x2000), |e| {
            e.user_side_comment = Some("scope start".to_string());
            e.scope = Some(Addr(0x2010));
        });
        let serialized = serde_json::to_string(&expected).unwrap();
        let round_tripped: AnnotationManager = serde_json::from_str(&serialized).unwrap();
        assert_eq!(expected, round_tripped);
        assert!(round_tripped.is_scope_start(Addr(0x2000)));
        assert!(round_tripped.is_scope_end(Addr(0x2011)));
    }

    // T1.6
    #[test]
    fn test_annotation_manager_non_scope_update_preserves_scopes() {
        let mut manager = AnnotationManager::default();
        manager.update(Addr(0x1000), |e| {
            e.scope = Some(Addr(0x1020));
            e.user_side_comment = Some("initial".to_string());
        });
        manager.update(Addr(0x2000), |e| {
            e.system_comment = Some("sys".to_string());
        });

        // Mutate non-scope field on the entry that has a scope
        manager.update(Addr(0x1000), |e| {
            e.user_side_comment = Some("updated".to_string());
        });
        // Mutate non-scope field on an entry without a scope
        manager.update(Addr(0x2000), |e| {
            e.system_comment = None;
        });

        assert!(manager.is_scope_start(Addr(0x1000)));
        assert!(manager.is_scope_end(Addr(0x1021)));
        assert_eq!(manager.scopes().len(), 1);
        assert_eq!(manager.scope_ends_set().len(), 1);
    }
}
