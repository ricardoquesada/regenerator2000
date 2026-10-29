//! Decompressed memory boundary and entry point detection heuristics.

use super::bus::{is_basic_rom_mapped, is_io_mapped};

/// Diff bytes within this distance of a scan ceiling indicate the output continues upward.
const CEILING_PROXIMITY: usize = 256;

/// Distance below a scan ceiling within which `trimmed_end` must lie to escalate
/// for a disjoint high-memory output region (e.g. TinyCrunch in-place depacking).
const HIGH_REGION_WINDOW: usize = 0x2000;

/// Minimum written or diffed non-I/O bytes above a scan ceiling that indicate a
/// disjoint high-memory output region.
const MIN_HIGH_REGION_BYTES: usize = 4;

/// Start of the Kernal ROM fallback scan window (`$E000`–`$FFFF`) when no lower
/// memory boundary scan detects output.
const KERNAL_FALLBACK_START: usize = 0xE000;

/// End of the Kernal ROM fallback scan window (`$E000`–`$FFFF`).
const KERNAL_FALLBACK_END: usize = 0xFFFF;

/// Maximum number of pre-RAM written/diff bytes (`< 64`) before `system.ram_start()`
/// that are treated as low-memory depacker scratch rather than program start when
/// separated from `ram_start` by more than [`HYBRID_SCAN_GAP`] unchanged bytes.
const HYBRID_MIN_BYTES: usize = 64;

/// Minimum number of unchanged bytes (`> 128`) between pre-RAM writes and
/// `system.ram_start()` required to skip pre-RAM scratch and advance start to `ram_start`.
const HYBRID_SCAN_GAP: usize = 128;

/// Maximum forward extension through written-but-unchanged (trailing zero-fill) bytes.
const MAX_ZERO_FILL_EXTENSION: usize = 512;

/// Minimum offset above `ret_addr` (`+ 0x0300`, 3 pages) for a plausible `JMP $xxxx`
/// target in the pre-decompression snapshot, skipping initial bootstrap/depacker stubs.
const SNAPSHOT_ENTRY_MIN_OFFSET: u16 = 0x0300;

/// Maximum forward scan window from `ret_addr` (`0x0400` bytes, 4 pages) searched for a
/// depacker exit `JMP $xxxx` instruction in the pre-decompression snapshot.
const SNAPSHOT_ENTRY_SCAN_WINDOW: usize = 0x0400;

/// Number of operand bytes following a 6502 absolute `JMP $xxxx` opcode.
const JMP_ABS_OPERAND_LEN: usize = 2;

/// 6502 absolute `JMP $xxxx` opcode byte.
const OPCODE_JMP_ABS: u8 = 0x4C;

/// Exclusive upper address ceiling (`< $8000`) for a plausible entry point discovered
/// in the pre-decompression snapshot.
const SNAPSHOT_ENTRY_MAX_ADDR: u16 = 0x8000;

/// Minimum gap of unchanged bytes (`>= 4`) before a tiny trailing diff cluster
/// (such as PUCrunch's 10-byte tail) is trimmed.
const TRAILING_CLUSTER_GAP: usize = 4;

/// Exclusive upper bound (`< 16` diff bytes) for a tiny trailing diff cluster
/// trimmed at any gap of at least [`TRAILING_CLUSTER_GAP`] bytes.
const TRAILING_CLUSTER_MAX_SIZE: usize = 16;

/// Exclusive upper address bound (`< $F000`) on `gap_end` for trimming larger
/// high-memory depacker workspace clusters.
const WORKSPACE_CLUSTER_GAP_CEILING: usize = 0xF000;

/// Inclusive lower bound (`128` bytes) on the unchanged gap separating a high-memory
/// depacker workspace cluster from the main decompressed payload.
const WORKSPACE_CLUSTER_MIN_GAP: usize = 128;

/// Exclusive upper bound (`4096` bytes) on the unchanged gap separating a high-memory
/// depacker workspace cluster from the main decompressed payload.
const WORKSPACE_CLUSTER_MAX_GAP: usize = 4096;

