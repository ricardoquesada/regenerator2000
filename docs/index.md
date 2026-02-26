# Regenerator 2000

<p align="center">
  <img src="regenerator2000_logo.png" alt="Regenerator 2000 Logo" width="600"/>
</p>

# The Modern 6502 Disassembler

**Regenerator 2000** is a modern, interactive disassembler for the Commodore 64 and other 6502-based systems. It combines the retro feel of Turbo Debugger with the power of modern tools like IDA Pro, all within a fast, keyboard-centric terminal interface.

---

## At a Glance

Explore the regenerative power of Regenerator 2000 through its various views:

=== "Disassembly"

    **The Code View**: The heart of the operation. Navigate code, follow jumps, and label everything.

    ![Disassembly View](regenerator2000_disassembly_screenshot.png)

=== "Hexdump"

    **The Data View**: Inspect raw memory, spotting patterns in data and tables.

    ![Hexdump View](regenerator2000_hexdump_screenshot.png)

=== "Character Set"

    **Visualizing Graphics**: Instantly see 8x8 character data.

    ![Charset View](regenerator2000_charset_screenshot.png)

=== "Sprites"

    **Sprite Gallery**: View 24x21 sprites in all their glory.

    ![Sprites View](regenerator2000_sprites_screenshot.png)

=== "Bitmap"

    **Bitmap Mode**: Visualize memory as a bitmap (HiRes or MultiColor).

    ![Bitmap View](regenerator2000_bitmap_screenshot.png)

=== "Blocks"

    **Structure Analysis**: See how Regenerator 2000 analyzes and segments the binary into code and data blocks.

    ![Blocks View](regenerator2000_blocks_screenshot.png)

=== "Debugger"

    **Live Debugging**: Connect to VICE, view registers, memory, breakpoints, and step through code.

    ![Debugger View](regenerator2000_debugger_screenshot.png)

---

## Key Features

- **🚀 6502 & Undocumented Opcodes**: Full support for the 6502 instruction set.
- **⚡ Fast TUI**: Built with Rust for blazingly fast performance.
- **🧠 Analysis**: Automatically create labels and comments.
- **⏪ Undo/Redo**: Experiment without fear.
- **🏷️ Labels & Comments**: Rename subroutines and variable for readability.
- **🐛 VICE Debugger Integration**: Connect to a running VICE emulator for live debugging — step through code, inspect registers, set breakpoints and watchpoints.
- **🤖 MCP Integration**: Collaborate with AI assistants for deeper analysis.
- **💾 Project Saving**: Save your work and resume later.
- **📤 Export**: Generate compilable source code or VICE labels for debugging.

## Quick Start

1.  **Install**:

    ```bash
    cargo install regenerator2000
    ```

2.  **Run**:

    ```bash
    regenerator2000 my_game.prg
    ```

3.  **Explore**:
    - **Move**: Arrow keys, map, or jumps.
    - **Define Code**: Press ++c++
    - **Define Data**: Press ++b++
    - **Comment**: Press ++semicolon++
    - **Rename**: Press ++l++

[Get Started Now](install.md){ .md-button .md-button--primary }

---

## Documentation

- **[Installation & Usage](install.md)**: Setup guide.
- **[Views](views.md)**: Detailed breakdown of each view.
- **[Keyboard Shortcuts](keyboard_shortcuts.md)**: Master the controls.
- **[Debugger (VICE)](debugger.md)**: Connect to VICE for live debugging.
- **[MCP Integration](mcp.md)**: meaningful AI collaboration.

## Tutorial

A typical workflow involves loading a file, identifying code and data regions, labeling them, and iteratively refining the disassembly.

```mermaid
flowchart TD
    S1[1. Load File] --> S2[2. Explore]
    S2 --> S3[3-6. Define Code, Data, Labels, Comments]
    S3 --> S7[7. Save Project]
    S3 --> S2
    S7 --> Q{Debug?}
    Q -- No --> S10[10. Export Project]
    Q -- Yes --> S8[8. Export VICE Labels]
    S8 --> S9[9. Debug in VICE]
    S9 --> S2
    S10 --> Done[Assemble it, patch it, etc.]
```

For a detailed step-by-step walkthrough, check out the [full tutorial](tutorial.md).
