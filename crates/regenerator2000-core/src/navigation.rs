//! Address navigation, memory-grid range math, and cursor state for project saves.

use crate::disassembler::DisassemblyLine;
use crate::state::{Addr, AppState, ProjectSaveContext};
use crate::view_state::{ActivePane, CoreViewState, NavigationTarget, RightPane};
use std::collections::BTreeMap;
use std::ops::Range;

/// Returns `true` when `target_addr` lies inside the inclusive wrap-aware range `start_addr..=end_addr`.
///
/// When `start_addr > end_addr`, the range crosses the `$FFFF -> $0000` boundary.
#[must_use]
pub(crate) fn address_in_inclusive_span(
    target_addr: Addr,
    start_addr: Addr,
    end_addr: Addr,
) -> bool {
    if start_addr <= end_addr {
        target_addr >= start_addr && target_addr <= end_addr
    } else {
        target_addr >= start_addr || target_addr <= end_addr
    }
}

/// Returns `true` when `target_addr` lies inside the half-open wrap-aware byte span
/// `[start_addr, start_addr + byte_len)`.
///
/// Handles spans that reach `$FFFF` and wrap to `$0000` (for example, `$FFFE` with `byte_len == 2`
/// produces `end_addr == $0000`, matching `$FFFE..=$FFFF` without matching `$0000`).
#[must_use]
pub(crate) fn address_in_byte_span(target_addr: Addr, start_addr: Addr, byte_len: usize) -> bool {
    let Ok(len_u16) = u16::try_from(byte_len) else {
        return false;
    };
    if len_u16 == 0 {
        return false;
    }
    let end_addr = start_addr.wrapping_add(len_u16);
    if start_addr < end_addr {
        target_addr >= start_addr && target_addr < end_addr
    } else {
        target_addr >= start_addr || target_addr < end_addr
    }
}

/// Resolves the raw disassembly line index for `target_addr` before zero-byte header advancement.
///
/// Delegates span and `$0000` preamble isolation to [`AppState::get_line_index_containing_address`],
/// falling back to [`AppState::get_line_index_for_address`] only for non-external non-zero gaps.
#[must_use]
fn resolve_jump_line_index(app_state: &AppState, target_addr: Addr) -> Option<usize> {
    app_state
        .get_line_index_containing_address(target_addr)
        .or_else(|| {
            (!app_state.is_external(target_addr) && target_addr != Addr::ZERO)
                .then(|| app_state.get_line_index_for_address(target_addr))
                .flatten()
        })
}

/// Advances `idx` past consecutive zero-byte body headers (`{splitter}`, scope `{`) that share
/// `target_addr` without walking off external equates or collapsed summary rows.
#[must_use]
fn advance_to_content_line(app_state: &AppState, target_addr: Addr, mut idx: usize) -> usize {
    // Check `external_label_address.is_none()` on both `idx` and `idx + 1` so an external `$0000`
    // equate never walks onto the trailing preamble blank separator (also at `Addr::ZERO`), and
    // require `!is_collapsed` so a collapsed summary row never steps onto an adjacent header.
    while idx + 1 < app_state.disassembly.len()
        && app_state.disassembly[idx].external_label_address.is_none()
        && !app_state.disassembly[idx].is_collapsed
        && app_state.disassembly[idx].address == target_addr
        && app_state.disassembly[idx].bytes.is_empty()
        && app_state.disassembly[idx + 1]
            .external_label_address
            .is_none()
        && app_state.disassembly[idx + 1].address == target_addr
    {
        idx += 1;
    }
    idx
}

/// Returns the content [`DisassemblyLine`] index in [`AppState::disassembly`] for `address`.
///
/// Skips preamble lines and zero-byte headers at `address`. Both cursor jumps
/// ([`perform_jump_to_address`], [`perform_jump_to_address_no_history`]) and target previews
/// ([`AppState::resolve_disassembly_jump_targets`]) call this function so they land on the
/// same row.
///
/// - External addresses match only preamble equate rows (`external_label_address == Some(address)`).
/// - Collapsed blocks stop on their summary row.
/// - Multi-byte instructions match interior bytes without advancing to the next line.
/// - Out-of-range addresses without an external equate row return `None`.
#[must_use]
pub fn resolve_jump_content_line_index(state: &AppState, address: Addr) -> Option<usize> {
    let raw_idx = resolve_jump_line_index(state, address)?;
    Some(advance_to_content_line(state, address, raw_idx))
}

/// Converts an address-sorted map of first line indices into a suffix-minimum vector where each
/// `(addr, min_idx)` stores the smallest disassembly index among all entries with `>= addr`.
#[must_use]
fn build_suffix_min_table(entries: BTreeMap<Addr, usize>) -> Vec<(Addr, usize)> {
    let mut table = Vec::with_capacity(entries.len());
    let mut running_min = usize::MAX;
    for (addr, idx) in entries.into_iter().rev() {
        running_min = running_min.min(idx);
        table.push((addr, running_min));
    }
    table.reverse();
    table
}

/// Returns the smallest disassembly index among lines with `address >= target_addr` from a table
/// produced by [`build_suffix_min_table`].
#[must_use]
fn suffix_min_lookup(table: &[(Addr, usize)], target_addr: Addr) -> Option<usize> {
    let pos = table.partition_point(|&(addr, _)| addr < target_addr);
    table.get(pos).map(|&(_, min_idx)| min_idx)
}

/// Transient `O(L)` index accelerating batch [`resolve_jump_content_line_index`] queries when
/// building target previews for multi-entry pointer or split-address table lines.
pub(crate) struct JumpTargetIndex {
    collapsed_by_addr: BTreeMap<Addr, usize>,
    byte_spans: Vec<(Addr, u16, usize)>,
    spans_monotonic_disjoint: bool,
    exact_byte_line: BTreeMap<Addr, usize>,
    external_equates: BTreeMap<Addr, usize>,
    zero_byte_internal: BTreeMap<Addr, usize>,
    zero_addr_marker_idx: Option<usize>,
    min_idx_ge_byte_addr: Vec<(Addr, usize)>,
    min_idx_ge_any_addr: Vec<(Addr, usize)>,
}

impl JumpTargetIndex {
    /// Builds a single-pass address resolution index over `app_state.disassembly`.
    #[must_use]
    pub(crate) fn new(app_state: &AppState) -> Self {
        let mut collapsed_by_addr = BTreeMap::new();
        let mut byte_spans = Vec::new();
        let mut spans_monotonic_disjoint = true;
        let mut prev_end_excl = 0u32;
        let mut exact_byte_line = BTreeMap::new();
        let mut external_equates = BTreeMap::new();
        let mut zero_byte_internal = BTreeMap::new();
        let mut zero_addr_marker_idx = None;
        let mut first_any_by_addr = BTreeMap::new();

        for (idx, line) in app_state.disassembly.iter().enumerate() {
            first_any_by_addr.entry(line.address).or_insert(idx);

            if line.is_collapsed {
                collapsed_by_addr.entry(line.address).or_insert(idx);
            }

            if let Some(ext_addr) = line.external_label_address {
                external_equates.entry(ext_addr).or_insert(idx);
                continue;
            }

            if line.bytes.is_empty() {
                zero_byte_internal.entry(line.address).or_insert(idx);
                if line.address == Addr::ZERO && (line.is_splitter() || line.label.is_some()) {
                    zero_addr_marker_idx.get_or_insert(idx);
                }
            } else {
                exact_byte_line.entry(line.address).or_insert(idx);
                if let Ok(len_u16) = u16::try_from(line.bytes.len())
                    && len_u16 > 0
                {
                    let start_u32 = u32::from(line.address.0);
                    let end_excl = start_u32 + u32::from(len_u16);
                    if start_u32 < prev_end_excl || end_excl > 0x1_0000 {
                        spans_monotonic_disjoint = false;
                    } else {
                        prev_end_excl = end_excl;
                    }
                    byte_spans.push((line.address, len_u16, idx));
                }
            }
        }

        // Preserve `exact_byte_line` for exact fallback checks while building the suffix-min table.
        let min_idx_ge_byte_addr = build_suffix_min_table(exact_byte_line.clone());
        let min_idx_ge_any_addr = build_suffix_min_table(first_any_by_addr);

        Self {
            collapsed_by_addr,
            byte_spans,
            spans_monotonic_disjoint,
            exact_byte_line,
            external_equates,
            zero_byte_internal,
            zero_addr_marker_idx,
            min_idx_ge_byte_addr,
            min_idx_ge_any_addr,
        }
    }

