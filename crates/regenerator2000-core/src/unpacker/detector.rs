//! Decompressed memory boundary and entry point detection heuristics.

use super::bus::{is_basic_rom_mapped, is_io_mapped};

/// Detects the modified memory range using the write-tracking bitmap and
/// a pre-emulation snapshot.
///
/// `_ret_addr` is reserved for return-address boundary checking (typically `$0800`).
///
/// Uses a hybrid approach:
/// - **Start address**: determined by the `written` bitmap (catches all writes).
/// - **End address**: determined by the snapshot diff, then extended forward
///   through any bytes that were `written` but match the snapshot (trailing
///   zero-fills). This excludes depacker tables written past the output.
///
/// Returns `(start_addr, end_addr)` inclusive, or `None` if nothing was written.
#[must_use]
pub fn detect_output_range(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    _ret_addr: u16,
    load_addr: u16,
    system: &crate::state::types::System,
) -> Option<(u16, u16)> {
    let scan_start = (load_addr as usize).min(system.ram_start() as usize);

    // Cascading scans with progressively wider boundaries.
    // Each level is only tried if the previous scan's detected end is near
    // its ceiling (within 256 bytes), indicating the output continues past
    // that boundary. This keeps the scan range tight so the trim heuristics
    // work correctly with workspaces that fill high memory.
    //
    // Level 1: $0800..$9FFF — typical program area
    // Level 2: $0800..$CFFF — includes BASIC ROM area (all-RAM mode)
    // Level 3: $0800..$FFFF — includes I/O + Kernal ROM area (full RAM)
    //
    // In-place depackers (e.g. TinyCrunch) write to two disjoint regions:
    // a lower region (e.g. $0801-$7949) and a high region (e.g. $D000-$FFFD),
    // leaving a gap of unchanged bytes in the middle. The gap means
    // `untrimmed_end` stops early and `near_ceiling` is false. To handle
    // this we also escalate when written+diffed bytes exist above the current
    // boundary.
    let boundaries = system.memory_boundaries();

    for (i, &boundary) in boundaries.iter().enumerate() {
        if let Some((start, end, trimmed_end, has_diff)) =
            scan_hybrid_range(mem, snapshot, written, scan_start, boundary, false, system)
        {
            let is_last = i == boundaries.len() - 1;
            let near_ceiling = has_diff && (trimmed_end as usize) + 256 >= boundary;

            // Also escalate when there are written+diffed bytes above the
            // current boundary — in-place depackers write to a disjoint high
            // region while leaving unchanged data in between.
            let io_mapped = is_io_mapped(mem, system);
            let next_upper = if !is_last {
                boundaries[i + 1]
            } else {
                boundary
            };
            let has_output_above = !is_last
                && (trimmed_end as usize) >= boundary.saturating_sub(0x2000)
                && (boundary + 1..=next_upper)
                    .filter(|&addr| {
                        let is_io = system.is_in_io(addr as u16) && io_mapped;
                        let diff = written.get(addr).copied().unwrap_or(false)
                            || mem.get(addr).copied().unwrap_or(0)
                                != snapshot.get(addr).copied().unwrap_or(0);
                        !is_io && diff
                    })
                    .count()
                    >= 4;

            if is_last || (!near_ceiling && !has_output_above) {
                return Some((start, end));
            }
        }
    }

    let mid_boundary = if boundaries.len() > 1 {
        boundaries[1]
    } else {
        boundaries[0]
    };
    let basic_mapped = is_basic_rom_mapped(mem, system);
    if (boundaries[0] + 1..=mid_boundary).any(|a| {
        let in_rom = system.is_in_basic_rom(a as u16) && basic_mapped;
        !in_rom
            && written.get(a).copied().unwrap_or(false)
            && mem.get(a).copied().unwrap_or(0) != snapshot.get(a).copied().unwrap_or(0)
    }) && let Some((s, e, _, _)) = scan_hybrid_range(
        mem,
        snapshot,
        written,
        scan_start,
        mid_boundary,
        false,
        system,
    ) {
        return Some((s, e));
    }

    // Fallback: scan $E000-$FFFF for packers that decompress only into
    // the Kernal ROM area.
    scan_hybrid_range(mem, snapshot, written, 0xE000, 0xFFFF, false, system)
        .map(|(s, e, _, _)| (s, e))
}