/// Inclusive maximum diff byte count (`<= 512`) of a high-memory depacker workspace
/// cluster trimmed when separated by [`WORKSPACE_CLUSTER_MIN_GAP`]..[`WORKSPACE_CLUSTER_MAX_GAP`].
const WORKSPACE_CLUSTER_MAX_SIZE: usize = 512;

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
            let near_ceiling = has_diff && (trimmed_end as usize) + CEILING_PROXIMITY >= boundary;

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
                && (trimmed_end as usize) >= boundary.saturating_sub(HIGH_REGION_WINDOW)
                && (boundary + 1..=next_upper)
                    .filter(|&addr| {
                        let is_io = system.is_in_io(addr as u16) && io_mapped;
                        let diff = written.get(addr).copied().unwrap_or(false)
                            || mem.get(addr).copied().unwrap_or(0)
                                != snapshot.get(addr).copied().unwrap_or(0);
                        !is_io && diff
                    })
                    .count()
                    >= MIN_HIGH_REGION_BYTES;

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
    scan_hybrid_range(
        mem,
        snapshot,
        written,
        KERNAL_FALLBACK_START,
        KERNAL_FALLBACK_END,
        false,
        system,
    )
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
        if diffs_below < HYBRID_MIN_BYTES && gap_before_ram > HYBRID_SCAN_GAP {
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

    // Walk backward from diff_end to detect and trim small trailing clusters,
    // unless the caller signals that trimming should be skipped (`skip_trim`).
    let trimmed_end = if !skip_trim {
        trim_trailing_clusters(mem, snapshot, written, first, diff_end)
    } else {
        diff_end
    };

    // Extend past the trimmed end through written bytes that match the snapshot
    // (trailing zero-fills that are part of the real output).
    let mut extended_end = trimmed_end;
    let max_extend = upper.min(trimmed_end + MAX_ZERO_FILL_EXTENSION);
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
/// This function scans the **pre-decompression snapshot** within
/// `ret_addr..ret_addr + SNAPSHOT_ENTRY_SCAN_WINDOW` (`0x0400` bytes) for a
/// `JMP $xxxx` instruction that targets a plausible entry point
/// (`>= ret_addr + SNAPSHOT_ENTRY_MIN_OFFSET` (`0x0300`) and `< SNAPSHOT_ENTRY_MAX_ADDR` (`$8000`)).
///
/// The Dali packer, for example, stores `JMP $1100` at `$090A` in its packed
/// binary. Before decompression, the snapshot preserves this instruction even
/// though it gets overwritten by decompressed data.
///
/// Returns the lowest discovered entry point target, or `None` if none is found.
#[must_use]
pub fn find_entry_in_snapshot(
    snapshot: &[u8],
    load_addr: u16,
    load_size: usize,
    ret_addr: u16,
) -> Option<u16> {
    // Minimum plausible entry point: must be well past the depacker code
    // (ret_addr + 0x300 skips over common init stubs in the first 3 pages).
    let min_entry = ret_addr.saturating_add(SNAPSHOT_ENTRY_MIN_OFFSET);
    // Only scan in the depacker's own code region: [ret_addr, ret_addr+0x400].
    // The depacker exit JMP is typically within the first few pages of the
    // loaded binary. We avoid scanning deeper to prevent false positives from
    // JMP instructions in the init/bootstrap code.
    let scan_start = ret_addr as usize;
    let scan_end = (ret_addr as usize)
        .saturating_add(SNAPSHOT_ENTRY_SCAN_WINDOW)
        .min(load_addr as usize + load_size)
        .min(snapshot.len().saturating_sub(JMP_ABS_OPERAND_LEN));

    // Scan for JMP $xxxx (opcode $4C) targeting a plausible entry address.
    // Use the LOWEST valid target — the Dali packer stores JMP $1100 as the
    // first JMP-to-entry in its depacker code.
    let mut best: Option<u16> = None;
    for i in scan_start..scan_end {
        if snapshot[i] == OPCODE_JMP_ABS {
            let lo = snapshot[i + 1];
            let hi = snapshot[i + 2];
            let target = u16::from_le_bytes([lo, hi]);
            // Target must be a plausible entry: above min_entry and in RAM (<$8000)
            if target >= min_entry && target < SNAPSHOT_ENTRY_MAX_ADDR {
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
/// Walks backward from `end` down to `start` through the diff range, examining each
/// gap (run of same-as-snapshot bytes):
///
/// 1. **Tiny trailing cluster**: If `gap_len >= TRAILING_CLUSTER_GAP` (`4` bytes) and the
///    trailing cluster has fewer than `TRAILING_CLUSTER_MAX_SIZE` (`16`) diff bytes,
///    returns `pos` immediately (handles depacker tails such as PUCrunch's 10-byte cluster).
/// 2. **High-memory workspace cluster**: If `gap_end < WORKSPACE_CLUSTER_GAP_CEILING` (`$F000`),
///    `gap_len` lies in `WORKSPACE_CLUSTER_MIN_GAP..WORKSPACE_CLUSTER_MAX_GAP` (`128..4096`),
///    and the trailing cluster has at most `WORKSPACE_CLUSTER_MAX_SIZE` (`512`) diff bytes,
///    records `pos` as the candidate trim boundary and continues walking backward.
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
        if gap_len >= TRAILING_CLUSTER_GAP && cluster_diffs < TRAILING_CLUSTER_MAX_SIZE {
            return pos;
        }

        // Check 2: gap (>= 2 bytes) separating high workspace cluster from main decompressed payload.
        let main_len = (pos + 1).saturating_sub(start);
        if gap_end < WORKSPACE_CLUSTER_GAP_CEILING
            && (WORKSPACE_CLUSTER_MIN_GAP..WORKSPACE_CLUSTER_MAX_GAP).contains(&gap_len)
            && main_len > 0
            && (cluster_diffs == 0 || cluster_diffs <= WORKSPACE_CLUSTER_MAX_SIZE)
        {
            best_trim_pos = Some(pos);
            curr_cluster_end = pos;
        }
    }

    best_trim_pos.unwrap_or(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::TargetSystem;

    const MEM_SIZE: usize = 0x1_0000;

    fn blank_state() -> (Vec<u8>, Vec<u8>, Vec<bool>) {
        (
            vec![0u8; MEM_SIZE],
            vec![0u8; MEM_SIZE],
            vec![false; MEM_SIZE],
        )
    }

    fn mark_diff_range(
        mem: &mut [u8],
        snapshot: &[u8],
        written: &mut [bool],
        range: impl IntoIterator<Item = usize>,
    ) {
        for addr in range {
            mem[addr] = snapshot[addr].wrapping_add(1);
            written[addr] = true;
        }
    }

    /// T2.0.1: Pins the 256-byte ceiling-proximity boundary in `detect_output_range`.
    #[test]
    fn detector_ceiling_proximity_boundary() {
        // Case A: Level 0 ceiling on C64 ($9FFF). A trailing zero-fill extends from
        // `trimmed_end + 1` through $A001 (only 2 written bytes above $9FFF, so
        // `has_output_above` is false). Escalation to Level 1 ($CFFF) therefore
        // depends exclusively on `trimmed_end + 256 >= 0x9FFF`.
        let run_level0 = |trimmed_end: usize| -> Option<(u16, u16)> {
            let (mut mem, snapshot, mut written) = blank_state();
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=trimmed_end);
            for w in &mut written[(trimmed_end + 1)..=0xA001] {
                *w = true;
            }
            detect_output_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x0801,
                &TargetSystem::C64,
            )
        };

        // 0x9EFE + 256 = 0x9FFE < 0x9FFF -> does not escalate; extension stops at $9FFF.
        assert_eq!(run_level0(0x9EFE), Some((0x0801, 0x9FFF)));
        // 0x9EFF + 256 = 0x9FFF >= 0x9FFF -> escalates to $CFFF; extension reaches $A001.
        assert_eq!(run_level0(0x9EFF), Some((0x0801, 0xA001)));

        // Case B: Level 1 ceiling on C64 ($CFFF) with I/O mapped ($00=$2F, $01=$37).
        // A 20-byte cluster inside $D000..=$DFFF is ignored by `has_output_above`
        // because I/O is mapped, so escalation to Level 2 ($FFEF) depends only on
        // `trimmed_end + 256 >= 0xCFFF`.
        let run_level1 = |trimmed_end: usize| -> Option<(u16, u16)> {
            let (mut mem, mut snapshot, mut written) = blank_state();
            mem[0x00] = 0x2F;
            mem[0x01] = 0x37;
            snapshot[0x00] = 0x2F;
            snapshot[0x01] = 0x37;
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=trimmed_end);
            mark_diff_range(&mut mem, &snapshot, &mut written, 0xDF00..=0xDF13);
            detect_output_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x0801,
                &TargetSystem::C64,
            )
        };

        // 0xCEFE + 256 = 0xCFFE < 0xCFFF -> stops at Level 1 ($CEFE).
        assert_eq!(run_level1(0xCEFE), Some((0x0801, 0xCEFE)));
        // 0xCEFF + 256 = 0xCFFF >= 0xCFFF -> escalates to Level 2 and includes $DF00..=$DF13.
        assert_eq!(run_level1(0xCEFF), Some((0x0801, 0xDF13)));
    }

    /// T2.0.2: Pins the `0x2000` high-region escalation window in `detect_output_range`.
    #[test]
    fn detector_high_region_window_0x2000() {
        // On C64, Level 0 boundary is $9FFF (`0x9FFF - 0x2000 = 0x7FFF`).
        // With a 20-byte diff cluster at $B000..=$B013 (gap > 4096 bytes so it is not
        // trimmed as a workspace cluster), escalation happens iff `trimmed_end >= 0x7FFF`.
        let run_case = |lower_end: usize| -> Option<(u16, u16)> {
            let (mut mem, snapshot, mut written) = blank_state();
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=lower_end);
            mark_diff_range(&mut mem, &snapshot, &mut written, 0xB000..=0xB013);
            detect_output_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x0801,
                &TargetSystem::C64,
            )
        };

        // 0x7FFE < 0x9FFF - 0x2000 (0x7FFF): does not escalate at Level 0, nor does
        // the mid-boundary fallback overwrite the early return from the Level 0 loop.
        assert_eq!(run_case(0x7FFE), Some((0x0801, 0x7FFE)));
        // 0x7FFF == 0x9FFF - 0x2000: escalates to Level 1 ($CFFF) and includes $B000..=$B013.
        assert_eq!(run_case(0x7FFF), Some((0x0801, 0xB013)));
    }

    /// T2.0.3: Pins the 4-byte minimum high-region diff threshold in `detect_output_range`.
    #[test]
    fn detector_min_high_region_bytes_boundary() {
        // Setup 1: Level 0 boundary ($9FFF) with `trimmed_end = 0x9EFE` (not near ceiling,
        // within 0x2000 window) and written zero-fill bytes extending across $9FFF into
        // $A000..($A000 + high_count - 1).
        let run_zero_fill_above = |high_count: usize| -> Option<(u16, u16)> {
            let (mut mem, snapshot, mut written) = blank_state();
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=0x9EFE);
            for w in &mut written[0x9EFF..(0xA000 + high_count)] {
                *w = true;
            }
            detect_output_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x0801,
                &TargetSystem::C64,
            )
        };

        // 3 written bytes in $A000..=$CFFF (< 4): does not escalate; extension caps at $9FFF.
        assert_eq!(run_zero_fill_above(3), Some((0x0801, 0x9FFF)));
        // 4 written bytes in $A000..=$CFFF (>= 4): escalates to $CFFF; extension reaches $A003.
        assert_eq!(run_zero_fill_above(4), Some((0x0801, 0xA003)));

        // Setup 2: Level 1 boundary ($CFFF) with I/O mapped ($00=$2F, $01=$37).
        // 16 diff bytes at $DFF0..=$DFFF are inside I/O (excluded from `has_output_above`),
        // and `high_count` diff bytes at $E000..($E000 + high_count - 1) are outside I/O.
        let run_io_split_above = |high_count: usize| -> Option<(u16, u16)> {
            let (mut mem, mut snapshot, mut written) = blank_state();
            mem[0x00] = 0x2F;
            mem[0x01] = 0x37;
            snapshot[0x00] = 0x2F;
            snapshot[0x01] = 0x37;
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=0xB000);
            mark_diff_range(&mut mem, &snapshot, &mut written, 0xDFF0..=0xDFFF);
            mark_diff_range(
                &mut mem,
                &snapshot,
                &mut written,
                0xE000..(0xE000 + high_count),
            );
            detect_output_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x0801,
                &TargetSystem::C64,
            )
        };

        // 3 non-I/O diff bytes above $CFFF (< 4): stops at Level 1 ($B000).
        assert_eq!(run_io_split_above(3), Some((0x0801, 0xB000)));
        // 4 non-I/O diff bytes above $CFFF (>= 4): escalates to Level 2 ($E003).
        assert_eq!(run_io_split_above(4), Some((0x0801, 0xE003)));
    }

    /// T2.0.4: Pins the 512-byte maximum forward zero-fill extension in `scan_hybrid_range`.
    #[test]
    fn detector_zero_fill_extension_limit() {
        let run_with_zero_fill = |zero_fill_len: usize| -> Option<(u16, u16, u16, bool)> {
            let (mut mem, snapshot, mut written) = blank_state();
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=0x1000);
            for w in &mut written[0x1001..=(0x1000 + zero_fill_len)] {
                *w = true;
            }
            scan_hybrid_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x9FFF,
                false,
                &TargetSystem::C64,
            )
        };

        // 511 bytes of trailing written zero-fill -> extends full 511 bytes to $11FF.
        assert_eq!(
            run_with_zero_fill(511),
            Some((0x0801, 0x11FF, 0x1000, true))
        );
        // 512 bytes of trailing written zero-fill -> extends full 512 bytes to $1200.
        assert_eq!(
            run_with_zero_fill(512),
            Some((0x0801, 0x1200, 0x1000, true))
        );
        // 513 bytes (and 700 bytes) of trailing written zero-fill -> capped at +512 ($1200).
        assert_eq!(
            run_with_zero_fill(513),
            Some((0x0801, 0x1200, 0x1000, true))
        );
        assert_eq!(
            run_with_zero_fill(700),
            Some((0x0801, 0x1200, 0x1000, true))
        );
    }

    /// T2.0.5: Pins the pre-RAM diff count (`< 64`) and gap (`> 128`) thresholds in `scan_hybrid_range`.
    #[test]
    fn detector_hybrid_scan_thresholds() {
        // Part 1: `diffs_below < 64` boundary when `gap_before_ram > 128` (start = $0400,
        // C64 ram_start = $0800, so total span is 1024 bytes and gap is >= 960 > 128).
        let run_diffs_below = |diffs_below: usize| -> Option<(u16, u16, u16, bool)> {
            let (mut mem, snapshot, mut written) = blank_state();
            mark_diff_range(
                &mut mem,
                &snapshot,
                &mut written,
                0x0400..(0x0400 + diffs_below),
            );
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=0x1000);
            scan_hybrid_range(
                &mem,
                &snapshot,
                &written,
                0x0400,
                0x9FFF,
                false,
                &TargetSystem::C64,
            )
        };

        // 63 diffs below $0800 (< 64) with gap 961 (> 128) -> skips pre-RAM diffs to $0801.
        assert_eq!(run_diffs_below(63), Some((0x0801, 0x1000, 0x1000, true)));
        // 64 diffs below $0800 (< 64 is false) -> keeps pre-RAM start $0400.
        assert_eq!(run_diffs_below(64), Some((0x0400, 0x1000, 0x1000, true)));

        // Part 2: `gap_before_ram > 128` boundary with 1 diff byte at `first` just below $0800.
        // For a single diff at `first`, `gap_before_ram = (0x0800 - first) - 1`.
        let run_gap_before_ram = |gap_len: usize| -> Option<(u16, u16, u16, bool)> {
            let (mut mem, snapshot, mut written) = blank_state();
            let first_pre_ram = 0x0800 - (gap_len + 1);
            mark_diff_range(
                &mut mem,
                &snapshot,
                &mut written,
                first_pre_ram..=first_pre_ram,
            );
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=0x1000);
            scan_hybrid_range(
                &mem,
                &snapshot,
                &written,
                0x0400,
                0x9FFF,
                false,
                &TargetSystem::C64,
            )
        };

        // gap_before_ram == 128 (> 128 is false, first = $077F) -> keeps $077F.
        assert_eq!(
            run_gap_before_ram(128),
            Some((0x077F, 0x1000, 0x1000, true))
        );
        // gap_before_ram == 129 (> 128 is true, first = $077E) -> advances start to $0801.
        assert_eq!(
            run_gap_before_ram(129),
            Some((0x0801, 0x1000, 0x1000, true))
        );

        // Part 3: `skip_trim` bypasses `trim_trailing_clusters`.
        let (mut mem, snapshot, mut written) = blank_state();
        mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=0x1000);
        mark_diff_range(&mut mem, &snapshot, &mut written, 0x1010..=0x1015);
        assert_eq!(
            scan_hybrid_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x9FFF,
                false,
                &TargetSystem::C64
            ),
            Some((0x0801, 0x1000, 0x1000, true))
        );
        assert_eq!(
            scan_hybrid_range(
                &mem,
                &snapshot,
                &written,
                0x0800,
                0x9FFF,
                true,
                &TargetSystem::C64
            ),
            Some((0x0801, 0x1015, 0x1015, true))
        );
    }

    /// T2.0.6: Pins all cluster-size, gap-length, and address-ceiling thresholds in `trim_trailing_clusters`.
    #[test]
    fn detector_trim_trailing_clusters_thresholds() {
        let run_cluster = |main_end: usize, gap_len: usize, cluster_len: usize| -> usize {
            let (mut mem, snapshot, mut written) = blank_state();
            mark_diff_range(&mut mem, &snapshot, &mut written, 0x0801..=main_end);
            let cluster_start = main_end + gap_len + 1;
            let cluster_end = cluster_start + cluster_len - 1;
            mark_diff_range(
                &mut mem,
                &snapshot,
                &mut written,
                cluster_start..=cluster_end,
            );
            trim_trailing_clusters(&mem, &snapshot, &written, 0x0801, cluster_end)
        };

        // Check 1 gap boundary (`gap_len >= 4`) with a tiny cluster of 10 bytes (< 16):
        // gap_len = 3 -> not trimmed (returns cluster_end = 0x1000 + 3 + 10 = 0x100D).
        assert_eq!(run_cluster(0x1000, 3, 10), 0x100D);
        // gap_len = 4 -> trimmed to main_end (0x1000).
        assert_eq!(run_cluster(0x1000, 4, 10), 0x1000);

        // Check 1 cluster-size boundary (`cluster_diffs < 16`) with gap_len = 10 (< 128):
        // cluster_len = 15 (< 16) -> trimmed to 0x1000.
        assert_eq!(run_cluster(0x1000, 10, 15), 0x1000);
        // cluster_len = 16 (< 16 is false, gap < 128) -> not trimmed (0x1000 + 10 + 16 = 0x101A).
        assert_eq!(run_cluster(0x1000, 10, 16), 0x101A);

        // Check 2 gap lower boundary (`(128..4096).contains(&gap_len)`) with cluster_len = 32:
        // gap_len = 127 -> not trimmed (0x1000 + 127 + 32 = 0x109F).
        assert_eq!(run_cluster(0x1000, 127, 32), 0x109F);
        // gap_len = 128 -> trimmed to 0x1000.
        assert_eq!(run_cluster(0x1000, 128, 32), 0x1000);

        // Check 2 gap upper boundary (`128..4096` excludes 4096) with cluster_len = 32:
        // gap_len = 4095 -> trimmed to 0x1000.
        assert_eq!(run_cluster(0x1000, 4095, 32), 0x1000);
        // gap_len = 4096 -> not trimmed (0x1000 + 4096 + 32 = 0x2020).
        assert_eq!(run_cluster(0x1000, 4096, 32), 0x2020);

        // Check 2 workspace cluster max size (`cluster_diffs <= 512`) with gap_len = 200:
        // cluster_len = 512 -> trimmed to 0x1000.
        assert_eq!(run_cluster(0x1000, 200, 512), 0x1000);
        // cluster_len = 513 -> not trimmed (0x1000 + 200 + 513 = 0x12C9).
        assert_eq!(run_cluster(0x1000, 200, 513), 0x12C9);

        // Check 2 `gap_end < 0xF000` ceiling with gap_len = 200, cluster_len = 32:
        // gap_end = 0xEFFF (main_end = 0xEFFF - 200 = 0xEF37) -> trimmed to 0xEF37.
        assert_eq!(run_cluster(0xEF37, 200, 32), 0xEF37);
        // gap_end = 0xF000 (main_end = 0xF000 - 200 = 0xEF38) -> not trimmed (0xF000 + 32 = 0xF020).
        assert_eq!(run_cluster(0xEF38, 200, 32), 0xF020);

        // Degenerate range `end <= start` returns `end` unchanged.
        let (mem, snapshot, written) = blank_state();
        assert_eq!(
            trim_trailing_clusters(&mem, &snapshot, &written, 0x1000, 0x1000),
            0x1000
        );
    }

    /// T2.0.7: Verifies that untouched memory returns `None` and pins fallback scan paths.
    #[test]
    fn detector_nothing_written_returns_none() {
        let (mem, snapshot, written) = blank_state();
        for system in [
            TargetSystem::C64,
            TargetSystem::C128,
            TargetSystem::Vic20,
            TargetSystem::Plus4,
            TargetSystem::Pet20,
        ] {
            assert_eq!(
                scan_hybrid_range(&mem, &snapshot, &written, 0x0800, 0x9FFF, false, &system),
                None
            );
            assert_eq!(
                detect_output_range(&mem, &snapshot, &written, 0x0800, 0x0801, &system),
                None
            );
        }

        // Kernal fallback ($E000..=$FFFF): when diffs exist only above the highest
        // standard ceiling ($FFEF), all boundary scans return `None` and the
        // $E000..=$FFFF fallback catches the range.
        let (mut mem_k, snapshot_k, mut written_k) = blank_state();
        mark_diff_range(&mut mem_k, &snapshot_k, &mut written_k, 0xFFF0..=0xFFF5);
        assert_eq!(
            detect_output_range(
                &mem_k,
                &snapshot_k,
                &written_k,
                0x0800,
                0x0801,
                &TargetSystem::C64
            ),
            Some((0xFFF0, 0xFFF5))
        );
    }

    /// T2.0.8: Pins `find_entry_in_snapshot` offsets (`+0x300`, `+0x400`), `< 0x8000` ceiling, and lowest-target selection.
    #[test]
    fn find_entry_in_snapshot_cases() {
        let ret_addr: u16 = 0x080D;
        let load_addr: u16 = 0x0801;
        let load_size: usize = 0x1000;

        let place_jmp = |offset_from_ret: usize, target: u16| -> Vec<u8> {
            let mut snapshot = vec![0u8; MEM_SIZE];
            let pos = (ret_addr as usize) + offset_from_ret;
            let [lo, hi] = target.to_le_bytes();
            snapshot[pos] = 0x4C;
            snapshot[pos + 1] = lo;
            snapshot[pos + 2] = hi;
            snapshot
        };

        // Empty snapshot (no JMP $4C) -> None.
        let empty = vec![0u8; MEM_SIZE];
        assert_eq!(
            find_entry_in_snapshot(&empty, load_addr, load_size, ret_addr),
            None
        );

        // Minimum plausible target is `ret_addr + 0x300` ($0B0D):
        // $0B0C (ret_addr + 0x2FF) is rejected; $0B0D (ret_addr + 0x300) is accepted.
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x10, 0x0B0C), load_addr, load_size, ret_addr),
            None
        );
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x10, 0x0B0D), load_addr, load_size, ret_addr),
            Some(0x0B0D)
        );

        // Upper target ceiling is `< 0x8000`:
        // $7FFF is accepted; $8000 is rejected.
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x10, 0x7FFF), load_addr, load_size, ret_addr),
            Some(0x7FFF)
        );
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x10, 0x8000), load_addr, load_size, ret_addr),
            None
        );

        // Scan window is `[ret_addr, ret_addr + 0x400)`:
        // Offset 0x3FF is inside the window; offset 0x400 is outside.
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x3FF, 0x1100), load_addr, load_size, ret_addr),
            Some(0x1100)
        );
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x400, 0x1100), load_addr, load_size, ret_addr),
            None
        );

        // `load_addr + load_size` clamps the scan window when smaller than `ret_addr + 0x400`.
        assert_eq!(
            find_entry_in_snapshot(&place_jmp(0x50, 0x1100), load_addr, 0x20, ret_addr),
            None
        );

        // Multiple valid JMP targets: prefers the lowest target address.
        let mut multi = place_jmp(0x10, 0x2400);
        let pos2 = (ret_addr as usize) + 0x30;
        let [lo, hi] = 0x1100u16.to_le_bytes();
        multi[pos2] = 0x4C;
        multi[pos2 + 1] = lo;
        multi[pos2 + 2] = hi;
        assert_eq!(
            find_entry_in_snapshot(&multi, load_addr, load_size, ret_addr),
            Some(0x1100)
        );
    }

    /// T2.4.2: Pins the exact values of every named threshold constant in `detector.rs`.
    #[test]
    fn detector_constants_have_expected_values() {
        assert_eq!(CEILING_PROXIMITY, 256);
        assert_eq!(HIGH_REGION_WINDOW, 0x2000);
        assert_eq!(MIN_HIGH_REGION_BYTES, 4);
        assert_eq!(KERNAL_FALLBACK_START, 0xE000);
        assert_eq!(KERNAL_FALLBACK_END, 0xFFFF);
        assert_eq!(HYBRID_MIN_BYTES, 64);
        assert_eq!(HYBRID_SCAN_GAP, 128);
        assert_eq!(MAX_ZERO_FILL_EXTENSION, 512);
        assert_eq!(SNAPSHOT_ENTRY_MIN_OFFSET, 0x0300);
        assert_eq!(SNAPSHOT_ENTRY_SCAN_WINDOW, 0x0400);
        assert_eq!(JMP_ABS_OPERAND_LEN, 2);
        assert_eq!(OPCODE_JMP_ABS, 0x4C);
        assert_eq!(SNAPSHOT_ENTRY_MAX_ADDR, 0x8000);
        assert_eq!(TRAILING_CLUSTER_GAP, 4);
        assert_eq!(TRAILING_CLUSTER_MAX_SIZE, 16);
        assert_eq!(WORKSPACE_CLUSTER_GAP_CEILING, 0xF000);
        assert_eq!(WORKSPACE_CLUSTER_MIN_GAP, 128);
        assert_eq!(WORKSPACE_CLUSTER_MAX_GAP, 4096);
        assert_eq!(WORKSPACE_CLUSTER_MAX_SIZE, 512);
    }
}