    /// Resolves the raw disassembly line index for `target_addr`, matching [`resolve_jump_line_index`].
    #[must_use]
    fn resolve_raw_line_index(&self, app_state: &AppState, target_addr: Addr) -> Option<usize> {
        let is_ext = app_state.is_external(target_addr);

        if !is_ext {
            for (start_idx, end_idx) in &app_state.collapsed_blocks {
                let start_addr = app_state.origin.wrapping_add(*start_idx as u16);
                let end_addr = app_state.origin.wrapping_add(*end_idx as u16);
                if address_in_inclusive_span(target_addr, start_addr, end_addr)
                    && let Some(&idx) = self.collapsed_by_addr.get(&start_addr)
                {
                    return Some(idx);
                }
            }
        }

        if self.spans_monotonic_disjoint {
            let pos = self
                .byte_spans
                .partition_point(|&(start, _, _)| start <= target_addr);
            if let Some(prev) = pos.checked_sub(1)
                && let Some(&(start, len_u16, idx)) = self.byte_spans.get(prev)
                && address_in_byte_span(target_addr, start, usize::from(len_u16))
            {
                return Some(idx);
            }
        } else {
            for &(start, len_u16, idx) in &self.byte_spans {
                if address_in_byte_span(target_addr, start, usize::from(len_u16)) {
                    return Some(idx);
                }
            }
        }

        if is_ext {
            return self.external_equates.get(&target_addr).copied();
        }

        if let Some(&idx) = self.collapsed_by_addr.get(&target_addr) {
            return Some(idx);
        }

        if target_addr != Addr::ZERO {
            if let Some(&idx) = self.zero_byte_internal.get(&target_addr) {
                return Some(idx);
            }

            // Mirror `AppState::get_line_index_for_address` fallback ordering.
            if let Some(&idx) = self.exact_byte_line.get(&target_addr) {
                return Some(idx);
            }
            let exact_internal = match (
                self.exact_byte_line.get(&target_addr),
                self.zero_byte_internal.get(&target_addr),
            ) {
                (Some(&a), Some(&b)) => Some(a.min(b)),
                (Some(&a), None) | (None, Some(&a)) => Some(a),
                (None, None) => None,
            };
            if exact_internal.is_some() {
                return exact_internal;
            }
            if let Some(&idx) = self.external_equates.get(&target_addr) {
                return Some(idx);
            }
            if let Some(idx) = suffix_min_lookup(&self.min_idx_ge_byte_addr, target_addr) {
                return Some(idx);
            }
            return suffix_min_lookup(&self.min_idx_ge_any_addr, target_addr);
        }

        self.zero_addr_marker_idx
    }

    /// Resolves the final content line index for `target_addr`, matching [`resolve_jump_content_line_index`].
    #[must_use]
    pub(crate) fn resolve_content_line_index(
        &self,
        app_state: &AppState,
        target_addr: Addr,
    ) -> Option<usize> {
        let raw_idx = self.resolve_raw_line_index(app_state, target_addr)?;
        Some(advance_to_content_line(app_state, target_addr, raw_idx))
    }
}

/// Jumps to `target_addr` and pushes the current cursor position onto [`CoreViewState::navigation_history`].
///
/// If [`resolve_jump_content_line_index`] returns `None`, emits an out-of-range status message
/// on `view_state` without modifying cursor position or navigation history.
pub fn perform_jump_to_address(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
) {
    let Some(idx) = resolve_jump_content_line_index(app_state, target_addr) else {
        report_out_of_range(app_state, view_state, target_addr);
        return;
    };

    let active_pane = view_state.active_pane;
    let target = match active_pane {
        ActivePane::Disassembly => {
            if let Some(current_line) = app_state.disassembly.get(view_state.cursor_index) {
                let addr = current_line
                    .external_label_address
                    .unwrap_or(current_line.address);
                NavigationTarget::Address(addr.0)
            } else {
                NavigationTarget::Index(view_state.cursor_index)
            }
        }
        ActivePane::HexDump => NavigationTarget::Index(view_state.hex_cursor_index),
        ActivePane::Sprites => NavigationTarget::Index(view_state.sprites_cursor_index),
        ActivePane::Charset => NavigationTarget::Index(view_state.charset_cursor_index),
        ActivePane::Bitmap => NavigationTarget::Index(view_state.bitmap_cursor_index),
        ActivePane::Blocks => {
            NavigationTarget::Index(view_state.blocks_selected_index.unwrap_or(0))
        }
        ActivePane::Debugger => NavigationTarget::Index(view_state.cursor_index),
    };
    view_state.navigation_history.push((active_pane, target));

    apply_resolved_jump(app_state, view_state, target_addr, idx);
}

/// Jumps to `target_addr` without modifying [`CoreViewState::navigation_history`].
///
/// Emits an out-of-range status message on `view_state` when [`resolve_jump_content_line_index`]
/// returns `None`.
pub fn perform_jump_to_address_no_history(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
) {
    let Some(idx) = resolve_jump_content_line_index(app_state, target_addr) else {
        report_out_of_range(app_state, view_state, target_addr);
        return;
    };
    apply_resolved_jump(app_state, view_state, target_addr, idx);
}

/// Applies cursor, viewport scroll, sub-cursor, active pane, and status updates after a target
/// content index has already been resolved and advanced past zero-byte headers.
fn apply_resolved_jump(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
    content_idx: usize,
) {
    // Preserve the current scroll offset when `content_idx` is already inside the viewport.
    let is_visible = view_state.disassembly_viewport_height > 0
        && content_idx >= view_state.scroll_index
        && content_idx < view_state.scroll_index + view_state.disassembly_viewport_height;

    view_state.cursor_index = content_idx;
    if !is_visible {
        view_state.scroll_index = content_idx;
        view_state.scroll_sub_index = 0;
    }

    if let Some(line) = app_state.disassembly.get(content_idx) {
        view_state.sub_cursor_index = line.get_sub_index_for_address(app_state, target_addr);
    } else {
        view_state.sub_cursor_index = 0;
    }

    view_state.active_pane = ActivePane::Disassembly;
    view_state.status_message = Some(format!("Jumped to ${target_addr:04X}"));
}

fn report_out_of_range(app_state: &AppState, view_state: &mut CoreViewState, target_addr: Addr) {
    if !app_state.disassembly.is_empty() {
        let end = app_state
            .origin
            .wrapping_add(app_state.raw_data.len().saturating_sub(1) as u16);
        view_state.status_message = Some(format!(
            "Address ${target_addr:04X} out of range (${:04X}-${:04X})",
            app_state.origin.0, end.0
        ));
    }
}

/// Floor-aligned memory grid geometry for right-pane views (`HexDump`, `Sprites`, `Charset`,
/// and `Bitmap`).
///
/// Each view aligns its first grid slot to the floor multiple of `alignment` at or below
/// `app_state.origin`. It pads bytes before `origin` with zeros to preserve hardware boundaries
/// (`$2000` bitmaps, `$0400` charsets, `$0040` sprites, and `$0008`/`$0010` hex rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignedGridRange {
    /// First loaded byte address (`app_state.origin`).
    pub origin: usize,
    /// Floor-aligned address of grid item `0`.
    pub aligned_origin: usize,
    /// Exclusive upper byte bound (`min(origin + raw_data.len(), 0x10000)`).
    pub end_address: usize,
    /// Address spacing between consecutive grid items.
    pub stride: usize,
    /// Active payload size in bytes for each grid item.
    pub item_bytes: usize,
}

impl AlignedGridRange {
    /// Computes the floor-aligned grid range for `app_state` with explicit `alignment`,
    /// per-item address `stride`, and per-item payload size `item_bytes`.
    #[must_use]
    pub fn new(app_state: &AppState, alignment: usize, stride: usize, item_bytes: usize) -> Self {
        let origin = app_state.origin.0 as usize;
        let aligned_origin = (origin / alignment) * alignment;
        let end_address = (origin + app_state.raw_data.len()).min(0x10000);
        Self {
            origin,
            aligned_origin,
            end_address,
            stride,
            item_bytes,
        }
    }

    /// Computes a uniform floor-aligned grid range where `alignment == stride == item_bytes`.
    #[must_use]
    pub fn uniform(app_state: &AppState, stride: usize) -> Self {
        Self::new(app_state, stride, stride, stride)
    }

    /// Computes the canonical hex dump grid range (`8` bytes per row for [`RightPane::HexDump8`],
    /// `16` bytes per row otherwise).
    #[must_use]
    pub fn hex_dump(app_state: &AppState, right_pane: RightPane) -> Self {
        let bytes_per_row = match right_pane {
            RightPane::HexDump8 => 8,
            _ => 16,
        };
        Self::uniform(app_state, bytes_per_row)
    }

    /// Computes the canonical sprites grid range (64-byte hardware sprite slots).
    #[must_use]
    pub fn sprites(app_state: &AppState) -> Self {
        Self::uniform(app_state, 64)
    }

    /// Computes the canonical charset grid range (`$0400`-aligned 8-byte character tiles).
    #[must_use]
    pub fn charset(app_state: &AppState) -> Self {
        Self::new(app_state, 0x400, 8, 8)
    }

    /// Computes the canonical bitmap grid range (`$2000`-aligned 8,000-byte bitmaps).
    #[must_use]
    pub fn bitmap(app_state: &AppState) -> Self {
        Self::new(app_state, 8192, 8192, 8000)
    }

    /// Returns the number of grid items in `[aligned_origin, end_address)`.
    ///
    /// Returns `0` if `raw_data` is empty (`end_address <= origin`).
    #[must_use]
    pub fn item_count(&self) -> usize {
        if self.end_address <= self.origin {
            return 0;
        }
        self.end_address
            .saturating_sub(self.aligned_origin)
            .div_ceil(self.stride)
    }

    /// Returns the starting 16-bit memory address of grid item `index`.
    #[must_use]
    pub fn item_addr(&self, index: usize) -> usize {
        self.aligned_origin + index * self.stride
    }

    /// Returns the grid item index containing `addr`, or `None` when `addr < aligned_origin`
    /// or the resulting index is outside `0..self.item_count()`.
    #[must_use]
    pub fn index_for_addr(&self, addr: usize) -> Option<usize> {
        let idx = addr.checked_sub(self.aligned_origin)? / self.stride;
        (idx < self.item_count()).then_some(idx)
    }

