# Regenerator 2000

Welcome to the **Regenerator 2000** documentation.

![logo](regenerator2000_logo.png)

Regenerator 2000 is a modern 6502 disassembler, TUI-based, with a Turbo Debugger look and feel, enhanced with features from modern tools like IDA Pro.

## Features

- **6502 Support**: Full support for 6502 instruction set including undocumented opcodes.
- **Interactive TUI**: Built with Rust and Ratatui for a fast, keyboard-centric terminal interface.
- **Analysis**: Distinguishes code from data, identifies jump tables, and more.
- **Editing**: Undo/Redo support, comments, labels, code, data and customizable views.
- **Views**: Disassembly, Hexdump, Sprites, Charset, Bitmap and blocks views.
- **Navigation**: Jump to address, line, operand, and more.
- **Shortcuts**: Keyboard shortcuts for common actions.
- **Settings**: System settings and document settings.

## Getting Started

- **[Installation & Usage](install.md)** - Learn how to install and start using Regenerator 2000.
- **[Views](views.md)** - Discover the different views available in the interface and how to use them.
- **[Blocks](blocks.md)** - Understand the core concept of code and data blocks.
- **[Tutorial](#tutorial)** - Follow a step-by-step walkthrough of a sample reverse-engineering session.

## Tutorial

This short guide walks you through a typical reverse-engineering session with Regenerator 2000.

1.  **Load a file**: Start by loading a program:
    ```bash
    regenerator2000 my_game.prg
    ```
2.  **Explore**: Use the arrow keys or ++page-up++/++page-down++ to scroll. Press ++enter++ on any
    `JMP` or `JSR` operand to follow the code flow and jump to that address. Press ++backspace++
    to return to where you were.
3.  **Define Code**: As you explore, you might find bytes marked as data that look like code.
    Move the cursor there and press ++c++ to disassemble them into instructions.
4.  **Define Data**: Conversely, if you see "code" that looks like garbage (illegal opcodes or nonsense instructions),
    it's likely graphics or tables. Select the lines (using ++shift+v++ and arrows) and press ++b++ to mark them as bytes.
5.  **Labels**: When you identify a subroutine (e.g., a music player init), press ++l++ on its start address
    to give it a name like `init_music`. This name will automatically appear everywhere that address is referenced.
6.  **Comments**: Select a line and press ++semicolon++ to add a side comment explaining what the code does.
    Or press ++colon++ to add a line comment.
7.  **Save Project**: Press ++ctrl+s++ to save your work. This creates a project file
    that preserves all your labels, comments, and formatting.
8.  **Export Project**: Finally, where you are done disassembling, you can export the project
    to a file that can be used by an assembler. Press ++ctrl+e++ to export the project.

## User Reference

- [Keyboard Shortcuts](keyboard_shortcuts.md): Comprehensive list of all keyboard shortcuts.
- [Settings](settings.md): How to configure the application.
- [Assemblers](assemblers.md): Supported assemblers and command line usage.
- [FAQ](faq.md): Frequently Asked Questions.

## Development & Internals

- [Architecture](architecture.md): Deep dive into the internal design and components.
- [Requirements](requirements.md): The project's design goals and manifesto.
