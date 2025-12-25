# Regenerator2000 requirements

Have 6502 disassembler with a Turbo Assembler / Turbo Debugger (from Borland in 80's and 90s) look and feel, but with
modern features from IDA Pro 5.0.

* It should be able to disassemble 6502 code, including undocumented opcodes
* It should be able to convert (or tag) opcodes to code, or data
* Data opcodes can be represented as: bytes, words, entry for lookup table (word), or lookup tables with hi/lo addresses
* User should be able to add comments
* User should be able to add labels
* It should support undo/redo commands.
* It should export the disassembled code to something that can be re-assembled with 64tass. But it should be flexible
  because in the future it should support other assemblers like ACME, cc65, etc.
* Everything can be done from the keyboard. Navigating the code should support vi-like keys.
* Mouse support is welcome. But if the user doesn't have a mouse, it should work as well.
* The "Menu" at the top should have all the typical Menu entries like:
    * File: Open binary, Open project, save project, export disassembly
    * Edit: Undo, Redo, Code, Data, Undefined, (data can have: Byte, Word, Lookup table, Lo/Hi, Hi/Lo), Add comment,
      Rename (label)
    * Jump: Jump to operand, Jump to address, Jump by name
    * View: Show opcodes, show auto-comments
* The main view, where the disassembly is should be "tabulated", in the sense that address, opcodes, comments, etc,
  should have its own "tab order".
    * it should also have arrows pointing up/down when there is a jump, similar to IDA Pro, at least IDA Pro 5.0 (the
      old version)
* It should support different binary formats like:
    * bin: pure data
    * prg: typical c64 format, where the first 2 bytes represent the load address (Hi/lo)
* It should be possible to change the origin (if the user made a mistake when loading a bin, it should be able to re set
  the origin)
* It should use Rust and Ratatui for the interface. It is going to be a TUI, like Tuber Assembler from Borland.
* The project should contain a README.md describing the project and explaining how to compile and run it.
* The project should have a reasonable .gitignore file, and should follow modern Rust best practices
