# Roadmap - Regenerator 2000

## 🎯 Vision
Regenerator 2000 aims to be the ultimate interactive disassembler for retro computing platforms. Our philosophy is **visual fidelity and platform-native idioms**. We do not want to just say "we support Commodore 64" because it uses a 6502; we want the disassembler to *understand* the Commodore 64—its memory maps, its specific graphics modes, and its hardware registers.

To achieve this without compromising code quality, we are planning a major architectural shift to a **Plugin/Platform Modular System**.

---

## 📅 Development Phases

### 📦 v1.0 - Polishing & Stability for Commodore 64 (Current Focus)
*Target: May 2026*
- Stabilize existing features for Commodore 64.
- Finalize documentation and tutorial.
- General UX polish and bug fixes.

### 📦 v1.x - Add support for other Commodore 8-bit machines
- Add support for other Commodore 8-bit machines: VIC-20, C128, Plus/4, PET
- Each minor release will focus on one machine.
- Adding support for a new machine will require:
    - Adding a new memory map.
    - Adding new or updated views.

### 🏗️ v2.0 - The Architectural Refactor (Plugin System)
To scale to other platforms properly, we need a robust abstraction layer.
- **Modular Views**: Abstract views (Charset, Sprites, Bitmaps) so they are not hardcoded to Commodore.
- **Platform Definitions**: Each platform (C64, Apple II, NES) will define its own memory maps, CPU variants, and supported views.

---

### 🕹️ Phase 3 - Expanding the 6502 Family
Once the v2.0 architecture is in place, we will expand to other classic 6502 machines:
- **Apple II**: Text, Lo-Res, and Hi-Res graphics rendering.
- **NES (Nintendo Entertainment System)**: PPU visualization, Pattern/Name tables.
- **Atari 8-bit family** (Atari 2600/400/800/XL/XE):
- **BBC Micro**:
- **Oric-1**:

---

### 🚀 Phase 4 - Extending to 65xx Varaints
Moving to processors that are similar to the 6502 but offer unique extensions:
- **65C816** (Apple II GS): Support for 16-bit data/addressing and 24-bit memory spaces.
- **CSG 4510** (Mega65 / Commodore 65): Support for the 4510 CPU (internal to the 65-series SOCs), quad-banking, etc.

---

### 🌌 Phase 5 - The Ultimate Goal: Commodore Amiga, and other 68k machines
- Support for the **Motorola 68000 (68k)** architecture.
- Full visualization of custom chips (Copper, Blitter, etc.).
- This will leverage the v2.0 plugin system to its fullest extent.
