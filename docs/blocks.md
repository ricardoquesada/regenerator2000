# Block Types and helpers

In Regenerator 2000, every byte of the loaded binary is assigned a **Block Type**. This type tells the disassembly
engine how to interpret that byte. You can change the Block Type for any region of memory using keyboard shortcuts
(in Visual Mode or for the single line under the cursor).

The available Block Types are:

## 1. Code

- **Shortcut**: ++c++
- **Description**: Interprets the bytes as MOS 6502/6510 instructions.
- **Use Case**: Use this for all executable machine code.

!!! example

    ```asm
    ; Code blocks are represented as code
    lda #$00
    sta aD020
    ```

## 2. Data Byte

- **Shortcut**: ++b++
- **Description**: Represents data as single 8-bit values.
- **Use Case**: sprite data, distinct variables, tables, memory regions where the data format is
  unknown, etc.

!!! example

    ```asm
    ; Byte blocks are represented as bytes
    .byte $80, $40, $a2, $ff
    ```

## 3. Data Word

- **Shortcut**: ++w++
- **Description**: Represents data as 16-bit Little-Endian values.
- **Use Case**: Use for 16-bit counters, pointers (that shouldn't be analyzed as code references), or math constants.

!!! example

    ```asm
    ; Word blocks are represented as words
    .word $1234, $ffaa, $5678, $0000, $abcd
    ```

## 4. Address

- **Shortcut**: ++a++
- **Description**: Represents data as 16-bit addresses. Unlike "Data Word", this type explicitly tells the analyzer that
  the value points to a location in memory.
- **Use Case**: Essential for **Jump Tables**. When you mark a table as "Address", Regenerator 2000 will create
  Cross-References (X-Refs) to the target locations, allowing you to see where indirect jumps land.

!!! example

    ```asm
    ; Addresss blocks represented as words, that generates an address reference
    .word a1234, aFFAA, a5678, a0000, aABCD
    ```

## 5. PETSCII Text

- **Shortcut**: ++t++
- **Description**: Interprets bytes as PETSCII text sequences.
- **Use Case**: Use for game messages, high score names, or print routines. The disassembler will try to group
  contiguous characters into a single string.

!!! example

    ```asm
    .encode
    .enc "none"
    .text "hello world"
    .endencode
    ```

## 6. Screencode Text

- **Shortcut**: ++s++
- **Description**: Interprets bytes as Commodore Screen Codes (Matrix codes) text.
- **Use Case**: Use for data that is directly copied to Screen RAM ($0400). These values differ from standard PETSCII
  (e.g., 'A' is 1, not 65).

!!! example

    ```asm
    .encode
    .enc "screen"
    .text "hello world"
    .endencode
    ```

## 7. Lo/Hi Address

- **Shortcut**: ++less-than++
- **Description**: Marks the selected bytes as the **Low / High** address table. Must have an even number of bytes.
  The first half will be the lo addresses, the second half will be the hi addresses.
- **Use Case**: C64 games often split address tables into two arrays (one for Low bytes, one for High bytes) for faster
  indexing with `LDA $xxxx,X`. Mark the Low byte table with this type.

!!! example

    ```asm
    ; Assume that you have these bytes:
    ; $00, $01, $02, $03, $c0, $d1, $e2, $f3
    ; They will be represented as:
    .byte <aC000, <aD101, <aE202, <aF303
    .byte >aC000, >aD101, >aE202, >aF303
    ```

## 8. Hi/Lo Address

- **Shortcut**: ++greater-than++
- **Description**: Marks the selected bytes as the **High / Low** address table. Must have an even number of bytes.
  The first half will be the hi addresses, the second half will be the lo addresses.
- **Use Case**: C64 games often split address tables into two arrays (one for Low bytes, one for High bytes) for faster
  indexing with `LDA $xxxx,X`. Mark the Low byte table with this type.

!!! example

    ```asm
    ; Assume that you have these bytes:
    ; $00, $01, $02, $03, $c0, $d1, $e2, $f3
    ; They will be represented as:
    .byte >a00C0, >a01D1, >a02E2, >a03F3
    .byte <a00C0, <a01D1, <a02E2, <a03F3
    ```

## 9. External File

- **Shortcut**: ++e++
- **Description**: Treats the selected region as external binary data.
- **Use Case**: Use for large chunks of included binary data (like music SID files, raw bitmaps, or character sets) that
  you don't want to clutter the main source file. These will be exported as `.binary "filename.bin"` includes.

!!! example

    ```asm
    ; Assume that you have these bytes at address $1000
    ; $00, $01, $02, $03, $c0, $d1, $e2, $f3
    ; A binary file called "export-$1000-$1007.bin" will be generated
    ; And this code will be generated
    .binary "export-$1000-$1007.bin"
    ```

## 10. Undefined

- **Shortcut**: ++question-mark++
- **Description**: Resets the block to an "Unknown" state.
- **Use Case**: Use this if you made a mistake and want the Auto-Analyzer to take a fresh look at the usage of this
  region.

!!! example

    ```asm
    ; Undefined blocks are represented as single bytes, one byte per line.
    .byte $00
    .byte $ca
    .byte $ff
    ```

# Organization Tools

Beyond data types, you can organize your view using Splitters and Collapsing:

## Splitters

- **Shortcut**: ++pipe++
- **Description**: Inserts a visual separator (newline) in the disassembly view without affecting the binary.
- **Use Case**: Use this to visually separate logic blocks, subroutines, or data tables that are contiguous in memory
  but logically distinct.

## Collapsing Blocks

- **Collapse/Uncollapse**: ++ctrl+k++
- **Description**: Hides or shows the content of a block, showing only a summary line (e.g., "; ... 256 bytes ...").
- **Use Case**: Use this to hide large tables, long text strings, or finished subroutines to keep your workspace clean
  and focus on the code you are currently analyzing.
