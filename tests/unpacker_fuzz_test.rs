#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! No-panic fuzz and robustness tests for the 6502 unpacker and packer detection.
//!
//! The unpacker runs untrusted bytes through a full 6502 emulation, so every
//! public entry point must return `Ok` or a typed [`UnpackError`] for any
//! input. These tests feed deterministic pseudo-random and pathological images
//! to [`unpack`] and [`detect`] and assert that nothing panics.
//!
//! Every grid test is table-driven: it runs each input under
//! [`std::panic::catch_unwind`], collects every panicking input, and asserts
//! once at the end, so one run shows the complete set of failing inputs
//! instead of stopping at the first one.
//!
//! Random data almost never contains a BASIC `SYS` line, so most grids also
//! run with `forced_entry = Some(load_addr)` to make sure the emulator itself
//! is exercised and not only the "no entry point" early return.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use regenerator2000_core::packers::LoadedImage;
use regenerator2000_core::packers::registry::detect;
use regenerator2000_core::state::types::System;
use regenerator2000_core::unpacker::{UnpackConfig, UnpackError, unpack};

// ─────────────────────── helpers ───────────────────────

/// Instruction budget for fuzz runs. Small enough that the full grid stays
/// fast, large enough that random code runs through both emulation stages.
const FUZZ_MAX_INSTRUCTIONS: u64 = 20_000;

/// Wall-clock limit for the full TX.1 grid (Test Plan TX.1: "under 2 s").
/// The grid takes well under a second on a laptop, because the unpacker
/// crate builds with `opt-level = 3` even in the dev profile.
const TX1_TIME_LIMIT: Duration = Duration::from_secs(2);

/// The 6502 `JMP absolute` opcode.
const OPCODE_JMP_ABS: u8 = 0x4C;

/// The 6502 `JAM` (KIL) illegal opcode that halts a real CPU.
const OPCODE_JAM: u8 = 0x02;

/// The CBM BASIC `SYS` token.
const BASIC_TOKEN_SYS: u8 = 0x9E;

/// Size of the 6502 address space in bytes.
const ADDRESS_SPACE: usize = 0x1_0000;

/// Deterministic pseudo-random byte generator (xorshift32).
///
/// Same algorithm as `tests/parser_fuzz_test.rs`, so failures reproduce from
/// the seed alone. A zero seed would produce an all-zero stream, so callers
/// use seeds `>= 1`.
fn pseudo_random_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        out.push(state as u8);
    }
    out
}

/// One fuzz input: the image, where it loads, and how the run is configured.
#[derive(Debug, Clone)]
struct FuzzInput {
    label: String,
    data: Vec<u8>,
    load_addr: u16,
    forced_entry: Option<u16>,
    system: Option<System>,
}

/// Runs [`unpack`] on `input` and returns a description of the input if the
/// call panicked. A typed `Err` is a valid outcome and is not reported.
fn unpack_panics(input: &FuzzInput) -> Option<String> {
    let config = UnpackConfig {
        forced_entry: input.forced_entry,
        max_instructions: FUZZ_MAX_INSTRUCTIONS,
        target_system: input.system.clone(),
        ..UnpackConfig::default()
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // The result value is irrelevant here: `Ok` and every typed `Err` are
        // acceptable outcomes. Only a panic is a failure.
        unpack(&input.data, input.load_addr, &config, None).is_ok()
    }));
    outcome.is_err().then(|| {
        format!(
            "{} (len={}, load=${:04X}, forced_entry={:?}, system={:?})",
            input.label,
            input.data.len(),
            input.load_addr,
            input.forced_entry.map(|e| format!("${e:04X}")),
            input.system,
        )
    })
}

/// Runs every input and panics once with the complete list of panicking inputs.
fn assert_no_unpack_panics(inputs: &[FuzzInput]) {
    let panicking: Vec<String> = inputs.iter().filter_map(unpack_panics).collect();
    assert!(
        panicking.is_empty(),
        "unpack() panicked on {} of {} inputs:\n  {}",
        panicking.len(),
        inputs.len(),
        panicking.join("\n  ")
    );
}

/// Builds the cartesian fuzz grid of seeds × sizes × load addresses ×
/// `{no forced entry, forced entry at the load address}`.
fn random_grid(
    seeds: &[u32],
    sizes: &[usize],
    load_addrs: &[u16],
    system: Option<&System>,
) -> Vec<FuzzInput> {
    let mut inputs = Vec::new();
    for &seed in seeds {
        for &size in sizes {
            let data = pseudo_random_bytes(seed, size);
            for &load_addr in load_addrs {
                for forced_entry in [None, Some(load_addr)] {
                    inputs.push(FuzzInput {
                        label: format!("seed={seed}"),
                        data: data.clone(),
                        load_addr,
                        forced_entry,
                        system: system.cloned(),
                    });
                }
            }
        }
    }
    inputs
}