    /// Reads the byte at `addr` from `raw_data`, returning `0` when `addr` falls in floor-alignment
    /// padding (`< origin`) or beyond `end_address`.
    #[inline]
    #[must_use]
    pub fn byte_at(&self, raw_data: &[u8], addr: usize) -> u8 {
        if addr >= self.origin && addr < self.end_address {
            raw_data.get(addr - self.origin).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Clamps `raw_addr` to the loaded binary range `[origin, end_address - 1]`, returning `None`
    /// when `raw_data` is empty.
    #[must_use]
    pub fn clamped_addr(&self, raw_addr: usize) -> Option<Addr> {
        (self.end_address > self.origin)
            .then(|| Addr(raw_addr.max(self.origin).min(self.end_address - 1) as u16))
    }

    /// Returns the clamped starting address of grid item `index` within `[origin, end_address - 1]`.
    #[must_use]
    pub fn clamped_item_addr(&self, index: usize) -> Option<Addr> {
        self.clamped_addr(self.item_addr(index))
    }

    /// Converts an inclusive `[start_addr, end_addr]` address range into a clamped
    /// `(start_offset, end_offset)` byte-offset pair within `raw_data`. Returns `None` when the
    /// range lies outside `[origin, end_address)`.
    #[must_use]
    pub fn clamped_byte_offsets(
        &self,
        start_addr: usize,
        end_addr: usize,
    ) -> Option<(usize, usize)> {
        if self.end_address <= self.origin
            || end_addr < self.origin
            || start_addr >= self.end_address
        {
            return None;
        }
        let clamped_start = start_addr.max(self.origin) - self.origin;
        let clamped_end = end_addr.min(self.end_address - 1) - self.origin;
        (clamped_start <= clamped_end).then_some((clamped_start, clamped_end))
    }

    /// Converts an inclusive `(first_item, last_item)` grid index range (in either order) into a
    /// clamped `(start_offset, end_offset)` byte-offset pair within `raw_data`.
    #[must_use]
    pub fn item_range_byte_offsets(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        let (first, last) = if a <= b { (a, b) } else { (b, a) };
        let start_addr = self.item_addr(first);
        let end_addr = self.item_addr(last) + self.item_bytes.saturating_sub(1);
        self.clamped_byte_offsets(start_addr, end_addr)
    }

    /// Returns the clamped `(start_addr, end_addr)` range for grid items `first_item..=second_item`
    /// (in either order).
    ///
    /// Clamps both bounds to `[origin, end_address - 1]`. Returns `None` if the item range lies
    /// outside the binary (for example, inside alignment padding before `origin`).
    ///
    /// # Examples
    ///
    /// - **Sprites (`64` bytes):** `(37, 37)` returns `$0940..=$097F`. Both `(37, 38)` and
    ///   `(38, 37)` return `$0940..=$09BF`.
    /// - **Pre-origin padding (`origin = $0801`):** Sprite `0` (`$0800..=$083F`) clamps to
    ///   `$0801..=$083F`.
    /// - **Bitmaps (`stride = 8192`, `item_bytes = 8000`):** `(0, 0)` returns `$0000..=$1F3F` and
    ///   excludes the 192-byte gap. `(0, 1)` returns `$0000..=$3F3F` as one contiguous range.
    #[must_use]
    pub fn clamped_item_addr_range(
        &self,
        first_item: usize,
        second_item: usize,
    ) -> Option<(Addr, Addr)> {
        let (start_offset, end_offset) = self.item_range_byte_offsets(first_item, second_item)?;
        // `item_range_byte_offsets` clamps within `[origin, min(end_address, 0x10000) - 1]`.
        // Therefore `self.origin + end_offset` always fits in `u16`.
        let start_addr = Addr((self.origin + start_offset) as u16);
        let end_addr = Addr((self.origin + end_offset) as u16);
        Some((start_addr, end_addr))
    }

    /// Returns the half-open `raw_data` byte range `start..end` for a hex dump cursor and
    /// optional selection anchor.
    ///
    /// Clamps column indices to `self.stride - 1` so an 8-column switch cannot overflow into
    /// the next row. Returns `None` if `self.stride == 0`, `raw_data` is empty, or the range
    /// lies in alignment padding before `origin`.
    #[must_use]
    pub fn hex_byte_offset_range(
        &self,
        cursor_row: usize,
        cursor_col: usize,
        selection_start: Option<(usize, usize)>,
    ) -> Option<Range<usize>> {
        let max_col = self.stride.checked_sub(1)?;
        let cursor_addr = self.item_addr(cursor_row) + cursor_col.min(max_col);
        let (start_addr, end_addr) = if let Some((selection_row, selection_col)) = selection_start {
            let anchor_addr = self.item_addr(selection_row) + selection_col.min(max_col);
            (anchor_addr.min(cursor_addr), anchor_addr.max(cursor_addr))
        } else {
            (cursor_addr, cursor_addr)
        };
        // `clamped_byte_offsets` returns inclusive offsets `< raw_data.len()`, so `end + 1` will not overflow.
        self.clamped_byte_offsets(start_addr, end_addr)
            .map(|(start, end)| start..(end + 1))
    }

    /// Returns the clamped `(start_addr, end_addr)` range of the focused item in `Sprites`,
    /// `Charset`, or `Bitmap` when no disassembly selection is active.
    ///
    /// The disassembly view uses this range to highlight all bytes of the focused item.
    ///
    /// - Returns `None` if [`CoreViewState::selection_start`] is `Some(_)`, so cursor focus never
    ///   overrides an active selection.
    /// - Returns `None` for [`ActivePane::HexDump`] because each hex cell is already one byte.
    /// - Returns `None` for [`ActivePane::Disassembly`], [`ActivePane::Blocks`], and
    ///   [`ActivePane::Debugger`].
    #[must_use]
    pub fn active_pane_item_addr_range(
        app_state: &AppState,
        view_state: &CoreViewState,
    ) -> Option<(Addr, Addr)> {
        if view_state.selection_start.is_some() {
            return None;
        }
        match view_state.active_pane {
            ActivePane::Sprites => {
                let index = view_state.sprites_cursor_index;
                Self::sprites(app_state).clamped_item_addr_range(index, index)
            }
            ActivePane::Charset => {
                let index = view_state.charset_cursor_index;
                Self::charset(app_state).clamped_item_addr_range(index, index)
            }
            ActivePane::Bitmap => {
                let index = view_state.bitmap_cursor_index;
                Self::bitmap(app_state).clamped_item_addr_range(index, index)
            }
            ActivePane::Disassembly
            | ActivePane::HexDump
            | ActivePane::Blocks
            | ActivePane::Debugger => None,
        }
    }
}

/// Returns the half-open `raw_data` byte range `start..end` for the active memory pane
/// (`HexDump`, `Sprites`, `Charset`, or `Bitmap`).
///
/// Edit menu block conversions and the `b` shortcut share this range calculation:
///
/// - Without a selection anchor, it covers the focused item (`1` hex byte, `64` sprite bytes,
///   `8` charset bytes, or `8,000` bitmap bytes).
/// - With a selection anchor, it covers all items from the anchor to the cursor.
/// - Returns `None` if another pane is active or if the range lies in alignment padding
///   before `origin`.
#[must_use]
pub fn active_pane_byte_offset_range(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> Option<Range<usize>> {
    let (grid, cursor, selection) = match view_state.active_pane {
        ActivePane::HexDump => {
            return AlignedGridRange::hex_dump(app_state, view_state.right_pane)
                .hex_byte_offset_range(
                    view_state.hex_cursor_index,
                    view_state.hex_col_cursor,
                    view_state
                        .hex_selection_start
                        .map(|row| (row, view_state.hex_selection_start_col)),
                );
        }
        ActivePane::Sprites => (
            AlignedGridRange::sprites(app_state),
            view_state.sprites_cursor_index,
            view_state.sprites_selection_start,
        ),
        ActivePane::Charset => (
            AlignedGridRange::charset(app_state),
            view_state.charset_cursor_index,
            view_state.charset_selection_start,
        ),
        ActivePane::Bitmap => (
            AlignedGridRange::bitmap(app_state),
            view_state.bitmap_cursor_index,
            view_state.bitmap_selection_start,
        ),
        ActivePane::Disassembly | ActivePane::Blocks | ActivePane::Debugger => return None,
    };
    let anchor = selection.unwrap_or(cursor);
    grid.item_range_byte_offsets(cursor, anchor)
        .map(|(start, end)| start..(end + 1))
}

/// Returns the half-open `raw_data` byte range `start..end` covered by `line`.
///
/// - Returns `None` for external equates, `$0000` preamble lines, splitters, scope delimiters
///   (`{` and `}`), and out-of-range addresses.
/// - For collapsed blocks (`line.is_collapsed`), resolves the full block range from
///   [`AppState::collapsed_blocks`] or [`AppState::get_block_range`].
/// - Clamps the end offset to `state.raw_data.len()`.
#[must_use]
pub fn line_byte_offset_range(state: &AppState, line: &DisassemblyLine) -> Option<Range<usize>> {
    if line.external_label_address.is_some() || state.is_external(line.address) {
        return None;
    }
    let start_offset = line.address.offset_from(state.origin);
    if start_offset >= state.raw_data.len() {
        return None;
    }
    if line.is_collapsed {
        if let Some(&(_, end_offset)) = state
            .collapsed_blocks
            .iter()
            .find(|&&(block_start, _)| block_start == start_offset)
        {
            let end_exclusive = end_offset.saturating_add(1).min(state.raw_data.len());
            return (start_offset < end_exclusive).then_some(start_offset..end_exclusive);
        }
        if let Some((start_addr, end_addr)) = state.get_block_range(line.address) {
            let block_start = start_addr.offset_from(state.origin);
            let end_exclusive = end_addr
                .offset_from(state.origin)
                .saturating_add(1)
                .min(state.raw_data.len());
            return (block_start < end_exclusive).then_some(block_start..end_exclusive);
        }
        // Collapsed summary rows leave `line.bytes` empty, so return `None` if lookup fails.
        return None;
    }
    if line.bytes.is_empty() {
        return None;
    }
    let end_exclusive = start_offset
        .saturating_add(line.bytes.len())
        .min(state.raw_data.len());
    (start_offset < end_exclusive).then_some(start_offset..end_exclusive)
}

/// Returns the inclusive `usize` address range `(start, end)` covered by `line`.
///
/// Calls [`line_byte_offset_range`] and returns `None` for non-binary lines (such as external
/// equates, preamble lines, splitters, and scope delimiters). Uses `usize` instead of `u16`
/// so blocks that cross `$FFFF` (for example, `$FFFE..=$0001` as `0xFFFE..=0x10001`) do not
/// wrap around.
#[must_use]
pub fn line_byte_addr_span(app_state: &AppState, line: &DisassemblyLine) -> Option<(usize, usize)> {
    let range = line_byte_offset_range(app_state, line)?;
    // Guard against empty ranges before subtracting `1` from `range.end`.
    if range.is_empty() {
        return None;
    }
    let origin_usize = usize::from(app_state.origin.0);
    Some((origin_usize + range.start, origin_usize + range.end - 1))
}

/// Returns the half-open `raw_data` byte range `first.start..last.end` across `slice`.
///
/// Scans from both ends of `slice` for the first and last lines with binary bytes via
/// [`line_byte_offset_range`]. Skips external equates, preamble lines, splitters, and scope
/// delimiters. Returns `None` if `slice` covers no binary bytes.
#[must_use]
pub fn slice_byte_offset_range(
    state: &AppState,
    slice: &[DisassemblyLine],
) -> Option<Range<usize>> {
    let first = slice
        .iter()
        .find_map(|line| line_byte_offset_range(state, line))?;
    let last = slice
        .iter()
        .rev()
        .find_map(|line| line_byte_offset_range(state, line))?;
    (first.start < last.end).then_some(first.start..last.end)
}

/// Returns the inclusive `usize` address range `(start, end)` for a disassembly selection or
/// cursor line.
///
/// If `selection_start` is `Some(anchor)`, resolves the byte range across
/// `anchor..=cursor_index` via [`slice_byte_offset_range`]. Otherwise, resolves `cursor_index`
/// via [`line_byte_addr_span`]. Returns `None` if the selection covers no binary bytes.
#[must_use]
pub fn disassembly_selection_addr_span(
    app_state: &AppState,
    selection_start: Option<usize>,
    cursor_index: usize,
) -> Option<(usize, usize)> {
    if let Some(anchor_index) = selection_start {
        let (min_index, max_index) = if anchor_index <= cursor_index {
            (anchor_index, cursor_index)
        } else {
            (cursor_index, anchor_index)
        };
        let slice = app_state.disassembly.get(min_index..=max_index)?;
        let range = slice_byte_offset_range(app_state, slice)?;
        if range.is_empty() {
            return None;
        }
        let origin_usize = usize::from(app_state.origin.0);
        Some((origin_usize + range.start, origin_usize + range.end - 1))
    } else {
        let line = app_state.disassembly.get(cursor_index)?;
        line_byte_addr_span(app_state, line)
    }
}

/// Maps a disassembly selection to `(anchor_item, cursor_item)` indices in `grid`.
///
/// Resolves the byte range via [`disassembly_selection_addr_span`] and clamps it to
/// `[grid.aligned_origin, grid.end_address - 1]`. Preserves selection direction: returns
/// `(start_item, end_item)` when `selection_start <= Some(cursor_index)` and
/// `(end_item, start_item)` when backward. Returns `None` if `selection_start` is `None` or
/// the selection does not overlap `grid`.
#[must_use]
pub fn mirror_disassembly_selection_to_grid(
    app_state: &AppState,
    selection_start: Option<usize>,
    cursor_index: usize,
    grid: &AlignedGridRange,
) -> Option<(usize, usize)> {
    let anchor_index = selection_start?;
    if grid.end_address <= grid.aligned_origin || grid.stride == 0 {
        return None;
    }
    let (first_start, last_end) =
        disassembly_selection_addr_span(app_state, Some(anchor_index), cursor_index)?;
    let max_grid_addr = grid.end_address - 1;
    if first_start > max_grid_addr || last_end < grid.aligned_origin {
        return None;
    }

    // Clamp spans that cross `$FFFF` into `[aligned_origin, max_grid_addr]` before division.
    let clamped_start = first_start.max(grid.aligned_origin);
    let clamped_end = last_end.min(max_grid_addr);
    let max_item = grid.item_count().saturating_sub(1);
    let start_item = ((clamped_start - grid.aligned_origin) / grid.stride).min(max_item);
    let end_item = ((clamped_end - grid.aligned_origin) / grid.stride).min(max_item);

    if anchor_index <= cursor_index {
        Some((start_item, end_item))
    } else {
        Some((end_item, start_item))
    }
}

/// Syncs a grid pane (`Charset`, `Sprites`, or `Bitmap`) from the disassembly cursor and
/// selection.
///
/// - If the disassembly selection overlaps `grid`, sets `*pane_selection` and `*pane_cursor`
///   to the matching item range and preserves direction.
/// - Otherwise, clears `*pane_selection` and moves `*pane_cursor` to `sync_target_addr` when
///   present.
pub fn sync_grid_pane_from_disassembly(
    app_state: &AppState,
    disassembly_selection: Option<usize>,
    disassembly_cursor: usize,
    sync_target_addr: Option<Addr>,
    grid: &AlignedGridRange,
    pane_cursor: &mut usize,
    pane_selection: &mut Option<usize>,
) {
    if let Some((anchor_item, cursor_item)) = mirror_disassembly_selection_to_grid(
        app_state,
        disassembly_selection,
        disassembly_cursor,
        grid,
    ) {
        *pane_selection = Some(anchor_item);
        *pane_cursor = cursor_item;
        return;
    }

    *pane_selection = None;
    if let Some(target_addr) = sync_target_addr
        && let Some(index) = grid.index_for_addr(usize::from(target_addr.0))
    {
        *pane_cursor = index;
    }
}

/// Syncs the `HexDump` cursor and selection from the disassembly pane.
///
/// Preserves byte-level hex selection anchors inside multi-byte instructions across `Tab`
/// switches when the disassembly selection still matches those lines.
pub fn sync_hex_pane_from_disassembly(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    sync_target_addr: Option<Addr>,
) {
    let grid = AlignedGridRange::hex_dump(app_state, view_state.right_pane);
    let Some(anchor_index) = view_state.selection_start else {
        view_state.hex_selection_start = None;
        view_state.hex_selection_start_col = 0;
        if let Some(target_addr) = sync_target_addr {
            let addr_usize = usize::from(target_addr.0);
            if let Some(row) = grid.index_for_addr(addr_usize) {
                view_state.hex_cursor_index = row;
                view_state.hex_col_cursor = (addr_usize - grid.aligned_origin) % grid.stride;
            }
        }
        return;
    };

    let Some(max_col) = grid.stride.checked_sub(1) else {
        return;
    };
    if grid.end_address <= grid.aligned_origin {
        return;
    }

    if let Some(selection_row) = view_state.hex_selection_start {
        let hex_anchor_raw =
            grid.item_addr(selection_row) + view_state.hex_selection_start_col.min(max_col);
        let hex_cursor_raw =
            grid.item_addr(view_state.hex_cursor_index) + view_state.hex_col_cursor.min(max_col);
        let expected_anchor = grid
            .clamped_addr(hex_anchor_raw)
            .and_then(|addr| app_state.get_line_index_containing_address(addr));
        let expected_cursor = grid
            .clamped_addr(hex_cursor_raw)
            .and_then(|addr| app_state.get_line_index_containing_address(addr));
        if expected_anchor == Some(anchor_index) && expected_cursor == Some(view_state.cursor_index)
        {
            return;
        }
    }

    let Some((first_start, last_end)) =
        disassembly_selection_addr_span(app_state, Some(anchor_index), view_state.cursor_index)
    else {
        view_state.hex_selection_start = None;
        view_state.hex_selection_start_col = 0;
        return;
    };

    let max_grid_addr = grid.end_address - 1;
    if first_start > max_grid_addr || last_end < grid.aligned_origin {
        view_state.hex_selection_start = None;
        view_state.hex_selection_start_col = 0;
        return;
    }

    let clamped_start = first_start.max(grid.aligned_origin);
    let clamped_end = last_end.min(max_grid_addr);
    let (anchor_addr, cursor_addr) = if anchor_index <= view_state.cursor_index {
        (clamped_start, clamped_end)
    } else {
        (clamped_end, clamped_start)
    };

    let max_row = grid.item_count().saturating_sub(1);
    let anchor_row = ((anchor_addr - grid.aligned_origin) / grid.stride).min(max_row);
    let anchor_col = (anchor_addr - grid.aligned_origin) % grid.stride;
    let cursor_row = ((cursor_addr - grid.aligned_origin) / grid.stride).min(max_row);
    let cursor_col = (cursor_addr - grid.aligned_origin) % grid.stride;

    view_state.hex_selection_start = Some(anchor_row);
    view_state.hex_selection_start_col = anchor_col;
    view_state.hex_cursor_index = cursor_row;
    view_state.hex_col_cursor = cursor_col;
}

/// Builds a [`ProjectSaveContext`] from the current application state and view state.
#[must_use]
pub fn create_save_context(app_state: &AppState, view_state: &CoreViewState) -> ProjectSaveContext {
    let cursor_addr = app_state
        .disassembly
        .get(view_state.cursor_index)
        .map(|l| l.external_label_address.unwrap_or(l.address));

    let has_data = !app_state.raw_data.is_empty();

    let hex_addr = has_data.then(|| {
        let grid = AlignedGridRange::hex_dump(app_state, view_state.right_pane);
        Addr(grid.item_addr(view_state.hex_cursor_index) as u16)
    });

    let sprites_addr = has_data.then(|| {
        let grid = AlignedGridRange::sprites(app_state);
        Addr(grid.item_addr(view_state.sprites_cursor_index) as u16)
    });

    let charset_addr = has_data.then(|| {
        let grid = AlignedGridRange::charset(app_state);
        Addr(grid.item_addr(view_state.charset_cursor_index) as u16)
    });

    let bitmap_addr = has_data.then(|| {
        let grid = AlignedGridRange::bitmap(app_state);
        Addr(grid.item_addr(view_state.bitmap_cursor_index) as u16)
    });

    ProjectSaveContext {
        cursor_address: cursor_addr,
        hex_dump_cursor_address: hex_addr,
        sprites_cursor_address: sprites_addr,
        right_pane_visible: Some(format!("{:?}", view_state.right_pane)),
        charset_cursor_address: charset_addr,
        bitmap_cursor_address: bitmap_addr,
        sprite_multicolor_mode: view_state.sprite_multicolor_mode,
        charset_multicolor_mode: view_state.charset_multicolor_mode,
        bitmap_multicolor_mode: view_state.bitmap_multicolor_mode,
        hexdump_view_mode: view_state.hexdump_view_mode,
        splitters: app_state.splitters.clone(),
        blocks_view_cursor: view_state.blocks_selected_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, AppState, BlockType, Label, LabelKind, LabelType};
    use crate::view_state::CoreViewState;

    #[test]
    fn test_perform_jump_to_address_pushes_external_label_address() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA];
        app_state.block_types = vec![BlockType::Code; 1];

        app_state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        app_state.settings.all_labels = true;
        app_state.disassemble();

        let ext_label_idx = app_state
            .get_line_index_for_address(Addr(0xD020))
            .expect("External label should be in disassembly");

        let mut view_state = CoreViewState {
            cursor_index: ext_label_idx,
            ..Default::default()
        };

        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0801));

