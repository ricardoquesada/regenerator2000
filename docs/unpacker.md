# Binary Unpacker

Regenerator 2000 features a built-in, CPU-emulated **Binary Unpacker** designed specifically for compressed or "packed"
Commodore 64 binaries (`.prg`).

Many C64 programs and games are distributed in compressed forms using packers like Dali, Exomizer, PUCrunch, or
ByteBoozer. These programs contain a small decompression loop (the depacker stub) at the start, followed by compressed
data. Analyzing a packed binary directly is impossible because the real code and data are scrambled until executed.

Instead of requiring you to exit the program and run command-line utilities like `unp64`, Regenerator 2000 can emulate
the unpacking routine directly inside a background 6502 emulator sandbox and extract the fully decompressed binary ready
for disassembly!

---

## How It Works: The Two-Phase Emulation

The unpacker runs a cycle-accurate MOS 6502 emulation sandbox with custom system memory mappings. It uses a robust
two-phase execution heuristic based on the classic [**unp64**](http://iancoog.altervista.org/)
algorithm:

```
  ┌────────────────────────────────────────────────────────┐
  │ Phase 1: Find the Depacker                             │
  │ - Starts at BASIC SYS entry point (e.g., SYS 2061)     │
  │ - Emulates instructions                                │
  │ - Stops when PC drops below return boundary ($0800)    │
  └──────────────────────────┬─────────────────────────────┘
                             │ (depacker loop located)
                             ▼
  ┌────────────────────────────────────────────────────────┐
  │ Phase 2: Decompress Binary                             │
  │ - Continues emulation from the located loop            │
  │ - Tracks every byte written to RAM                     │
  │ - Stops when PC jumps back above $0800 (exit stub)     │
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
  the bootstrap finishes setting up, the program counter (`PC`) drops below the return address boundary (default:
  `$0800`).

### Phase 2: Decompress Binary

Once the decompression routine begins:

- The emulator continues execution, but it now turns on a **write-tracking bitmap** covering the entire 64 KB RAM
  address space.
- Every time the emulated CPU writes a byte to memory, the corresponding bit is set to `true`.
- Decompression continues until the `PC` jumps back above the return address boundary (returning to normal RAM space,
  usually around `$0800` or the game's start address), signaling that the packer has completed decompression and is
  about to jump to the main game loop.

### Extraction & Range Reconstruction

Once the emulation stops:

- The unpacker analyzes the write-tracking bitmap to locate the lowest and highest modified RAM memory addresses.
- It compares the final memory state with a pre-emulation snapshot to trim out temporary depacker workspaces (which are
  often written near the very end of RAM).
- The modified range is extracted as a fresh, clean binary payload, and the final `PC` is captured to automatically
  suggest the correct decompressed **Entry Point** for your disassembly session.

---

## Packer Signature Database

To ensure reliable decompression across diverse packer variants, Regenerator 2000 includes an automated **Packer Signature Database** that inspects the binary before emulation begins.

When a signature match is found, the unpacker automatically tunes its emulation parameters:

- **Exomizer (v1.x, 2.x, 3.0, 3.02+)**: Identifies decruncher loops, extracts entry points, and handles zero-page pointer overrides.
- **Dali (v0.3.3+)**: Dynamically resolves entry points and extracts end-address pointers from zero-page decruncher tables.
- **ByteBoozer (v1.0 & v2.0)**: Detects zero-page workspace locations (`$10`) and landing entry points.
- **PUCrunch**: Intercepts zero-page end-address pointer (`$FA`) and start address headers.
- **TinyCrunch (v1 & v2)**: Handles 2-pass in-place decrunchers and calculates accurate memory boundaries.
- **MC-Cracken**: Extracted end-address pointers (`$AE-$AF`) and entry point targets (`$1100`).
- **Other Supported Packers**: Cruel Cruncher, Time Cruncher (Scoop), Commodore Cruncher System (CCS), Turbo Cruncher, Action Replay, Final Cartridge III, Triad Cruncher, Eagle Cruncher, Super Cruncher, and more.

---

## Advanced Sandboxing Features

To ensure high compatibility with packers that utilize sophisticated hardware configurations, the unpacker implements
advanced system-level sandboxing features:

- **PLA Bank Switching**: Simulates C64 Processor Port (`$01`) banking logic. It correctly handles cases where packers
  decompress data into RAM hidden underneath the `$D000`–`$DFFF` I/O area, or mapping BASIC and Kernal ROMs in and out.
- **ROM Interception Stubs**: Rather than loading actual C64 Kernal and BASIC ROMs (which are copyrighted), the emulator
  intercepts standard Commodore ROM entry vectors (such as the KERNAL `GETIN` vector at `$FFE4`, `CHROUT` at `$FFD2`, or
  screen clear `CINT` at `$FF5B`). It feeds simulated keystrokes, completes screen operations, and forces simulated
  subroutine returns (`RTS`) to prevent the emulated CPU from looping forever on missing hardware components.
- **PLA-visible RAM Writes**: Suppresses writes to the `$D000` I/O chip registers unless the PLA configuration maps RAM
  underneath, preventing hardware registers from acting as dead-ends during emulation.

---

## Using the Unpacker

### 1. Automatic Entropy Detection

When you load a binary file into Regenerator 2000, it calculates the **Shannon Entropy** of the data. Compressed or
encrypted binaries have extremely high entropy (>= 7.5).

If a newly imported file matches this signature (or a known packer is detected):

1. **Import Context Setup Dialog**: A warning is displayed at the bottom of the dialog (e.g. `⚠️ High Entropy (7.82). Packed with Exomizer 3.x.`) and the primary focused button is automatically set to **`< Unpack >`** so you can unpack immediately with a single press of **++enter++**.
2. **High Entropy Warning Dialog**: Standalone warnings also notify you of packed binaries:

> **High Entropy Detected**
> The loaded file has high entropy (e.g. 7.82).
> It is likely compressed or packed.
>
> You can unpack it from Menu -> File -> Unpack Binary...,
> or use external tools like unp64, and reload the unpacked file.

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
3. The unpacker will spawn a background thread to run the 6502 emulation without blocking the UI.
4. The status bar will display a real-time progress counter showing instructions executed (e.g., `Unpacking... $0002F8A0`).
5. Once complete, the disassembler automatically reloads the project with the fully decompressed binary data, correctly
   aligned to its new start address, and updates the disassembly cursor directly to the decompressed entry point!

### 3. Programmatic Unpacking via MCP

If you are using the Model Context Protocol (MCP) server or an AI agent, you can unpack the currently loaded binary using the `r2000_unpack_binary` tool. You can optionally supply custom parameters matching the `unp64` CLI flags:

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

The MCP handler runs the 6502 emulation, reloads the unpacked binary payload into the project state, performs control-flow disassembly from the detected entry point, and returns a summary containing the unpacked memory range (`${start_addr}-${end_addr}`), entry point, depacker address, and total executed instructions.

---

## Troubleshooting & Configuration Limits

- **Timeout limits**: To prevent bad stubs or infinite loops from freezing the application, the unpacker has a safety
  limit of **50 million instructions** (`max_instructions`). If a packer exceeds this without exiting Phase 2, the operation aborts with a
  timeout error.
- **Custom ROMs**: For extremely specialized packers that require actual KERNAL/BASIC ROM code execution, the underlying
  library supports loading custom `$A000` and `$E000` ROM images (configurable via `UnpackConfig`).

---

## unp64 Compatibility & Parity

Regenerator 2000's unpacker uses `unp64` as its reference standard and includes specific compatibility handlers and signature-based overrides to ensure 1:1 parity with `unp64` output across all supported packer families:

- **Exomizer 3**: Intercepts the `CLI; JMP` decruncher signature near the end of packed data to extract the true payload entry point and strip the `$0800`–`$080C` BASIC stub (`start_addr: $080D`), matching `unp64` reference output.
- **ByteBoozer 2**: Extracts zero-page `$77`–`$78` end-address pointers deposited by the decruncher to accurately bound the unpacked payload (e.g., `$E7FF`) and exclude trailing workspace bytes.
- **TinyCrunch**: Correctly handles 2-pass in-place decrunchers that write to disjoint memory regions, ensuring high-memory payload bytes (e.g., `$FFFD`) are preserved in parity with `unp64`.
- **Dali (v0.3.3+)**: Inspects zero-page decruncher end pointers and dynamic JMP targets (e.g., `$1100`), ensuring complete payload extraction without truncation.
- **MC-Cracken**: Sniffs zero-page `$AE`–`$AF` end-address pointers and `$1100` pass-2 entry points to match `unp64` decompressed boundaries.
