use crate::state::types::{Addr, ImmediateFormat};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[allow(clippy::unnecessary_map_or)]
fn is_str_none_or_empty(s: &Option<String>) -> bool {
    s.as_deref().map_or(true, |st| st.trim().is_empty())
}

/// Compact sparse annotation entry storing user annotations & formatting overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressEntry {
    /// Auto-generated or system comment (loaded from system assets; not saved in project files).
    #[serde(default, skip_serializing)]
    pub system_comment: Option<String>,

    /// User-defined side (inline) comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_side_comment: Option<String>,

    /// User-defined line (above instruction) comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_line_comment: Option<String>,

    /// Specific formatting override for immediate operand values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immediate_format: Option<ImmediateFormat>,

    /// User bookmark title or description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,

    /// Local symbol scope boundary parent address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Addr>,

    /// Named enumeration type usage.
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

    /// Returns true if all metadata fields are empty, None, or empty strings.
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

    /// Returns true if the entry contains user-defined annotations that must be persisted to project files.
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

/// Encapsulated manager guaranteeing deterministic BTreeMap node pruning on empty entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationManager {
    map: BTreeMap<Addr, AddressEntry>,
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
    /// Fetches an immutable reference to the annotation entry at `addr`, if present.
    #[must_use]
    pub fn get(&self, addr: Addr) -> Option<&AddressEntry> {
        self.map.get(&addr)
    }

    /// Mutates an entry in-place, normalizes string fields, and automatically prunes empty BTreeMap nodes.
    pub fn update<F>(&mut self, addr: Addr, f: F)
    where
        F: FnOnce(&mut AddressEntry),
    {
        use std::collections::btree_map::Entry;
        match self.map.entry(addr) {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                entry.get_mut().normalize();
                if entry.get().is_empty() {
                    entry.remove();
                }
            }
            Entry::Vacant(entry) => {
                let mut new_entry = AddressEntry::default();
                f(&mut new_entry);
                new_entry.normalize();
                if !new_entry.is_empty() {
                    entry.insert(new_entry);
                }
            }
        }
    }

    /// Returns an iterator over all non-empty address annotations.
    pub fn iter(&self) -> impl Iterator<Item = (Addr, &AddressEntry)> {
        self.map.iter().map(|(&a, e)| (a, e))
    }

    /// Clears all annotations.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns true if there are no annotations stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the number of annotated addresses.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns a set of addresses immediately following the end of each scope annotation.
    #[must_use]
    pub fn scope_ends(&self) -> BTreeSet<Addr> {
        self.map
            .values()
            .filter_map(|e| e.scope)
            .map(|end| end.wrapping_add(1))
            .collect()
    }
}

impl<'de> serde::Deserialize<'de> for AnnotationManager {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
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
}