/// Scans a range using a hybrid of the `written` bitmap and snapshot diff.
///
/// - **Start**: first byte in the `written` bitmap.
/// - **End**: last byte where `mem != snapshot`, trimmed of any small trailing
///   diff clusters (depacker tables) separated by matching bytes, then extended
///   through written zero-fills (`mem == snapshot`).
///
/// If `skip_trim` is `true`, the `trim_trailing_clusters` heuristic is bypassed
/// and the last diff byte (or last written byte if no diff) is used directly.
/// This is used when escalating due to an in-place depacker output gap.
#[must_use]
pub fn scan_hybrid_range(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    start: usize,
    end: usize,
    skip_trim: bool,
    system: &crate::state::types::System,
) -> Option<(u16, u16, u16, bool)> {
    let upper = end
        .min(written.len().saturating_sub(1))
        .min(mem.len().saturating_sub(1))
        .min(snapshot.len().saturating_sub(1));

    // Find the first written byte or first diff byte in RAM (start..=upper)
    let mut first_written = None;
    for addr in start..=upper {
        if written.get(addr).copied().unwrap_or(false) || mem[addr] != snapshot[addr] {
            first_written = Some(addr);
            break;
        }
    }
    let mut first = first_written?;
    let ram_start = system.ram_start() as usize;
    if first < ram_start {
        let mut diffs_below = 0;
        let mut gap_before_ram = 0;
        for a in first..ram_start {
            if mem[a] != snapshot[a] || written.get(a).copied().unwrap_or(false) {
                diffs_below += 1;
            } else {
                gap_before_ram += 1;
            }
        }
        if diffs_below < 64 && gap_before_ram > 128 {
            for a in ram_start..=upper {
                if written.get(a).copied().unwrap_or(false) || mem[a] != snapshot[a] {
                    first = a;
                    break;
                }
            }
        }
    }

    // Find all diff bytes and identify the end of the "main" diff block
    // by trimming small trailing clusters separated by non-diff gaps.
    let mut last_diff = None;
    for addr in start..=upper {
        if written.get(addr).copied().unwrap_or(false) || mem[addr] != snapshot[addr] {
            last_diff = Some(addr);
        }
    }

    let diff_end = last_diff?;

    // Walk backward from diff_end to detect and trim small trailing clusters.
    // Only apply trimming when the diff extends near the scan boundary (within
    // 128 bytes). A clean gap between diff_end and the boundary means the
    // output ends naturally with no depacker workspace to separate — trimming
    // would only produce false positives on natural gaps in program data.
    // Skip trimming entirely when the caller signals an in-place depacker gap.
    let trimmed_end = if !skip_trim {
        trim_trailing_clusters(mem, snapshot, written, first, diff_end)
    } else {
        diff_end
    };

    // Extend past the trimmed end through written bytes that match the snapshot
    // (trailing zero-fills that are part of the real output).
    let mut extended_end = trimmed_end;
    let max_extend = upper.min(trimmed_end + 512);
    for addr in (trimmed_end + 1)..=max_extend {
        if written[addr] && mem[addr] == snapshot[addr] {
            extended_end = addr;
        } else {
            break;
        }
    }

    Some((first as u16, extended_end as u16, trimmed_end as u16, true))
}

