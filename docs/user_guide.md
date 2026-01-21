# Regenerator2000 User Guide

## Introduction

**Regenerator2000** is a modern, interactive disassembler for the Commodore 64 (and related 8-bit systems) written in
Rust. Unlike traditional batch disassemblers that produce a single text output, Regenerator2000 provides a \*
\*unidirectional data flow\*\* environment where you can interactively refine your disassembly.

As you identify code, data, and text regions, the built-in **Auto Labeler** and **Analyzer** constantly work in the
background to trace code paths, identify subroutine entry points, and validate your changes. The result is a project
that can be exported as fully compilable source code for popular assemblers like **64tass**, **ACME**, **KickAssembler**
and **ca65**.

## Command Line Options

Regenerator2000 can be launched from the terminal with some options:

```bash
regenerator2000 [OPTIONS] [FILE]
```

### Options

* `--help`: Displays the help message listing all available options and supported file types.
* `--version`: Displays the current version of Regenerator2000.

### Arguments

* `FILE`: (Optional) The path to a file you wish to open. Regenerator2000 supports various formats, including:
    * **C64 Program Files**: `.prg`
    * **Cartridge Images**: `.crt`
    * **Tape Images**: `.t64`
    * **Vice Snapshot Files**: `.vsf`
    * **Raw Binary Files**: `.bin`, `.raw`
    * **Regenerator2000 Project Files**: `.regen2000proj`

## Block Types

In Regenerator2000, every byte of the loaded binary is assigned a **Block Type**. This type tells the disassembly engine
how to interpret that byte. You can change the Block Type for any region of memory using keyboard shortcuts (in Visual
Mode or for the single line under the cursor).

The available Block Types are:

### 1. Code

- **Shortcut**: `c`
- **Description**: Interprets the bytes as MOS 6502/6510 instructions.
- **Use Case**: Use this for all executable machine code.

Example:

```asm
    ; Code blocks are represented as code
    lda #$00
    sta aD020
```

### 2. Data Byte

- **Shortcut**: `b`
- **Description**: Represents data as single 8-bit values.
- **Use Case**: sprite data, distinct variables, tables, memory regions where the data format is
  unknown, etc.

Example:

```asm
    ; Byte blocks are represented as bytes
    .byte $80, $40, $a2, $ff
```

### 3. Data Word

