# Binary Unpacker

Regenerator 2000 features a built-in, CPU-emulated **Binary Unpacker** designed specifically for compressed or "packed" Commodore 64 binaries (`.prg`).

Many C64 programs and games are distributed in compressed forms using packers like Dali, Exomizer, PUCrunch, or ByteBuster. These programs contain a small decompression loop (the depacker stub) at the start, followed by compressed data. Analyzing a packed binary directly is impossible because the real code and data are scrambled until executed.

Instead of requiring you to exit the program and run command-line utilities like `unp64`, Regenerator 2000 can emulate the unpacking routine directly inside a background 6502 emulator sandbox and extract the fully decompressed binary ready for disassembly!

---

## How It Works: The Two-Phase Emulation

The unpacker runs a cycle-accurate MOS 6502 emulation sandbox with custom system memory mappings. It uses a robust two-phase execution heuristic based on the classic [**unp64**](https://csdb.dk/release/?id=260619&show=summary) algorithm:

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
* The emulator begins executing instructions starting at the parsed `SYS` address.
* It emulates instructions sequentially, looking for where the decompressed unpacking loop is located.
* The emulator detects this because the depacker typically runs in high memory or inside a zero-page workspace, and once the bootstrap finishes setting up, the program counter (`PC`) drops below the return address boundary (default: `$0800`).

### Phase 2: Decompress Binary
Once the decompression routine begins:
* The emulator continues execution, but it now turns on a **write-tracking bitmap** covering the entire 64 KB RAM address space.
* Every time the emulated CPU writes a byte to memory, the corresponding bit is set to `true`.
* Decompression continues until the `PC` jumps back above the return address boundary (returning to normal RAM space, usually around `$0800` or the game's start address), signaling that the packer has completed decompression and is about to jump to the main game loop.

### Extraction & Range Reconstruction
Once the emulation stops:
* The unpacker analyzes the write-tracking bitmap to locate the lowest and highest modified RAM memory addresses.
* It compares the final memory state with a pre-emulation snapshot to trim out temporary depacker workspaces (which are often written near the very end of RAM).
* The modified range is extracted as a fresh, clean binary payload, and the final `PC` is captured to automatically suggest the correct decompressed **Entry Point** for your disassembly session.

---

## Advanced Sandboxing Features

To ensure high compatibility with packers that utilize sophisticated hardware configurations, the unpacker implements advanced system-level sandboxing features:

* **PLA Bank Switching**: Simulates C64 Processor Port (`$01`) banking logic. It correctly handles cases where packers decompress data into RAM hidden underneath the `$D000`–`$DFFF` I/O area, or mapping BASIC and Kernal ROMs in and out.
* **ROM Interception Stubs**: Rather than loading actual C64 Kernal and BASIC ROMs (which are copyrighted), the emulator intercepts standard Commodore ROM entry vectors (such as the KERNAL `GETIN` vector at `$FFE4`, `CHROUT` at `$FFD2`, or screen clear `CINT` at `$FF5B`). It feeds simulated keystrokes, completes screen operations, and forces simulated subroutine returns (`RTS`) to prevent the emulated CPU from looping forever on missing hardware components.
* **PLA-visible RAM Writes**: Suppresses writes to the `$D000` I/O chip registers unless the PLA configuration maps RAM underneath, preventing hardware registers from acting as dead-ends during emulation.

---

## Using the Unpacker in the TUI

### 1. Automatic Entropy Detection
When you load a binary file into Regenerator 2000, it calculates the **Shannon Entropy** of the data. Compressed or encrypted binaries have extremely high entropy (>= 7.5).

If a newly imported file matches this signature, Regenerator 2000 will display a helpful warning dialog:

> **The loaded file has high entropy.**
> It is likely compressed or packed.
> You can unpack it from Menu -> File -> Unpack Binary.

### 2. Executing the Unpack
To unpack the loaded binary:
1. Press **++f10++** (or **++alt+f++**) to open the **File** menu.
2. Select **Unpack Binary**.
3. The unpacker will spawn a background thread to run the 6502 emulation.
4. The status bar will display a real-time execution progress counter representing instructions executed (e.g. `Unpacking... $0002F8A0`).
5. Once complete, the disassembler automatically reloads the project with the fully decompressed binary data, correctly aligned to its new start address, and updates the disassembly cursor directly to the decompressed entry point!

---

## Troubleshooting / Configuration Limits
* **Timeout limits**: To prevent bad stubs or infinite loops from freezing the application, the unpacker has a safety limit of **50 million instructions**. If a packer exceeds this without exiting Phase 2, the operation aborts with a timeout error.
* **Custom ROMs**: For extremely specialized packers that require actual KERNAL/BASIC ROM code execution, the underlying library supports loading custom `$A000` and `$E000` ROM images (configurable via the core configuration).