/// After the depacker transfers control to `ret_addr`, some packers execute
/// a brief init/bootstrap stub before jumping to the real program entry.
/// This function scans the **pre-decompression snapshot** for a `JMP $xxxx`
/// instruction that targets a plausible entry point (≥ `ret_addr + 0x100`).
///
/// The Dali packer, for example, stores `JMP $1100` at $090A in its packed
/// binary. Before decompression, the snapshot preserves this instruction even
/// though it gets overwritten by decompressed data.
///
/// Returns the discovered entry point, or `None` if none is found.
#[must_use]
pub fn find_entry_in_snapshot(
    snapshot: &[u8],
    load_addr: u16,
    load_size: usize,
    ret_addr: u16,
) -> Option<u16> {
    // Minimum plausible entry point: must be well past the depacker code
    // (ret_addr + 0x300 skips over common init stubs in the first 3 pages).
    let min_entry = ret_addr.saturating_add(0x300);
    // Only scan in the depacker's own code region: [ret_addr, ret_addr+0x400].
    // The depacker exit JMP is typically within the first few pages of the
    // loaded binary. We avoid scanning deeper to prevent false positives from
    // JMP instructions in the init/bootstrap code.
    let scan_start = ret_addr as usize;
    let scan_end = (ret_addr as usize)
        .saturating_add(0x400)
        .min(load_addr as usize + load_size)
        .min(snapshot.len().saturating_sub(2));

    // Scan for JMP $xxxx (opcode $4C) targeting a plausible entry address.
    // Use the LOWEST valid target — the Dali packer stores JMP $1100 as the
    // first JMP-to-entry in its depacker code.
    let mut best: Option<u16> = None;
    for i in scan_start..scan_end {
        if snapshot[i] == 0x4C {
            let lo = snapshot[i + 1];
            let hi = snapshot[i + 2];
            let target = u16::from_le_bytes([lo, hi]);
            // Target must be a plausible entry: above min_entry and in RAM (<$8000)
            if target >= min_entry && target < 0x8000 {
                // Prefer the LOWEST target — closest to the decompressed start
                match best {
                    None => best = Some(target),
                    Some(prev) if target < prev => best = Some(target),
                    _ => {}
                }
            }
        }
    }
    best
}

/// Trims trailing depacker workspace from the detected diff range.
///
/// Walks backward from `end` through the diff range, examining each gap
/// (run of same-as-snapshot bytes). Trims at the first gap where either:
///
/// 1. The trailing diff cluster is tiny (< 16 bytes) — handles depacker
///    tails like PUCrunch's 10-byte cluster.
/// 2. The trailing range is > 128 bytes AND proportionally small (< 2% of
///    the main region) — handles large depacker workspaces like ERA's
///    hundreds of bytes.
///
/// Stops scanning at 95% of the range to avoid false positives deep inside
/// the real output data.
#[must_use]
pub fn trim_trailing_clusters(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    start: usize,
    end: usize,
) -> usize {
    if end <= start {
        return end;
    }

    let scan_floor = start;
    let mut pos = end;
    let mut curr_cluster_end = end;
    let mut best_trim_pos: Option<usize> = None;

    while pos > scan_floor {
        // Walk backward through diff bytes
        while pos > scan_floor && mem.get(pos) != snapshot.get(pos) {
            pos -= 1;
        }

        if pos <= scan_floor {
            break;
        }

        // Found a matching byte — walk backward through the gap
        let gap_end = pos;
        while pos > start && mem.get(pos) == snapshot.get(pos) {
            pos -= 1;
        }

        // Count diff bytes in the cluster immediately following the gap
        let cluster_diffs: usize = ((gap_end + 1)..=curr_cluster_end)
            .filter(|&a| {
                written.get(a).copied().unwrap_or(false)
                    || mem.get(a) != snapshot.get(a)
                    || mem.get(a).copied().unwrap_or(0) != 0
            })
            .count();

        let gap_len = gap_end - (pos + 1) + 1;

        // Check 1: tiny trailing cluster (< 16 diff bytes) with a gap (>= 4 bytes).
        if gap_len >= 4 && cluster_diffs < 16 {
            return pos;
        }

        // Check 2: gap (>= 2 bytes) separating high workspace cluster from main decompressed payload.
        let main_len = (pos + 1).saturating_sub(start);
        if gap_end < 0xF000
            && (128..4096).contains(&gap_len)
            && main_len > 0
            && (cluster_diffs == 0 || cluster_diffs <= 512)
        {
            best_trim_pos = Some(pos);
            curr_cluster_end = pos;
        }
    }

    best_trim_pos.unwrap_or(end)
}