- **Shortcut**: `w`
- **Description**: Represents data as 16-bit Little-Endian values.
- **Use Case**: Use for 16-bit counters, pointers (that shouldn't be analyzed as code references), or math constants.

Example:

```asm
    ; Word blocks are represented as words
    .word $1234, $ffaa, $5678, $0000, $abcd
```

### 4. Address

- **Shortcut**: `a`
- **Description**: Represents data as 16-bit addresses. Unlike "Data Word", this type explicitly tells the analyzer that
  the value points to a location in memory.
- **Use Case**: Essential for **Jump Tables**. When you mark a table as "Address", Regenerator2000 will create
  Cross-References (X-Refs) to the target locations, allowing you to see where indirect jumps land.

Example:

```asm
  ; Addresss blocks represented as words, that generates an address reference
  .word a1234, aFFAA, a5678, a0000, aABCD
```

### 5. PETSCII Text

- **Shortcut**: `t`
- **Description**: Interprets bytes as PETSCII text sequences.
- **Use Case**: Use for game messages, high score names, or print routines. The disassembler will try to group
  contiguous characters into a single string.

Example:

```asm
  .encode
  .enc "none"
  .text "hello world"
  .endencode
```

### 6. Screencode Text

- **Shortcut**: `s`
- **Description**: Interprets bytes as Commodore Screen Codes (Matrix codes) text.
- **Use Case**: Use for data that is directly copied to Screen RAM ($0400). These values differ from standard PETSCII (
  e.g., 'A' is 1, not 65).

Example:

```asm
  .encode
  .enc "screen"
  .text "hello world"
  .endencode
```

### 7. Lo/Hi Address

- **Shortcut**: `<` (Shift + ,)
- **Description**: Marks the selected bytes as the **Low / High** address table. Must have an even number of bytes.
  The first half will be the lo addresses, the second half will be the hi addresses.
- **Use Case**: C64 games often split address tables into two arrays (one for Low bytes, one for High bytes) for faster
  indexing with `LDA $xxxx,X`. Mark the Low byte table with this type.

  Example:

```asm
  ; Assume that you have these bytes:
  ; $00, $01, $02, $03, $c0, $d1, $e2, $f3
  ; They will be represented as:
  .byte <aC000, <aD101, <aE202, <aF303
  .byte >aC000, >aD101, >aE202, >aF303
```

### 8. Hi/Lo Address

- **Shortcut**: `>` (Shift + .)
- **Description**: Marks the selected bytes as the **High / Low** address table. Must have an even number of bytes.
  The first half will be the hi addresses, the second half will be the lo addresses.
- **Use Case**: C64 games often split address tables into two arrays (one for Low bytes, one for High bytes) for faster
  indexing with `LDA $xxxx,X`. Mark the Low byte table with this type.

  Example:

```asm
  ; Assume that you have these bytes:
  ; $00, $01, $02, $03, $c0, $d1, $e2, $f3
  ; They will be represented as:
  .byte >a00C0, >a01D1, >a02E2, >a03F3
  .byte <a00C0, <a01D1, <a02E2, <a03F3
```

### 9. External File

- **Shortcut**: `e`
- **Description**: Treats the selected region as external binary data.
- **Use Case**: Use for large chunks of included binary data (like music SID files, raw bitmaps, or character sets) that
  you don't want to clutter the main source file. These will be exported as `.binary "filename.bin"` includes.

Example:

```asm
  ; Assume that you have these bytes at address $1000
  ; $00, $01, $02, $03, $c0, $d1, $e2, $f3
  ; A binary file called "export-$1000-$1007.bin" will be generated
  ; And this code will be generated
  .binary "export-$1000-$1007.bin"
```

### 10. Undefined

- **Shortcut**: `?`
- **Description**: Resets the block to an "Unknown" state.
- **Use Case**: Use this if you made a mistake and want the Auto-Analyzer to take a fresh look at the usage of this
  region.

Example:

```asm
  ; Undefined blocks are represented as single bytes, one byte per line.
  .byte $00
  .byte $ca
  .byte $ff
```

## Organization Tools

Beyond data types, you can organize your view using Splitters and Collapsing:

### Splitters

- **Shortcut**: `|` (Pipe)
- **Description**: Inserts a visual separator (newline) in the disassembly view without affecting the binary.
- **Use Case**: Use this to visually separate logic blocks, subroutines, or data tables that are contiguous in memory
  but logically distinct.

### Collapsing Blocks

- **Collapse/Uncollapse**: `Ctrl + k`
- **Description**: Hides or shows the content of a block, showing only a summary line (e.g., "; ... 256 bytes ...").
- **Use Case**: Use this to hide large tables, long text strings, or finished subroutines to keep your workspace clean
  and focus on the code you are currently analyzing.

## Document Settings

You can customize how Regenerator2000 analyzes the binary and exports the code by accessing the **Document Settings**
dialog (Shortcut: `Alt + d`, or `Ctrl + Shift + d`).

### Options

1. **All Labels**
    - **Description**: If enabled, generates labels for all branch targets and referenced addresses, even if they aren't
      strictly necessary for the current view. Useful for ensuring a complete symbol table is generated.

2. **Preserve long bytes**
    - **Description**: Ensures that instructions using absolute addressing (3 bytes) are not optimized by the assembler
      into zero-page addressing (2 bytes) upon re-assembly. It adds prefixes like `@w`, `+2`, or `.abs` depending on the
      selected assembler to maintain the exact byte count of the original binary.

3. **BRK single byte**
    - **Description**: Treats the `BRK` instruction as a 1-byte instruction. By default, the 6502 treats `BRK` as a
      2-byte instruction (the instruction itself followed by a padding/signature byte). Enable this if your code uses
      `BRK` as a 1-byte breakpoint.

4. **Patch BRK**
    - **Description**: If `BRK single byte` is disabled (standard behavior), this option ensures that the exported
      assembly code correctly includes the padding byte after `BRK`, preserving the original program structure on
      assemblers that might otherwise treat `BRK` as a single byte.

5. **Use Illegal Opcodes**
    - **Description**: Enables the disassembler to recognize and decode undocumented (illegal) opcodes. If disabled,
      these bytes will be treated as invalid instructions or data.

6. **Max X-Refs**
    - **Description**: The maximum number of Cross-References (addresses that call/jump to a location) to display in the
      side comments for any given line.

7. **Arrow Columns**
    - **Description**: The number of character columns reserved on the left side of the disassembly view for drawing
      control flow arrows (branches and jumps). Increasing this can make complex control flow easier to read.

8. **Text Line Limit**
    - **Description**: The maximum number of characters to display on a single line for Text block types before wrapping
      or truncating.

9. **Words/Addrs per line**
    - **Description**: Controls how many 16-bit values (Words or Addresses) are displayed on a single line when using
      that Block Type. Range: 1-8.

10. **Bytes per line**
    - **Description**: Controls how many 8-bit values (Bytes) are displayed on a single line when using the Byte Block
      Type. Range: 1-40.

11. **Assembler**
    - **Description**: Selects the target assembler syntax for export. Supported assemblers include **64tass**, **ACME
      **, **KickAssembler**, and **ca65**. Changing this updates the syntax used in the disassembly view to match the
      target.

12. **Platform**
    - **Description**: Defines the target hardware platform (e.g., C64). This helps the analyzer identify
      system-specific memory maps, hardware registers (like VIC-II or SID), and ROM routines.
