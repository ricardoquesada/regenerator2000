# Settings

Regenerator 2000 has two types of settings (like most modern editors):

* Settings:
* Document Settings:

## Settings

The Settings dialog allows you to configure global preferences for the application. It can be accessed via `File -> Settings` or using the shortcut `Alt + O` or `Ctrl + ,`.

```text
┌ Settings ----------------------------------------┐
│                                                  │
│  [X] Open the latest file on startup             │
│  [X] Auto-analyze on load                        │
│  [X] Sync Hex Dump View                          │
│  [ ] Sync Charset View                           │
│  [ ] Sync Sprites View                           │
│  [ ] Sync Bitmap View                            │
│  [X] Sync Blocks View                            │
│  Theme: < Solarized Dark >                       │
│                                                  │
└──────────────────────────────────────────────────┘
```

### Options

1. **Open the latest file on startup**
    - **Description**: When enabled, the application will automatically open the last project you were working on when it starts up. This allows you to quickly resume your work.

2. **Auto-analyze on load**
    - **Description**: If checked, the application will automatically perform a code analysis when a file is loaded. Analysis includes creating labels, cross-references, and other analysis data.

3. **Sync View Options**
    - **Description**: The following options control whether different views automatically synchronize their cursor with the main Disassembly View. When enabled, navigating in the Disassembly View will update the others to show the corresponding memory location.
    - **Sync Hex Dump View**: Synchronizes the Hex Dump view.
    - **Sync Charset View**: Synchronizes the Charset view.
    - **Sync Sprites View**: Synchronizes the Sprites view.
    - **Sync Bitmap View**: Synchronizes the Bitmap view.
    - **Sync Blocks View**: Synchronizes the Blocks view.

4. **Theme**
    - **Description**: Allows you to choose the visual theme of the application. Press `Enter` on this option to open a sub-menu where you can select from available themes (e.g., `Solarized Dark`, `Solarized Light`, etc.).

## Document Settings

You can customize how Regenerator 2000 analyzes the binary and exports the code by accessing the **Document Settings**
dialog (Shortcut: `Alt + d`, or `Ctrl + Shift + d`).

```text
┌ Document Settings ----------------------───────────────────────────────┐
│                                                                        │
│  [ ] All Labels                                                        │
│  [x] Preserve long bytes (@w, +2, .abs, etc)                           │
│  [ ] BRK single byte                                                   │
│  [x] Patch BRK                                                         │
│  [ ] Use Illegal Opcodes                                               │
│                                                                        │
│  Max X-Refs: < 5 >                                                     │
│                                                                        │
│  Arrow Columns: < 6 >                                                  │
│                                                                        │
│  Text Line Limit: < 40 >                                               │
│                                                                        │
│  Words/Addrs per line: < 5 >                                           │
│                                                                        │
│  Bytes per line: < 8 >                                                 │
│                                                                        │
│  Assembler: < 64tass >                                                 │
│                                                                        │
│  Platform: < C64 >                                                     │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

### Options

1. **All Labels**
    - **Description**: If enabled, generates labels, including external labels in the disassembly view. The exported
      file will contain all labels, regardless of this option.

2. **Preserve long bytes**
    - **Description**: Ensures that instructions using absolute addressing (3 bytes) are not optimized by the assembler
      into zero-page addressing (2 bytes) upon re-assembly. It adds prefixes like `@w`, `+2`, or `.abs` depending on the
      selected assembler to maintain the exact byte count of the original binary.

      This is useful to preserve the original byte count of the binary, for example, when disassembling a binary that
      contains absolute addresses.

3. **BRK single byte**
    - **Description**: Treats the `BRK` instruction as a 1-byte instruction. By default, the 6502 treats `BRK` as a
      2-byte instruction (the instruction itself followed by a padding/signature byte). Enable this if your code uses
      `BRK` as a 1-byte breakpoint.

   When "BRK single byte" is enabled, it gets represented as:
    ```asm
    ; These bytes will be diassembled as:
    ; $00, $00, $00, $00
    ; Each BRK consumes only one byte
    $c000   brk
    $c001   brk
    $c002   brk
    $c003   brk
    ```


4. **Patch BRK**
    - **Description**: If `BRK single byte` is disabled (standard behavior), this option ensures that the exported
      assembly code correctly includes the padding byte after `BRK`, preserving the original program structure on
      assemblers that might otherwise treat `BRK` as a single byte.
    - Notice that not all assemblers support the "Patch BRK" disabled.

   When "Patch BRK" is enabled, it gets represented as:
    ```asm
    ; These bytes will be diassembled as:
    ; $00, $00, $00, $00
    ; Each BRK consumes two bytes (BRK + byte data)
    $c000   brk
    $c001   .byte $00 
    $c002   brk
    $c003   .byte $00 
    ```

   When "Patch BRK" is disabled, it gets represented as:
    ```asm
    ; These bytes will be diassembled as:
    ; $00, $00, $00, $00
    ; Each BRK consumes two bytes
    $c000   brk #$00
    $c002   brk #$00
    ```

5. **Use Illegal Opcodes**
    - **Description**: Enables the disassembler to recognize and decode undocumented (illegal) opcodes. If disabled,
      these bytes will be treated as data.

      When "Use Illegal Opcodes" is disabled, the disassembly might look like the following:
      ```asm
      sei
      .byte $ab        ; Invalid or partial instruction
      brk
      .byte $8e
      jsr $8ed0
      and ($d0,x)
      lda $d012        ; Raster Position
      cmp #$60
      bne $0816
      ldy #$00
      .byte $bf        ; Invalid or partial instruction
      brk
      .byte $09
      stx $d020        ; Border Color
      lda #$04
      sta $02
      .byte $c7        ; Invalid or partial instruction
      .byte $02        ; Invalid or partial instruction
      bne $0829
      .byte $8f        ; Invalid or partial instruction
      brk
      .byte $04
      iny
      cpy #$28
      bne $081f
      lda #$80
      .byte $0b        ; Invalid or partial instruction
      .byte $ff        ; Invalid or partial instruction
      bcc $083e
      jmp $0816
      inc $d020        ; Border Color
      jmp $083e
      ```

      When "Use Illegal Opcodes" is disabled, the disassembly might look like the following:
      ```asm
      sei
      lax #$00
      stx $d020        ; Border Color
      stx $d021        ; Background Color 0
      lda $d012        ; x-ref: $081b, $083b; Raster Position
      cmp #$60
      bne $0816
      ldy #$00
      lax $0900,y      ; x-ref: $0833
      stx $d020        ; Border Color
      lda #$04
      sta a02
      dcp a02          ; x-ref: $082b
      bne $0829
      sax $0400
      iny
      cpy #$28
      bne $081f
      lda #$80
      anc #$ff
      bcc $083e
      jmp $0816
      inc $d020        ; x-ref: $0839, $0841; Border Color
      jmp $083e
      ```

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
    - **Description**: Selects the target assembler syntax for export. Supported assemblers include **64tass**,
      **ACME**, **KickAssembler**, and **ca65**. Changing this updates the syntax used in the disassembly view to match
      the target.

12. **Platform**
    - **Description**: Defines the target hardware platform (e.g., C64). This helps the analyzer identify
      system-specific memory maps, hardware registers (like VIC-II or SID), and ROM routines.
