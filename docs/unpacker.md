# Binary Unpacker

Regenerator 2000 features a built-in, CPU-emulated **Binary Unpacker** designed specifically for compressed or "packed"
Commodore binaries (`.prg`).

Many C64 programs and games are distributed in compressed forms using packers like Dali, Exomizer, PUCrunch, or
ByteBoozer. These programs contain a small decompression loop (the depacker stub) at the start, followed by compressed
data. Analyzing a packed binary directly is impossible because the real code and data are scrambled until executed.

Instead of requiring you to exit the program and run command-line utilities like `unp64`, Regenerator 2000 can emulate
the unpacking routine directly inside a background 6502 emulator sandbox and extract the fully decompressed binary ready
for disassembly!

---

## Subsystem Architecture & 5-Module Hierarchy

The binary unpacker is organized as an SRP-compliant 5-module subsystem under [
`crates/regenerator2000-core/src/unpacker/`](https://github.com/ricardoquesada/regenerator2000/tree/main/crates/regenerator2000-core/src/unpacker):

```
crates/regenerator2000-core/src/unpacker/
├── mod.rs        // Public API facade (unpack, UnpackConfig, UnpackResult, re-exports)
├── cia.rs        // MOS 6526 CIA 1 & CIA 2 timer state emulation & cycle-stepping (CiaState)
├── bus.rs        // C64Bus (UnpackerMemory) bus, $00/$01 processor port, ROM banking, I/O dispatch
├── engine.rs     // Decompression execution engine, 2-phase 6502 loop, ROM trap handling
└── detector.rs   // Memory range detection, trailing cluster trimming, snapshot diffing heuristics
```

1. **`mod.rs`**: High-level entry point exposing `unpack()`, `UnpackConfig`, `UnpackResult`, `UnpackerMemory`, and
   `UnpackError`.
2. **`cia.rs`**: Encapsulates `CiaState` for MOS 6526 CIA 1 & CIA 2 timer latches, counters, and cycle-stepping (
   `step_cycles(cycles)`).
3. **`bus.rs`**: Implements `mos6502::memory::Bus` for `C64Bus` (`UnpackerMemory`). Provides safe checked ROM slice
   lookups (`checked_sub` and `.get()`), processor port `$00`/`$01` banking, and RAM fallback.
4. **`detector.rs`**: Pure memory range scanning (`detect_output_range`), trailing cluster trimming, and snapshot
   diffing algorithms.
5. **`engine.rs`**: 2-Phase 6502 execution engine, illegal opcode emulation (`SHX`, `SHY`), and ROM vector trap
   handling.

---

## How It Works: Two-Phase Emulation

The unpacker runs a cycle-accurate MOS 6502 emulation sandbox with custom system memory mappings. It uses a robust
two-phase execution heuristic based on the classic [**unp64**](http://iancoog.altervista.org/)
algorithm:

```
  ┌────────────────────────────────────────────────────────┐
  │ Phase 1: Find the Depacker                             │
  │ - Starts at BASIC SYS entry point (e.g., SYS 2061)     │
  │ - Emulates instructions                                │
  │ - Stops when PC drops below RAM start ($0800 on C64)   │
  └──────────────────────────┬─────────────────────────────┘
                             │ (depacker loop located)
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Phase 2: Decompress Binary                             │
  │ - Continues emulation from the located loop            │
  │ - Tracks every byte written to RAM                     │
  │ - Stops when PC jumps back above RAM start (exit stub) │
  └──────────────────────────┬─────────────────────────────┘
                             │ (decompression completed)
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Extraction & Reconstruction                            │
  │ - Scans write-tracking bitmap to locate boundaries     │
  │ - Extracts modified RAM range as clean binary          │
  │ - Identifies the new decompressed entry point          │
  └────────────────────────────────────────────────────────┘
```

### Phase 1: Find the Depacker

Many packers start with a BASIC bootstrap stub (`10 SYS 2061`) which jumps into a loader.

- The emulator begins executing instructions starting at the parsed `SYS` address.
- It emulates instructions sequentially, looking for where the decompressed unpacking loop is located.
- The emulator detects this because the depacker typically runs in high memory or inside a zero-page workspace, and once
  the bootstrap finishes setting up, the program counter (`PC`) drops below the target system's RAM start boundary (
  e.g.,
  `$0800` on C64, `$1C00` on C128, `$1000` on VIC-20, `$0400` on PET).

### Phase 2: Decompress Binary

Once the decompression routine begins:

- The emulator continues execution, but it now turns on a **write-tracking bitmap** covering the entire 64 KB RAM
  address space.
- Every time the emulated CPU writes a byte to memory, the corresponding bit is set to `true`.
- Decompression continues until the `PC` jumps back above the system's RAM start boundary (returning to normal RAM
  space,
  such as `$0800` on C64 or the game's start address), signaling that the packer has completed decompression and is
  about to jump to the main game loop.

### Extraction & Range Reconstruction

Once the emulation stops:

- The unpacker analyzes the write-tracking bitmap to locate the lowest and highest modified RAM memory addresses.
- It compares the final memory state with a pre-emulation snapshot to trim out temporary depacker workspaces (which are
  often written near the very end of RAM).
- The modified range is extracted as a fresh, clean binary payload, and the final `PC` is captured to automatically
  suggest the correct decompressed **Entry Point** for your disassembly session.

---

## Machine Architecture Memory Boundaries

The unpacker supports system-specific memory boundaries configured via `UnpackConfig.target_system`:

- **Commodore 64 (C64)**: Default RAM start `$0800`, BASIC start `$0801`, Screen RAM `$0400`, I/O `$D000`–`$DFFF`.
- **Commodore 128 (C128)**: Default RAM start `$1C00`, BASIC start `$1C01`, MMU banking.
- **VIC-20**: Default RAM start `$1000`, VIC-I character maps.
- **PET (2001/4008/8032)**: Default RAM start `$0400`, monochrome PETSCII maps.
- **Plus/4 & C16**: Default RAM start `$1000`, TED color registers.

---

## Packer Signature Database & Strategy Pattern

To ensure reliable decompression across diverse packer variants, Regenerator 2000 uses a trait-based `Packer` strategy
pattern (`Box<dyn Packer>`) under `src/packers/`.

Supported packers include:

- **Exomizer (v1.x, 2.x, 3.0, 3.02+)**: Identifies decruncher loops, extracts entry points, and handles zero-page
  pointer overrides.
- **Dali (v0.3.3+)**: Dynamically resolves entry points and extracts end-address pointers from zero-page decruncher
  tables.
- **ByteBoozer (v1.0 & v2.0)**: Detects zero-page workspace locations (`$10`) and landing entry points.
- **PUCrunch**: Intercepts zero-page end-address pointer (`$FA`) and start address headers.
- **TinyCrunch (v1 & v2)**: Handles 2-pass in-place decrunchers and calculates accurate memory boundaries.
- **MC-Cracken**: Extracted end-address pointers (`$AE-$AF`) and entry point targets (`$1100`).
- **Other Supported Packers**: Cruel Cruncher, Time Cruncher (Scoop), Commodore Cruncher System (CCS), Turbo Cruncher,
  Action Replay, Final Cartridge III, Triad Cruncher, Eagle Cruncher, Super Cruncher, and more.

---

## Advanced Sandboxing Features

To ensure high compatibility with packers that utilize sophisticated hardware configurations, the unpacker implements
advanced system-level sandboxing features:

- **PLA Bank Switching**: Simulates C64 Processor Port (`$01`) banking logic. It correctly handles cases where packers
  decompress data into RAM hidden underneath the `$D000`–`$DFFF` I/O area, or mapping BASIC and Kernal ROMs in and out.
- **ROM Interception Stubs**: Intercepts standard Commodore ROM entry vectors (such as KERNAL `GETIN` at `$FFE4`,
  `CHROUT` at `$FFD2`, or screen clear `CINT` at `$FF5B`). It feeds simulated keystrokes, completes screen operations,
  and forces simulated subroutine returns (`RTS`) to prevent infinite loops.
- **PLA-visible RAM Writes**: Suppresses writes to `$D000` I/O chip registers unless PLA configuration maps RAM
  underneath.

---

## Using the Unpacker

### 1. Automatic Entropy Detection

When you load a binary file into Regenerator 2000, it calculates the **Shannon Entropy** of the data. Compressed or
encrypted binaries have extremely high entropy (>= 7.4).

If a newly imported file matches this signature (or a known packer is detected):

1. **Import Context Setup Dialog**: A warning is displayed at the bottom of the dialog (e.g.
   `⚠️ High Entropy (7.82). Packed with Exomizer 3.x.`) and the primary focused button is automatically set to *
   *`< Unpack >`** so you can unpack immediately with a single press of **++enter++**.
2. **High Entropy Warning Dialog**: Standalone warnings also notify you of packed binaries.

### 2. Executing the Unpack in the TUI

To unpack the loaded binary from the terminal UI:

1. Open the **File** menu by pressing **++alt+f++** (or clicking **File**).
2. Select **Unpack Binary...** to open the Unpack Options dialog.
    - Pressing **++enter++** immediately runs with default auto-detection.
    - Alternatively, you can manually override:
        - **Entry Point (Hex)** (e.g. `0810`, equivalent to `unp64 -e`): Forcing Phase 1 start.
        - **Return Address (Hex)** (e.g. `0800`, equivalent to `unp64 -r`): Return address boundary for Phase 1.
        - **Depacker Address (Hex)** (e.g. `033C`, equivalent to `unp64 -d`): Forcing Phase 2 decruncher loop.
        - **Max Instructions** (e.g. `50000000`, equivalent to `unp64 -m`): Timeout limit.
3. The unpacker will spawn a background thread to run 6502 emulation without blocking the UI.
4. The status bar displays real-time progress (e.g., `Unpacking... $0002F8A0`).
5. Once complete, the disassembler reloads the decompressed binary data and updates the disassembly cursor to the new
   entry point!

### 3. Programmatic Unpacking via MCP

If you are using the Model Context Protocol (MCP) server or an AI agent, you can unpack the currently loaded binary
using the `r2000_unpack_binary` tool:

```json
{
  "name": "r2000_unpack_binary",
  "arguments": {
    "entry_point": "0810",
    "return_address": "0800",
    "depacker_address": "033C",
    "max_instructions": 100000000
  }
}
```

---

## Troubleshooting & Configuration Limits

- **Timeout limits**: Safety limit of **50 million instructions** (`max_instructions`). If a packer exceeds this without
  exiting Phase 2, the operation aborts with a timeout error.
- **Custom ROMs**: Supports loading custom `$A000` and `$E000` ROM images (configurable via `UnpackConfig`).

---

## unp64 Compatibility & Benchmark Parity

Regenerator 2000's unpacker achieves 100.0% benchmark parity with `unp64` across all 31 test binary files (
`cargo run --bin unpacker_compare_all`), verifying bit-for-bit decompressed payload output for Exomizer 3, ByteBoozer 2,
TinyCrunch, Dali, and MC-Cracken.
