# Regenerator2000 User Guide

## Introduction

**Regenerator2000** is a modern, interactive disassembler for the Commodore 64 (and related 8-bit systems) written in Rust. Unlike traditional batch disassemblers that produce a single text output, Regenerator2000 provides a **unidirectional data flow** environment where you can interactively refine your disassembly.

As you identify code, data, and text regions, the built-in **Auto Labeler** and **Analyzer** constantly work in the background to trace code paths, identify subroutine entry points, and validate your changes. The result is a project that can be exported as fully compilable source code for popular assemblers like **64tass** and **ACME**.

## Block Types

In Regenerator2000, every byte of the loaded binary is assigned a **Block Type**. This type tells the disassembly engine how to interpret that byte. You can change the Block Type for any region of memory using keyboard shortcuts (in Visual Mode or for the single line under the cursor).

The available Block Types are:

### 1. Code
*   **Shortcut**: `c`
*   **Description**: Interprets the bytes as MOS 6502/6510 instructions.
*   **Use Case**: Use this for all executable machine code. The analyzer will automatically follow jumps and branches from these blocks to find other code regions.

### 2. Data Byte
*   **Shortcut**: `b`
*   **Description**: Represents data as single 8-bit values (e.g., `!byte $01, $02`).
*   **Use Case**: Use for lookup tables, sprite data, distinct variables, or memory regions where the data format is unknown.

### 3. Data Word
*   **Shortcut**: `w`
*   **Description**: Represents data as 16-bit Little-Endian values (e.g., `!word $C000`).
*   **Use Case**: Use for 16-bit counters, pointers (that shouldn't be analyzed as code references), or math constants.

### 4. Address
*   **Shortcut**: `a`
*   **Description**: Represents data as 16-bit addresses. Unlike "Data Word", this type explicitly tells the analyzer that the value points to a location in memory.
*   **Use Case**: Essential for **Jump Tables**. When you mark a table as "Address", Regenerator2000 will create Cross-References (X-Refs) to the target locations, allowing you to see where indirect jumps land.

### 5. Text
*   **Shortcut**: `t`
*   **Description**: Interprets bytes as PETSCII/ASCII text sequences.
*   **Use Case**: Use for game messages, high score names, or print routines. The disassembler will try to group contiguous characters into a single string.

### 6. Screencode
*   **Shortcut**: `s`
*   **Description**: Interprets bytes as Commodore Screen Codes (Matrix codes).
*   **Use Case**: Use for data that is directly copied to Screen RAM ($0400). These values differ from standard PETSCII (e.g., 'A' is 1, not 65).

### 7. Lo/Hi Address
*   **Shortcut**: `<` (Shift + ,)
*   **Description**: Marks a byte as the **Low Byte** of a split address table.
*   **Use Case**: C64 games often split address tables into two arrays (one for Low bytes, one for High bytes) for faster indexing with `LDA $xxxx,X`. Mark the Low byte table with this type.

### 8. Hi/Lo Address
*   **Shortcut**: `>` (Shift + .)
*   **Description**: Marks a byte as the **High Byte** of a split address table.
*   **Use Case**: Counterpart to the Lo/Hi type. Mark the High byte table with this type. The analyzer pairs Lo/Hi blocks to resolve the full 16-bit destination address and generate X-Refs.

### 9. External File
*   **Shortcut**: `e`
*   **Description**: Treats the selected region as external binary data.
*   **Use Case**: Use for large chunks of included binary data (like music SID files, raw bitmaps, or character sets) that you don't want to clutter the main source file. These will be exported as `!binary "filename.bin"` includes.

### 10. Undefined
*   **Shortcut**: `?`
*   **Description**: Resets the block to an "Unknown" state.
*   **Use Case**: Use this if you made a mistake and want the Auto-Analyzer to take a fresh look at the usage of this region.

## Organization Tools

Beyond data types, you can organize your view using Splitters and Collapsing:

### Splitters
*   **Shortcut**: `|` (Pipe)
*   **Description**: Inserts a visual separator (newline) in the disassembly view without affecting the binary.
*   **Use Case**: Use this to visually separate logic blocks, subroutines, or data tables that are contiguous in memory but logically distinct.

### Collapsing Blocks
*   **Collapse**: `Ctrl + k`
*   **Uncollapse**: `Ctrl + Shift + k`
*   **Description**: Hides the content of a block, showing only a summary line (e.g., "; ... 256 bytes ...").
*   **Use Case**: Use this to hide large tables, long text strings, or finished subroutines to keep your workspace clean and focus on the code you are currently reverse-engineering.
