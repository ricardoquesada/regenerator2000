# Binary Unpacker

Regenerator 2000 includes a CPU-emulated **Binary Unpacker** for compressed ("packed") Commodore binaries (`.prg`).

Many Commodore 8-bit programs use packers such as Dali, Exomizer, PUCrunch, or ByteBoozer. A packed binary starts with a small decompression stub followed by compressed data. Static analysis cannot read the program until the stub decompresses the code and data into RAM.

Regenerator 2000 runs the unpacking routine inside a background 6502 emulator sandbox and loads the decompressed binary directly into the disassembler.

---

## Subsystem Architecture & 5-Module Hierarchy

The binary unpacker uses five single-responsibility modules under [`crates/regenerator2000-core/src/unpacker/`](https://github.com/ricardoquesada/regenerator2000/tree/main/crates/regenerator2000-core/src/unpacker):

```
crates/regenerator2000-core/src/unpacker/
├── mod.rs        // Public API facade (unpack, UnpackConfig, UnpackResult, re-exports)
├── cia.rs        // MOS 6526 CIA 1 & CIA 2 timer state emulation & cycle-stepping (CiaState)
├── bus.rs        // C64Bus (UnpackerMemory) bus, $00/$01 processor port, ROM banking, I/O dispatch
├── engine.rs     // Decompression execution engine, 2-phase 6502 loop, ROM trap handling
└── detector.rs   // Memory range detection, trailing cluster trimming, snapshot diffing heuristics
```

1. **`mod.rs`**: Exposes `unpack()`, `UnpackConfig`, `UnpackResult`, `UnpackerMemory`, and `UnpackError`.
2. **`cia.rs`**: Implements `CiaState` for MOS 6526 CIA 1 and CIA 2 timer latches, counters, and cycle stepping (`step_cycles(cycles)`).
3. **`bus.rs`**: Implements `mos6502::memory::Bus` for `C64Bus` (`UnpackerMemory`). It handles checked ROM slice lookups (`checked_sub` and `.get()`), processor port `$00`/`$01` banking, and RAM access.
4. **`detector.rs`**: Implements memory range scanning (`detect_output_range`), trailing cluster trimming, and snapshot diffing.
5. **`engine.rs`**: Runs the two-phase 6502 execution loop, undocumented opcodes (`SHX`, `SHY`), and ROM vector traps.

---

## How It Works: Two-Phase Emulation

The unpacker runs a cycle-accurate MOS 6502 emulation sandbox with target system memory mappings. It uses a two-phase execution algorithm based on [**unp64**](http://iancoog.altervista.org/):

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

Most packed programs start with a BASIC bootstrap stub (such as `10 SYS 2061`) that jumps into a loader:

- The emulator starts executing instructions at the parsed `SYS` address.
- It executes the bootstrap setup until it reaches the decompression loop.
- When the program counter (`PC`) drops below the system RAM start boundary (`$0800` on C64, `$1C00` on C128, `$1000` on VIC-20, or `$0400` on PET) or matches a known packer signature, Phase 1 completes.

### Phase 2: Decompress Binary

When the decompression routine starts:

- The emulator enables a **write-tracking bitmap** across the 64 KB RAM address space.
- Each time the CPU writes a byte to RAM, the unpacker marks the corresponding bit as `true`.
- Emulation continues until the `PC` jumps back above the system RAM start boundary (such as `$0800` on C64, `$1C00` on C128, `$1000` on VIC-20, or `$0400` on PET) into the unpacked program.

### Extraction & Range Reconstruction

When emulation stops:

- The unpacker scans the write-tracking bitmap to find the lowest and highest modified RAM addresses.
- It compares the final RAM state against the pre-emulation snapshot to trim temporary depacker workspaces near the top of RAM.
- The unpacker extracts the modified RAM range as a clean binary image and sets the final `PC` as the new **Entry Point**.

---

## Machine Architecture Memory Boundaries

`UnpackConfig.target_system` configures system-specific memory boundaries:

- **Commodore 64 (C64)**: Default RAM start `$0800`, BASIC start `$0801`, Screen RAM `$0400`, I/O `$D000`–`$DFFF`.
- **Commodore 128 (C128)**: Default RAM start `$1C00`, BASIC start `$1C01`, MMU banking.
- **VIC-20**: Default RAM start `$1000`, VIC-I character maps.
- **PET (2001/4008/8032)**: Default RAM start `$0400`, monochrome PETSCII maps.
- **Plus/4 & C16**: Default RAM start `$1000`, TED color registers.

---

## Packer Signature Database & Strategy Pattern

Regenerator 2000 uses the `Packer` trait (`Box<dyn Packer>`) under `src/packers/` to detect and handle specific packer variants:

- **Exomizer (v1.x, 2.x, 3.0, 3.02+)**: Detects decruncher loops, extracts entry points, and applies zero-page pointer overrides.
- **Dali (v0.3.3+)**: Resolves entry points and reads end-address pointers from zero-page decruncher tables.
- **ByteBoozer (v1.0 & v2.0)**: Detects zero-page workspace locations (`$10`) and entry points.
- **PUCrunch**: Reads the zero-page end-address pointer (`$FA`) and start address headers.
- **TinyCrunch (v1 & v2)**: Handles two-pass in-place decrunchers and calculates memory boundaries.
- **MC-Cracken**: Extracts end-address pointers (`$AE`–`$AF`) and entry point targets (`$1100`).
- **Other Supported Packers**: Supports Cruel Cruncher, Time Cruncher (Scoop), Commodore Cruncher System (CCS), Turbo Cruncher, Action Replay, Final Cartridge III, Triad Cruncher, Eagle Cruncher, and Super Cruncher.

---

## Advanced Sandboxing Features

The unpacker emulates hardware features that complex packers require:

- **PLA Bank Switching**: On C64, the bus simulates Processor Port (`$01`) banking so packers can decompress data into RAM under the `$D000`–`$DFFF` I/O area or bank ROMs in and out.
- **ROM Interception Stubs**: Traps standard Commodore ROM vectors (such as KERNAL `GETIN` at `$FFE4`, `CHROUT` at `$FFD2`, and `CINT` at `$FF5B`). It supplies simulated keystrokes and returns `RTS` to prevent infinite wait loops.
- **PLA-Visible RAM Writes**: Ignores writes to `$D000` I/O registers unless the processor port maps RAM into `$D000`–`$DFFF`.

---

## Using the Unpacker

### 1. Automatic Entropy Detection

When you load a binary file, Regenerator 2000 calculates its **Shannon Entropy**. Compressed or encrypted binaries have high entropy (`>= 7.4`).

If a file has high entropy or matches a known packer signature:

1. **Import Context Setup Dialog**: Displays a warning at the bottom of the dialog (for example, `[!] High Entropy (7.82). Packed with Exomizer 3.x.`) and focuses the **`< Unpack >`** button so you can press **++enter++** to unpack immediately.
2. **High Entropy Warning Dialog**: Alerts you when you open a packed binary.

### 2. Unpacking in the TUI

Unpack a loaded binary from the terminal UI:

1. Press **++alt+f++** (or click **File**) to open the **File** menu.
2. Select **Unpack Binary...** to open the Unpack Options dialog:
    - Press **++enter++** to run with automatic detection.
    - Or set manual overrides before you press **++enter++**:
        - **Entry Point (Hex)** (such as `0810`, equivalent to `unp64 -e`): Overrides the Phase 1 start address.
        - **Return Address (Hex)** (such as `0800`, equivalent to `unp64 -r`): Overrides the Phase 1 return boundary.
        - **Depacker Address (Hex)** (such as `033C`, equivalent to `unp64 -d`): Overrides the Phase 2 decruncher loop address.
        - **Max Instructions** (such as `50000000`, equivalent to `unp64 -m`): Sets the instruction timeout limit.
3. Wait while the background thread runs the 6502 emulator. The status bar shows live progress (for example, `Unpacking... $0002F8A0`).
4. When unpacking finishes, Regenerator 2000 loads the decompressed binary and moves the cursor to the new entry point.

### 3. Programmatic Unpacking via MCP

Call the `r2000_unpack_binary` tool from an MCP client or AI agent to unpack the loaded binary:

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

- **Timeout limits**: The unpacker enforces a default limit of **50 million instructions** (`max_instructions`). If a packer exceeds this limit, the unpacker aborts with a timeout error.
- **Custom ROMs**: `UnpackConfig` supports custom `$A000` and `$E000` ROM images.

---

## unp64 Compatibility & Benchmark Parity

The Regenerator 2000 unpacker matches `unp64` output across all 31 benchmark test binaries (`cargo run --bin unpacker_compare_all`), producing identical decompressed payloads for Exomizer 3, ByteBoozer 2, TinyCrunch, Dali, and MC-Cracken.