/// Encodes a minimal CBM BASIC line `<line> SYS<digits>` at `basic_start`,
/// followed by the end-of-program marker. Returns the raw bytes (no PRG
/// header) as they would sit in memory starting at `basic_start`.
fn basic_sys_line(basic_start: u16, digits: &[u8]) -> Vec<u8> {
    // Link pointer (2) + line number (2) + SYS token (1) + digits + EOL (1).
    let line_len = 2 + 2 + 1 + digits.len() + 1;
    let next_line = basic_start.wrapping_add(line_len as u16);
    let mut bytes = Vec::with_capacity(line_len + 2);
    bytes.extend_from_slice(&next_line.to_le_bytes());
    bytes.extend_from_slice(&10u16.to_le_bytes());
    bytes.push(BASIC_TOKEN_SYS);
    bytes.extend_from_slice(digits);
    bytes.push(0x00);
    // End-of-program marker: a zero link pointer.
    bytes.extend_from_slice(&[0x00, 0x00]);
    bytes
}

// ═══════════════════════ TX.1 ═══════════════════════

/// TX.1: random images of every size, at every interesting load address, with
/// and without a forced entry, never make `unpack()` panic.
#[test]
fn unpack_random_bytes_never_panics() {
    let seeds: Vec<u32> = (1..=64).collect();
    let sizes = [1, 2, 16, 256, 4096, 65535];
    let load_addrs = [0x0000, 0x0801, 0x1000, 0xC000, 0xFFFF];
    let inputs = random_grid(&seeds, &sizes, &load_addrs, None);
    assert_eq!(
        inputs.len(),
        64 * 6 * 5 * 2,
        "grid must cover every combination"
    );

    let started = Instant::now();
    assert_no_unpack_panics(&inputs);
    let elapsed = started.elapsed();
    assert!(
        elapsed < TX1_TIME_LIMIT,
        "the TX.1 grid took {elapsed:?}, above the {TX1_TIME_LIMIT:?} limit: \
         a small instruction budget must keep every run short"
    );
}

// ═══════════════════════ TX.2 ═══════════════════════

/// TX.2: a reduced random grid on every supported target system. Load
/// addresses come from each system's own `ram_start()` and
/// `default_basic_start()`, never from hardcoded constants.
#[test]
fn unpack_random_bytes_all_systems() {
    let systems = [
        System::C64,
        System::C128,
        System::Vic20,
        System::Plus4,
        System::C16,
        System::Pet20,
        System::Pet40,
    ];
    let seeds: Vec<u32> = (1..=8).collect();
    let sizes = [16, 4096];

    let mut inputs = Vec::new();
    for system in &systems {
        let load_addrs = [system.ram_start(), system.default_basic_start(), 0xC000];
        inputs.extend(random_grid(&seeds, &sizes, &load_addrs, Some(system)));
    }
    assert_eq!(inputs.len(), systems.len() * 8 * 2 * 3 * 2);
    assert_no_unpack_panics(&inputs);
}

// ═══════════════════════ TX.3 ═══════════════════════

