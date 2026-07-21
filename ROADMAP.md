# Roadmap - Regenerator 2000

## 🎯 Vision

Regenerator 2000 aims to be the ultimate interactive disassembler for retro computing systems.
Our philosophy is **visual fidelity and system-native idioms**.
We do not want to just say "we support Commodore 64" because it uses a 6502;
we want the disassembler to _understand_ each target system—its memory maps,
its specific graphics modes, custom chips, and hardware registers.

To achieve this without compromising code quality, the core architecture has been modernized with a zero-cost
`TargetSystem` domain model, modular action handlers, unified sparse address metadata (`AnnotationManager`), and an
SRP-compliant 5-module binary unpacker.

---

## 📅 Development Status & Phases

### ✅ Shipped: Commodore 8-Bit Machine Suite (v0.9.x)

Full native support for the entire Commodore 8-bit machine family is **100% Shipped**:

- **Commodore 64 (C64)**: Full RAM/ROM banking, VIC-II, SID, CIA 1/2, PETSCII/Screencode.
- **Commodore 128 (C128)**: MMU banking, 80-column VDC, BASIC 7.0 ROM vectors.
- **VIC-20**: Unexpanded & expanded RAM configurations, VIC-I character maps.
- **Plus/4 & C16**: TED chip memory maps, 121-color palettes, BASIC 3.5.
- **PET (BASIC 2.0 and 4.0)**: Monochrome PETSCII, non-standard memory origins.
- **Disk Images (D64 / D71 / D81)**: Track/sector directory parsing and binary file extraction.

---

### 📦 v1.0 - Polishing & Stability (Current Focus)

_Target: Q3 2026_

- Finalize release readiness checklist and full test coverage.
- High-grade architecture modernization across `regenerator2000-core` and `regenerator2000-tui`.
- Cycle-accurate binary unpacker sandbox with 100% UNP64 benchmark parity.
- Production-grade MCP server for automated AI agent collaboration (stdio and HTTP transports).
- Final UX polish, documentation alignment, and roundtrip assembler verification.

---

### 🏗️ v2.0 - Plugin System Architecture

To scale to non-Commodore systems cleanly without code duplication:

- **Decoupled System Definitions**: Abstract `TargetSystem` memory maps, I/O registers, and ROM definitions into plugin
  descriptors.
- **Modular Visual Views**: Abstract Charset, Sprites, and Bitmap renderers to accommodate non-Commodore pixel ratios
  and palette formats.
- **Custom Unpacker Extensions**: Expose trait-based unpacker interfaces for third-party packers.

---

### 🕹️ Phase 3 - Expanding the 6502 Family

Once the v2.0 plugin architecture is in place:

- **Apple II / IIe**: Text, Lo-Res, and Hi-Res graphics visualization.
- **NES (Nintendo Entertainment System)**: PPU visualization, Pattern/Name tables, CHR ROM.
- **Atari 8-bit family** (2600 / 400 / 800 / XL / XE): TIA/ANTIC/GTIA graphics visualization.
- **BBC Micro & Oric-1**: System memory maps and character renderers.

---

### 🚀 Phase 4 - 65xx Architecture Extensions

- **65C816** (Apple IIgs / SNES): 16-bit registers, 24-bit linear addressing.
- **CSG 4510** (Mega65 / Commodore 65): Quad-banking and extended instruction sets.

---

### 🌌 Phase 5 - 68k & Beyond

- **Motorola 68000 (68k)** (Commodore Amiga 500/1000/1200): Copper/Blitter visual state tracing and 32-bit disassembly.