        assert!(!view_state.navigation_history.is_empty());
        let (_pane, target) = view_state.navigation_history.last().unwrap();
        match target {
            NavigationTarget::Address(addr) => {
                assert_eq!(
                    *addr, 0xD020,
                    "Should have pushed $D020, but pushed ${addr:04X}"
                );
            }
            _ => panic!("Expected Address target, found {target:?}"),
        }
    }

    #[test]
    fn test_navigate_back_to_external_label() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA];
        app_state.block_types = vec![BlockType::Code; 1];

        app_state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        app_state.settings.all_labels = true;
        app_state.disassemble();

        let ext_label_idx = app_state.get_line_index_for_address(Addr(0xD020)).unwrap();

        let mut view_state = CoreViewState {
            cursor_index: ext_label_idx,
            ..Default::default()
        };

        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0801));

        let jumped_idx = app_state.get_line_index_for_address(Addr(0x0801)).unwrap();
        assert_eq!(view_state.cursor_index, jumped_idx);

        let (_, target) = view_state.navigation_history.pop().unwrap();
        match target {
            NavigationTarget::Address(addr) => {
                perform_jump_to_address_no_history(&app_state, &mut view_state, Addr(addr));
            }
            _ => panic!("Expected Address target"),
        }

        assert_eq!(
            view_state.cursor_index, ext_label_idx,
            "Should have returned to external label index {ext_label_idx}, instead got {}",
            view_state.cursor_index
        );
    }

    #[test]
    fn test_perform_jump_to_address_invalid_does_not_push_history() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA, 0xEA];
        app_state.block_types = vec![BlockType::Code; 2];
        app_state.disassemble();

        let mut view_state = CoreViewState {
            cursor_index: 1,
            ..Default::default()
        };

        // Jump to an address above the loaded range.
        perform_jump_to_address(&app_state, &mut view_state, Addr(0xC000));
        assert!(view_state.navigation_history.is_empty());
        assert_eq!(view_state.cursor_index, 1);

        // Jump to an address below `origin` ($0400 < $0801) with no external label.
        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0400));
        assert!(
            view_state.navigation_history.is_empty(),
            "Below-origin invalid jump must not push history or jump to origin"
        );
        assert_eq!(view_state.cursor_index, 1);
        assert!(
            view_state
                .status_message
                .as_deref()
                .is_some_and(|m| m.contains("out of range"))
        );
    }

    #[test]
    fn test_perform_jump_to_address_from_non_disassembly_pane() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 64];
        app_state.block_types = vec![BlockType::Code; 64];
        app_state.disassemble();

        let mut view_state = CoreViewState {
            active_pane: ActivePane::HexDump,
            hex_cursor_index: 2,
            ..Default::default()
        };

        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0801));

        assert_eq!(view_state.navigation_history.len(), 1);
        assert_eq!(
            view_state.navigation_history[0],
            (ActivePane::HexDump, NavigationTarget::Index(2))
        );
    }

    #[test]
    fn test_create_save_context_uses_floor_alignment_for_sprites_and_bitmap() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 16384];
        app_state.block_types = vec![BlockType::Code; 16384];
        app_state.disassemble();

        let view_state = CoreViewState {
            sprites_cursor_index: 2,
            bitmap_cursor_index: 1,
            charset_cursor_index: 3,
            hex_cursor_index: 4,
            ..Default::default()
        };

        let ctx = create_save_context(&app_state, &view_state);
        assert_eq!(ctx.sprites_cursor_address, Some(Addr(0x0800 + 2 * 64)));
        assert_eq!(ctx.bitmap_cursor_address, Some(Addr(8192)));
        assert_eq!(ctx.charset_cursor_address, Some(Addr(0x0800 + 3 * 8)));
        assert_eq!(ctx.hex_dump_cursor_address, Some(Addr(0x0800 + 4 * 16)));
    }

    #[test]
    fn core_nav_01_resolve_jump_content_line_index_body_and_mid_instruction() {
        use crate::state::BlockType;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        // $0801: LDA $1234 (AD 34 12), $0804: RTS (60)
        app_state.raw_data = vec![0xAD, 0x34, 0x12, 0x60];
        app_state.block_types = vec![BlockType::Code; 4];
        app_state.disassemble();

        let lda_idx = app_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x0801) && !l.bytes.is_empty())
            .expect("LDA $1234 line");
        let rts_idx = app_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x0804) && !l.bytes.is_empty())
            .expect("RTS line");

        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr(0x0801)),
            Some(lda_idx)
        );
        // Interior byte offsets inside the 3-byte LDA stay on the instruction line.
        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr(0x0802)),
            Some(lda_idx)
        );
        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr(0x0803)),
            Some(lda_idx)
        );
        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr(0x0804)),
            Some(rts_idx)
        );
    }

    #[test]
    fn core_nav_02_resolve_jump_content_line_index_collapsed_at_1000_and_0000() {
        use crate::disassembler::DisassemblyLine;
        use crate::state::BlockType;

        // 1. Collapsed block at $1002..=$1005 with following instruction at $1006.
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0x60];
        app_state.block_types = vec![BlockType::Code; 7];
        app_state.collapsed_blocks.push((2, 5));
        app_state.disassemble();

        let collapsed_idx = app_state
            .disassembly
            .iter()
            .position(|l| l.is_collapsed && l.address == Addr(0x1002))
            .expect("collapsed summary line at $1002");

        for target in [Addr(0x1002), Addr(0x1003), Addr(0x1005)] {
            assert_eq!(
                resolve_jump_content_line_index(&app_state, target),
                Some(collapsed_idx)
            );
        }

        // 2. Origin $0000 collapsed block preceded by `$0000` enum preamble headers must land strictly
        // on the collapsed summary row and never on preamble index 0 or the following instruction.
        let mut zero_state = AppState::new();
        zero_state.origin = Addr::ZERO;
        zero_state.raw_data = vec![0xEA, 0xEA, 0x60];
        zero_state.block_types = vec![BlockType::Code; 3];
        zero_state.collapsed_blocks.push((0, 1));
        zero_state.disassembly = vec![
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "; enum Mode".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "; Collapsed Code block from $0000-$0001".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: true,
            },
            DisassemblyLine {
                address: Addr(0x0002),
                bytes: vec![0x60],
                mnemonic: "rts".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];
        assert_eq!(
            resolve_jump_content_line_index(&zero_state, Addr::ZERO),
            Some(1)
        );
        assert_eq!(
            resolve_jump_content_line_index(&zero_state, Addr(0x0001)),
            Some(1)
        );
    }

    #[test]
    fn core_nav_03_resolve_jump_content_line_index_zero_external_equate_and_guard() {
        use crate::disassembler::DisassemblyLine;
        use crate::state::BlockType;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0x60];
        app_state.block_types = vec![BlockType::Code];
        app_state.disassembly = vec![
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "; enum Foo".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "zp_zero = $00".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: Some(Addr::ZERO),
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x0002),
                bytes: Vec::new(),
                mnemonic: "zp_two = $02".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: Some(Addr(0x0002)),
                is_collapsed: false,
            },
        ];

        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr::ZERO),
            Some(1)
        );

        // Even if a zero-byte splitter at `$0000` sits directly above a `$0000` external equate,
        // the `idx + 1` external-label guard stops advancement before sliding onto the equate.
        let mut guard_state = AppState::new();
        guard_state.origin = Addr::ZERO;
        guard_state.raw_data = vec![0xEA];
        guard_state.block_types = vec![BlockType::Code];
        guard_state.disassembly = vec![
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "{splitter}".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "ext_zero = $00".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: Some(Addr::ZERO),
                is_collapsed: false,
            },
        ];
        assert_eq!(
            resolve_jump_content_line_index(&guard_state, Addr::ZERO),
            Some(0)
        );
    }

    #[test]
    fn core_nav_04_resolve_jump_content_line_index_rejects_orphan_zero_preamble() {
        use crate::disassembler::DisassemblyLine;
        use crate::state::BlockType;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0x60];
        app_state.block_types = vec![BlockType::Code];
        app_state.disassembly = vec![
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "; enum Mode".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x0801),
                bytes: vec![0x60],
                mnemonic: "rts".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];

        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr::ZERO),
            None
        );
        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr(0xDEAD)),
            None
        );
    }

    #[test]
    fn core_nav_05_perform_jump_to_address_updates_history_pane_and_handles_invalid() {
        use crate::state::BlockType;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA, 0xEA, 0x60];
        app_state.block_types = vec![BlockType::Code; 3];
        app_state.disassemble();

        let target_idx = resolve_jump_content_line_index(&app_state, Addr(0x1002)).expect("$1002");

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::HexDump;
        view.hex_cursor_index = 4;

        perform_jump_to_address(&app_state, &mut view, Addr(0x1002));
        assert_eq!(view.active_pane, ActivePane::Disassembly);
        assert_eq!(view.cursor_index, target_idx);
        assert_eq!(
            view.navigation_history,
            vec![(ActivePane::HexDump, NavigationTarget::Index(4))]
        );

        // Invalid target does not mutate cursor, active_pane, or navigation_history.
        let history_before = view.navigation_history.clone();
        perform_jump_to_address(&app_state, &mut view, Addr(0xDEAD));
        assert_eq!(view.cursor_index, target_idx);
        assert_eq!(view.active_pane, ActivePane::Disassembly);
        assert_eq!(view.navigation_history, history_before);
        assert!(
            view.status_message
                .as_deref()
                .is_some_and(|msg| msg.contains("DEAD"))
        );
    }

    #[test]
    fn core_nav_06_perform_jump_to_address_no_history_preserves_history_stack() {
        use crate::state::BlockType;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA, 0x60];
        app_state.block_types = vec![BlockType::Code; 2];
        app_state.disassemble();

        let expected_idx =
            resolve_jump_content_line_index(&app_state, Addr(0x1001)).expect("$1001 line");

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Blocks;
        view.navigation_history
            .push((ActivePane::Disassembly, NavigationTarget::Address(0x1000)));

        perform_jump_to_address_no_history(&app_state, &mut view, Addr(0x1001));
        assert_eq!(view.active_pane, ActivePane::Disassembly);
        assert_eq!(view.cursor_index, expected_idx);
        assert_eq!(
            view.navigation_history,
            vec![(ActivePane::Disassembly, NavigationTarget::Address(0x1000))]
        );

        // Missing address leaves cursor and history untouched.
        perform_jump_to_address_no_history(&app_state, &mut view, Addr(0xCAFE));
        assert_eq!(view.cursor_index, expected_idx);
        assert_eq!(view.navigation_history.len(), 1);
    }

    #[test]
    fn core_nav_07_resolve_jump_content_line_index_skips_zero_byte_body_headers() {
        use crate::disassembler::DisassemblyLine;
        use crate::state::BlockType;

        // Case 1: consecutive zero-byte headers ({splitter}, scope `{`) followed by instruction body row.
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xA9, 0x00, 0x60];
        app_state.block_types = vec![BlockType::Code; 3];
        app_state.disassembly = vec![
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "; enum Header".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: Vec::new(),
                mnemonic: "{splitter}".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: Vec::new(),
                mnemonic: ".block".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: Some("sub_1000".to_string()),
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: vec![0xA9, 0x00],
                mnemonic: "lda".to_string(),
                operand: "#$00".to_string(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];
        assert_eq!(
            resolve_jump_content_line_index(&app_state, Addr(0x1000)),
            Some(3)
        );

        // Case 2: consecutive zero-byte headers followed by a zero-byte content row at `$1000`
        // advances via `advance_to_content_line` without sliding into `$1001`.
        let mut header_only_state = AppState::new();
        header_only_state.origin = Addr(0x1000);
        header_only_state.raw_data = vec![0xEA, 0x60];
        header_only_state.block_types = vec![BlockType::Code; 2];
        header_only_state.disassembly = vec![
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: Vec::new(),
                mnemonic: "{splitter}".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: Vec::new(),
                mnemonic: ".block".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: Some("sub_1000".to_string()),
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: Vec::new(),
                mnemonic: "sub_content".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1001),
                bytes: vec![0x60],
                mnemonic: "rts".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];
        assert_eq!(
            resolve_jump_content_line_index(&header_only_state, Addr(0x1000)),
            Some(2)
        );
    }

    #[test]
    fn core_nav_real_disassemble_zero_collision_and_wraparound() {
        use crate::state::{BlockType, EnumDefinition, Label, LabelKind, LabelType};
        use std::collections::BTreeMap;

        let add_colors_enum = |state: &mut AppState, enum_addr: Addr| {
            let mut variants = BTreeMap::new();
            variants.insert(0, "BLACK".to_string());
            state.enums.insert(
                "Colors".to_string(),
                EnumDefinition {
                    name: "Colors".to_string(),
                    description: None,
                    variants,
                    source_file: None,
                },
            );
            state.annotations.update(enum_addr, |e| {
                e.enum_usage = Some("Colors".to_string());
            });
        };

        // CORE-NAV-01: Case 1 (`origin > $0000`, external equate at `$0000`, `all_labels = true`, active enum preamble).
        let mut case1 = AppState::new();
        case1.origin = Addr(0x0801);
        case1.settings.all_labels = true;
        case1.raw_data = vec![0xA9, 0x00]; // LDA #Colors.BLACK
        case1.block_types = vec![BlockType::Code; 2];
        add_colors_enum(&mut case1, Addr(0x0801));
        case1.labels.insert(
            Addr::ZERO,
            vec![Label {
                name: "zp_zero".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ZeroPageField,
            }],
        );
        case1.disassemble();

        let eq_idx = resolve_jump_content_line_index(&case1, Addr::ZERO)
            .expect("$0000 external equate index");
        assert_eq!(
            case1.disassembly[eq_idx].external_label_address,
            Some(Addr::ZERO)
        );
        assert!(case1.disassembly[eq_idx].mnemonic.contains("zp_zero"));
        let mut view1 = CoreViewState::new();
        view1.active_pane = ActivePane::Disassembly;
        view1.cursor_index = case1.disassembly.len() - 1;
        perform_jump_to_address(&case1, &mut view1, Addr::ZERO);
        assert_eq!(view1.cursor_index, eq_idx);
        assert_eq!(view1.navigation_history.len(), 1);

        // CORE-NAV-02: Case 1b (`origin > $0000`, `.word $0000` with NO label at `$0000`, `all_labels = true`, enum preamble active).
        let mut case1b = AppState::new();
        case1b.origin = Addr(0x0801);
        case1b.settings.all_labels = true;
        case1b.raw_data = vec![0x00, 0x00, 0xA9, 0x00];
        case1b.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
        ];
        add_colors_enum(&mut case1b, Addr(0x0803));
        case1b.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        case1b.disassemble();

        assert_eq!(resolve_jump_content_line_index(&case1b, Addr::ZERO), None);
        let word_line = case1b
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x0801) && !l.bytes.is_empty())
            .expect(".word $0000 line");
        let word_targets = case1b.resolve_disassembly_jump_targets(word_line);
        assert_eq!(word_targets.len(), 1);
        assert_eq!(word_targets[0].target_address, Addr::ZERO);
        assert_eq!(
            word_targets[0].target_preview,
            Some("(out of range)".to_string())
        );
        let mut view1b = CoreViewState::new();
        view1b.active_pane = ActivePane::Disassembly;
        view1b.cursor_index = case1b.disassembly.len() - 1;
        let orig_cursor = view1b.cursor_index;
        perform_jump_to_address(&case1b, &mut view1b, Addr::ZERO);
        assert_eq!(view1b.cursor_index, orig_cursor);
        assert!(view1b.navigation_history.is_empty());
        assert!(
            view1b
                .status_message
                .as_deref()
                .is_some_and(|msg| msg.contains("out of range"))
        );

        // CORE-NAV-03: Case 2 (`origin == $0000`, collapsed `(0, 2)`, `all_labels = true`, external equate + enum preamble).
        let mut case2 = AppState::new();
        case2.origin = Addr::ZERO;
        case2.settings.all_labels = true;
        case2.raw_data = vec![0xEA, 0xEA, 0xEA, 0xA9, 0x00, 0x60];
        case2.block_types = vec![BlockType::Code; 6];
        case2.collapsed_blocks.push((0, 2));
        add_colors_enum(&mut case2, Addr(0x0003));
        case2.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        case2.disassemble();

        let collapsed_idx = resolve_jump_content_line_index(&case2, Addr::ZERO)
            .expect("collapsed $0000 summary index");
        assert!(case2.disassembly[collapsed_idx].is_collapsed);
        assert_eq!(case2.disassembly[collapsed_idx].address, Addr::ZERO);
        assert_eq!(
            resolve_jump_content_line_index(&case2, Addr(0x0002)),
            Some(collapsed_idx)
        );
        let mut view2 = CoreViewState::new();
        perform_jump_to_address(&case2, &mut view2, Addr(0x0002));
        assert_eq!(view2.cursor_index, collapsed_idx);

        // CORE-NAV-04: Case 3 (`origin == $0000`, non-collapsed `$0000` body + `{splitter}` / scope `{` + enum preamble).
        let mut case3 = AppState::new();
        case3.origin = Addr::ZERO;
        case3.settings.all_labels = true;
        case3.raw_data = vec![0xAD, 0x20, 0xD0, 0xA9, 0x00, 0x60];
        case3.block_types = vec![BlockType::Code; 6];
        case3.splitters.insert(Addr::ZERO);
        case3.annotations.update(Addr::ZERO, |e| {
            e.scope = Some(Addr(0x0005));
        });
        add_colors_enum(&mut case3, Addr(0x0003));
        case3.labels.insert(
            Addr::ZERO,
            vec![Label {
                name: "zp_start".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        case3.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        case3.disassemble();

        let body_idx =
            resolve_jump_content_line_index(&case3, Addr::ZERO).expect("body row at $0000");
        assert!(!case3.disassembly[body_idx].bytes.is_empty());
        assert_eq!(case3.disassembly[body_idx].address, Addr::ZERO);
        assert!(case3.disassembly[body_idx].external_label_address.is_none());

        // CORE-NAV-07: Wraparound span `$FFFE..=$FFFF` (`origin = $FFFC`, `raw_data.len() = 4`).
        let mut wrap = AppState::new();
        wrap.origin = Addr(0xFFFC);
        wrap.raw_data = vec![0xEA, 0xEA, 0x00, 0x08];
        wrap.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Address,
            BlockType::Address,
        ];
        wrap.disassemble();

        let word_fffe_idx =
            resolve_jump_content_line_index(&wrap, Addr(0xFFFF)).expect("$FFFF inside $FFFE .word");
        assert_eq!(wrap.disassembly[word_fffe_idx].address, Addr(0xFFFE));
        assert_eq!(wrap.disassembly[word_fffe_idx].bytes, vec![0x00, 0x08]);
        assert_eq!(resolve_jump_content_line_index(&wrap, Addr::ZERO), None);
    }

    #[test]
    fn jump_target_index_matches_resolve_jump_content_line_index_across_all_edge_cases() {
        use crate::disassembler::DisassemblyLine;
        use crate::state::{BlockType, EnumDefinition, Label, LabelKind, LabelType};
        use std::collections::BTreeMap;

        let assert_index_matches = |state: &AppState, probes: &[Addr]| {
            let index = JumpTargetIndex::new(state);
            for &addr in probes {
                assert_eq!(
                    index.resolve_content_line_index(state, addr),
                    resolve_jump_content_line_index(state, addr),
                    "JumpTargetIndex mismatch at ${:04X}",
                    addr.0
                );
            }
        };

        let add_colors_enum = |state: &mut AppState, enum_addr: Addr| {
            let mut variants = BTreeMap::new();
            variants.insert(0, "BLACK".to_string());
            state.enums.insert(
                "Colors".to_string(),
                EnumDefinition {
                    name: "Colors".to_string(),
                    description: None,
                    variants,
                    source_file: None,
                },
            );
            state.annotations.update(enum_addr, |e| {
                e.enum_usage = Some("Colors".to_string());
            });
        };

        // 1. External `$0000` equate + enum preamble + non-aligned gap probes.
        let mut ext_zero = AppState::new();
        ext_zero.origin = Addr(0x0801);
        ext_zero.settings.all_labels = true;
        ext_zero.raw_data = vec![0xAD, 0x20, 0xD0, 0xA9, 0x00];
        ext_zero.block_types = vec![BlockType::Code; 5];
        add_colors_enum(&mut ext_zero, Addr(0x0804));
        ext_zero.labels.insert(
            Addr::ZERO,
            vec![Label {
                name: "zp_zero".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ZeroPageField,
            }],
        );
        ext_zero.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        ext_zero.disassemble();
        assert_index_matches(
            &ext_zero,
            &[
                Addr::ZERO,
                Addr(0x0400),
                Addr(0x0800),
                Addr(0x0801),
                Addr(0x0802),
                Addr(0x0803),
                Addr(0x0804),
                Addr(0x0805),
                Addr(0x0900),
                Addr(0xD020),
                Addr(0xFFFF),
            ],
        );

        // 2. Origin `$0000` with collapsed `(0, 2)` + splitter/scope headers at `$0000` and `$0003`.
        let mut collapsed = AppState::new();
        collapsed.origin = Addr::ZERO;
        collapsed.settings.all_labels = true;
        collapsed.raw_data = vec![0xEA, 0xEA, 0xEA, 0xAD, 0x20, 0xD0];
        collapsed.block_types = vec![BlockType::Code; 6];
        collapsed.collapsed_blocks.push((0, 2));
        collapsed.splitters.insert(Addr(0x0003));
        collapsed.annotations.update(Addr(0x0003), |e| {
            e.scope = Some(Addr(0x0005));
        });
        add_colors_enum(&mut collapsed, Addr(0x0003));
        collapsed.disassemble();
        assert_index_matches(
            &collapsed,
            &[
                Addr::ZERO,
                Addr(0x0001),
                Addr(0x0002),
                Addr(0x0003),
                Addr(0x0004),
                Addr(0x0005),
                Addr(0x0006),
                Addr(0xD020),
            ],
        );

        // 3. Wraparound across `$FFFF -> $0000` (tests non-monotonic span fallback + `$FFFF`).
        let mut wrap = AppState::new();
        wrap.origin = Addr(0xFFFE);
        wrap.raw_data = vec![0x4C, 0x00, 0x08];
        wrap.block_types = vec![BlockType::Code; 3];
        wrap.disassemble();
        assert_index_matches(
            &wrap,
            &[
                Addr::ZERO,
                Addr(0x0001),
                Addr(0x0800),
                Addr(0xFFFD),
                Addr(0xFFFE),
                Addr(0xFFFF),
            ],
        );

        // 4. Synthetic non-monotonic zero-byte & wrapped-span disassembly covering fallback passes.
        let make_line = |address: Addr, bytes: Vec<u8>, mnemonic: &str| DisassemblyLine {
            address,
            bytes,
            mnemonic: mnemonic.to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let mut synthetic = AppState::new();
        synthetic.origin = Addr(0x1000);
        synthetic.raw_data = vec![0xEA; 0x20];
        synthetic.disassembly = vec![
            make_line(Addr::ZERO, Vec::new(), "; banner"),
            make_line(Addr(0x1010), Vec::new(), "; zero-byte header"),
            make_line(Addr(0x1008), vec![0xEA, 0xEA], "nop"),
            make_line(Addr(0x1010), vec![0xEA], "nop"),
            make_line(Addr(0xFFFE), vec![0x01, 0x02, 0x03], ".byte"),
        ];
        assert_index_matches(
            &synthetic,
            &[
                Addr::ZERO,
                Addr(0x0001),
                Addr(0x1000),
                Addr(0x1005),
                Addr(0x1008),
                Addr(0x1009),
                Addr(0x100A),
                Addr(0x1010),
                Addr(0x1015),
                Addr(0xFFFE),
                Addr(0xFFFF),
            ],
        );
    }

    #[test]
    fn test_clamped_item_addr_range_boundaries_and_bitmap_gaps() {
        // Sprites at `origin = $0000`: indices 37..=38 correspond to `$0940..=$09BF`.
        let mut zero_sprites = AppState::new();
        zero_sprites.origin = Addr(0x0000);
        zero_sprites.raw_data = vec![0xEA; 4096];
        let sprites0 = AlignedGridRange::sprites(&zero_sprites);
        assert_eq!(
            sprites0.clamped_item_addr_range(37, 37),
            Some((Addr(0x0940), Addr(0x097F)))
        );
        assert_eq!(
            sprites0.clamped_item_addr_range(37, 38),
            Some((Addr(0x0940), Addr(0x09BF)))
        );
        assert_eq!(
            sprites0.clamped_item_addr_range(38, 37),
            Some((Addr(0x0940), Addr(0x09BF)))
        );

        // Sprites at `origin = $0800`: indices 37..=38 correspond to `$1140..=$11BF`.
        let mut state = AppState::new();
        state.origin = Addr(0x0800);
        state.raw_data = vec![0xEA; 4096];
        let sprites = AlignedGridRange::sprites(&state);
        assert_eq!(
            sprites.clamped_item_addr_range(37, 37),
            Some((Addr(0x1140), Addr(0x117F)))
        );
        assert_eq!(
            sprites.clamped_item_addr_range(37, 38),
            Some((Addr(0x1140), Addr(0x11BF)))
        );
        assert_eq!(
            sprites.clamped_item_addr_range(38, 37),
            Some((Addr(0x1140), Addr(0x11BF)))
        );

        // Pre-origin alignment padding only (`origin = $0809`, charset slot 0 is `$0800..=$0807`).
        let mut padding_only = AppState::new();
        padding_only.origin = Addr(0x0809);
        padding_only.raw_data = vec![0xEA; 32];
        assert_eq!(
            AlignedGridRange::charset(&padding_only).clamped_item_addr_range(0, 0),
            None
        );

        // Partial overlap (`origin = $0801`, sprite slot 0 spans `$0800..=$083F` -> `$0801..=$083F`).
        let mut partial = AppState::new();
        partial.origin = Addr(0x0801);
        partial.raw_data = vec![0xEA; 128];
        assert_eq!(
            AlignedGridRange::sprites(&partial).clamped_item_addr_range(0, 0),
            Some((Addr(0x0801), Addr(0x083F)))
        );

        // Bitmap single vs. multi-bitmap 192-byte gap.
        let mut bitmap_state = AppState::new();
        bitmap_state.origin = Addr(0x0000);
        bitmap_state.raw_data = vec![0xEA; 16384];
        let bitmap = AlignedGridRange::bitmap(&bitmap_state);
        assert_eq!(
            bitmap.clamped_item_addr_range(0, 0),
            Some((Addr(0x0000), Addr(0x1F3F)))
        );
        assert_eq!(
            bitmap.clamped_item_addr_range(0, 1),
            Some((Addr(0x0000), Addr(0x3F3F)))
        );
        assert_eq!(
            bitmap.clamped_item_addr_range(1, 0),
            Some((Addr(0x0000), Addr(0x3F3F)))
        );

        // Bitmap origin inside trailing 192-byte tail (`$1F40..=$1FFF`).
        let mut tail_bitmap = AppState::new();
        tail_bitmap.origin = Addr(0x1F50);
        tail_bitmap.raw_data = vec![0xEA; 256];
        assert_eq!(
            AlignedGridRange::bitmap(&tail_bitmap).clamped_item_addr_range(0, 0),
            None
        );

        // Wrapped binary truncation at `$FFFF`.
        let mut wrap = AppState::new();
        wrap.origin = Addr(0xFFFE);
        wrap.raw_data = vec![0xEA; 4];
        assert_eq!(
            AlignedGridRange::sprites(&wrap).clamped_item_addr_range(0, 0),
            Some((Addr(0xFFFE), Addr(0xFFFF)))
        );

        // Empty binary (both origin $0000 and unaligned non-zero origin $0801).
        let empty = AppState::new();
        assert_eq!(
            AlignedGridRange::sprites(&empty).clamped_item_addr_range(0, 0),
            None
        );
        assert_eq!(AlignedGridRange::sprites(&empty).item_count(), 0);

        let mut empty_unaligned = AppState::new();
        empty_unaligned.origin = Addr(0x0801);
        assert_eq!(AlignedGridRange::sprites(&empty_unaligned).item_count(), 0);
        assert_eq!(AlignedGridRange::charset(&empty_unaligned).item_count(), 0);
        assert_eq!(AlignedGridRange::bitmap(&empty_unaligned).item_count(), 0);
        assert_eq!(
            AlignedGridRange::hex_dump(&empty_unaligned, RightPane::HexDump16).item_count(),
            0
        );
    }

    #[test]
    fn test_hex_byte_offset_range_and_col_clamping() {
        let mut state = AppState::new();
        state.origin = Addr(0x0801);
        state.raw_data = vec![0xEA; 32];

        let hex16 = AlignedGridRange::hex_dump(&state, RightPane::HexDump16);
        assert_eq!(hex16.hex_byte_offset_range(0, 0, None), None);
        assert_eq!(hex16.hex_byte_offset_range(1, 2, Some((0, 0))), Some(0..18));
        assert_eq!(hex16.hex_byte_offset_range(0, 2, Some((1, 0))), Some(1..16));

        let hex8 = AlignedGridRange::hex_dump(&state, RightPane::HexDump8);
        assert_eq!(hex8.hex_byte_offset_range(0, 15, None), Some(6..7));
        assert_eq!(
            hex8.hex_byte_offset_range(1, 15, Some((0, 15))),
            Some(6..15)
        );

        let empty = AppState::new();
        assert_eq!(
            AlignedGridRange::hex_dump(&empty, RightPane::HexDump16)
                .hex_byte_offset_range(0, 0, None),
            None
        );
    }

    #[test]
    fn test_active_pane_item_addr_range_and_byte_offset_range() {
        let mut state = AppState::new();
        state.origin = Addr(0x2000);
        state.raw_data = vec![0xEA; 0x4200];

        let mut view = CoreViewState::new();
        for pane in [
            ActivePane::Disassembly,
            ActivePane::HexDump,
            ActivePane::Blocks,
            ActivePane::Debugger,
        ] {
            view.active_pane = pane;
            assert_eq!(
                AlignedGridRange::active_pane_item_addr_range(&state, &view),
                None
            );
        }

        view.active_pane = ActivePane::Sprites;
        view.sprites_cursor_index = 2;
        assert_eq!(
            AlignedGridRange::active_pane_item_addr_range(&state, &view),
            Some((Addr(0x2080), Addr(0x20BF)))
        );
        view.selection_start = Some(0);
        assert_eq!(
            AlignedGridRange::active_pane_item_addr_range(&state, &view),
            None
        );
        view.selection_start = None;

        view.active_pane = ActivePane::Charset;
        view.charset_cursor_index = 3;
        assert_eq!(
            AlignedGridRange::active_pane_item_addr_range(&state, &view),
            Some((Addr(0x2018), Addr(0x201F)))
        );

        view.active_pane = ActivePane::Bitmap;
        view.bitmap_cursor_index = 0;
        assert_eq!(
            AlignedGridRange::active_pane_item_addr_range(&state, &view),
            Some((Addr(0x2000), Addr(0x3F3F)))
        );

        // `active_pane_byte_offset_range` checks.
        view.active_pane = ActivePane::Sprites;
        view.sprites_selection_start = Some(2);
        view.sprites_cursor_index = 1;
        assert_eq!(active_pane_byte_offset_range(&state, &view), Some(64..192));

        view.active_pane = ActivePane::Charset;
        view.charset_selection_start = None;
        view.charset_cursor_index = 4;
        assert_eq!(active_pane_byte_offset_range(&state, &view), Some(32..40));

        view.active_pane = ActivePane::Bitmap;
        view.bitmap_selection_start = Some(0);
        view.bitmap_cursor_index = 1;
        assert_eq!(
            active_pane_byte_offset_range(&state, &view),
            Some(0..0x3F40)
        );

        view.active_pane = ActivePane::HexDump;
        view.right_pane = RightPane::HexDump8;
        view.hex_cursor_index = 1;
        view.hex_col_cursor = 15;
        view.hex_selection_start = Some(0);
        view.hex_selection_start_col = 12;
        assert_eq!(active_pane_byte_offset_range(&state, &view), Some(7..16));

        for pane in [
            ActivePane::Disassembly,
            ActivePane::Blocks,
            ActivePane::Debugger,
        ] {
            view.active_pane = pane;
            assert_eq!(active_pane_byte_offset_range(&state, &view), None);
        }

        let mut padding_only = AppState::new();
        padding_only.origin = Addr(0x0809);
        padding_only.raw_data = vec![0xEA; 32];
        let mut padding_view = CoreViewState::new();
        padding_view.active_pane = ActivePane::Charset;
        padding_view.charset_cursor_index = 0;
        assert_eq!(
            active_pane_byte_offset_range(&padding_only, &padding_view),
            None
        );
    }

    #[test]
    fn test_line_byte_addr_span_excludes_non_body_and_handles_wrapped_collapsed() {
        use crate::state::Assembler;

        // Setup A: origin $0000 with preamble, external equate $D020, scope delimiter `}`, and LDA $D020.
        let mut state = AppState::new();
        state.origin = Addr(0x0000);
        state.raw_data = vec![0xAD, 0x20, 0xD0, 0xEA];
        state.block_types = vec![BlockType::Code; 4];
        state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        state.settings.all_labels = true;
        state.settings.assembler = Assembler::Kick;
        state.annotations.update(Addr(0x0000), |entry| {
            entry.scope = Some(Addr(0x0003));
        });
        state.disassemble();

        let zero_non_body_lines: Vec<_> = state
            .disassembly
            .iter()
            .filter(|line| {
                line.address == Addr(0x0000) && line.bytes.is_empty() && !line.is_collapsed
            })
            .collect();
        assert!(
            !zero_non_body_lines.is_empty(),
            "expected at least one non-body header/banner row at $0000"
        );
        for line in zero_non_body_lines {
            assert_eq!(line_byte_addr_span(&state, line), None);
        }

        let external_line = state
            .disassembly
            .iter()
            .find(|line| line.external_label_address == Some(Addr(0xD020)))
            .expect("external equate $D020 line");
        assert_eq!(line_byte_addr_span(&state, external_line), None);

        let lda_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x0000) && !l.bytes.is_empty())
            .expect("LDA $D020 line");
        assert_eq!(
            line_byte_addr_span(&state, lda_line),
            Some((0x0000, 0x0002))
        );

        let scope_end_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x0004) && l.mnemonic == "}")
            .expect("closing scope delimiter `}` at $0004");
        assert_eq!(line_byte_addr_span(&state, scope_end_line), None);

        let synthetic_overlong = DisassemblyLine {
            address: Addr(0x0002),
            bytes: vec![0x01; 10],
            mnemonic: ".byte".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            line_byte_addr_span(&state, &synthetic_overlong),
            Some((0x0002, 0x0003))
        );

        // Setup B: wrapped binary collapsed block `$FFFE..=$0001`.
        let mut wrap = AppState::new();
        wrap.origin = Addr(0xFFFE);
        wrap.raw_data = vec![0xEA; 4];
        wrap.block_types = vec![BlockType::Undefined; 4];
        wrap.collapsed_blocks = vec![(0, 3)];
        wrap.disassemble();

        let collapsed_line = wrap
            .disassembly
            .iter()
            .find(|l| l.is_collapsed && l.address == Addr(0xFFFE))
            .expect("collapsed line at $FFFE");
        assert_eq!(line_byte_offset_range(&wrap, collapsed_line), Some(0..4));
        assert_eq!(
            line_byte_addr_span(&wrap, collapsed_line),
            Some((0xFFFE, 0x10001))
        );
    }
}