/// TX.3: hand-built pathological images return without panicking.
#[test]
fn unpack_pathological_images() {
    let system = System::C64;
    let basic_start = system.default_basic_start();
    let full = ADDRESS_SPACE - usize::from(basic_start);

    let mut cases: Vec<(&str, Vec<u8>, u16)> = vec![
        ("all zero", vec![0x00; full], basic_start),
        ("all $FF", vec![0xFF; full], basic_start),
        ("all JAM", vec![OPCODE_JAM; full], basic_start),
        // The data ends exactly at the top of the address space.
        ("ends at $FFFF", pseudo_random_bytes(7, 0x100), 0xFF00),
        ("single byte at $FFFF", vec![0x60], 0xFFFF),
        // More bytes than fit between the load address and `$FFFF`.
        (
            "longer than address space",
            pseudo_random_bytes(9, ADDRESS_SPACE),
            0x0801,
        ),
        (
            "longer than space at $C000",
            pseudo_random_bytes(11, 0x8000),
            0xC000,
        ),
        (
            "full 64 KB at $0000",
            pseudo_random_bytes(13, ADDRESS_SPACE),
            0x0000,
        ),
    ];

    // Malformed SYS lines: the parser must reject or clamp them, and whatever
    // address it yields must not crash the emulator.
    let malformed_sys: [(&str, &[u8]); 6] = [
        ("SYS without digits", b""),
        ("SYS with non-digits", b"ABC"),
        ("SYS above $FFFF", b"99999"),
        ("SYS with huge number", b"18446744073709551616"),
        ("SYS to $FFFF", b"65535"),
        ("SYS with spaces and parens", b" (2061)"),
    ];
    for (label, digits) in malformed_sys {
        cases.push((label, basic_sys_line(basic_start, digits), basic_start));
    }

    // A SYS line whose link pointer points back to itself (an infinite BASIC
    // line chain).
    let mut self_linked = basic_sys_line(basic_start, b"2061");
    self_linked[..2].copy_from_slice(&basic_start.to_le_bytes());
    cases.push(("self-linked BASIC line", self_linked, basic_start));

    // A SYS line truncated right after the token, at the end of memory.
    cases.push((
        "truncated SYS at end of memory",
        vec![0x01, 0xFF, 0x0A, 0x00, BASIC_TOKEN_SYS],
        0xFFFB,
    ));

    let inputs: Vec<FuzzInput> = cases
        .into_iter()
        .flat_map(|(label, data, load_addr)| {
            [None, Some(load_addr)].map(|forced_entry| FuzzInput {
                label: label.to_string(),
                data: data.clone(),
                load_addr,
                forced_entry,
                system: None,
            })
        })
        .collect();
    assert_no_unpack_panics(&inputs);
}

// ═══════════════════════ TX.4 ═══════════════════════

/// TX.4: a self-jumping stub (`JMP *`) never leaves the depacker search, so
/// `unpack()` must return [`UnpackError::Phase1Timeout`] and must not execute
/// more instructions than the configured budget.
#[test]
fn jmp_self_returns_phase1_timeout_within_budget() {
    const BUDGET: u64 = 100_000;
    let load_addr = System::C64.default_basic_start();
    let [lo, hi] = load_addr.to_le_bytes();
    let stub = [OPCODE_JMP_ABS, lo, hi];

    let config = UnpackConfig {
        forced_entry: Some(load_addr),
        max_instructions: BUDGET,
        ..UnpackConfig::default()
    };
    let highest_progress = std::cell::Cell::new(0u64);
    let progress = |count: u64| highest_progress.set(highest_progress.get().max(count));

    let result = unpack(&stub, load_addr, &config, Some(&progress));

    assert_eq!(result.err(), Some(UnpackError::Phase1Timeout));
    assert!(
        highest_progress.get() <= BUDGET,
        "progress reported {} instructions, above the budget of {BUDGET}",
        highest_progress.get()
    );
}

// ═══════════════════════ TX.5 ═══════════════════════

/// TX.5: packer detection over random 64 KB images never panics, for every
/// interesting load address and for both `load_end` conventions in use (the
/// engine's exclusive end and `file_info`'s inclusive end).
#[test]
fn packer_detection_random_images_never_panics() {
    let load_addrs: [u16; 5] = [0x0000, 0x0801, 0x080B, 0x0817, 0xFFFF];
    let mut panicking = Vec::new();
    let mut runs = 0usize;

    for seed in 1..=256u32 {
        let image = pseudo_random_bytes(seed, ADDRESS_SPACE);
        // Vary the payload length per seed so `load_end` covers short files,
        // files that end at `$FFFF`, and everything in between.
        let payload_len = pseudo_random_bytes(seed ^ 0x5A5A_5A5A, 2)
            .iter()
            .fold(0usize, |acc, &b| (acc << 8) | usize::from(b));
        for &load_addr in &load_addrs {
            let exclusive_end = (usize::from(load_addr) + payload_len).min(ADDRESS_SPACE) as u16;
            let inclusive_end = exclusive_end.wrapping_sub(1);
            for load_end in [exclusive_end, inclusive_end] {
                runs += 1;
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    detect(&LoadedImage::for_c64(&image, load_addr, load_end)).is_some()
                }));
                if outcome.is_err() {
                    panicking.push(format!(
                        "seed={seed} load=${load_addr:04X} load_end=${load_end:04X}"
                    ));
                }
            }
        }
    }

    assert_eq!(runs, 256 * load_addrs.len() * 2);
    assert!(
        panicking.is_empty(),
        "detect() panicked on {} of {runs} inputs:\n  {}",
        panicking.len(),
        panicking.join("\n  ")
    );
}
