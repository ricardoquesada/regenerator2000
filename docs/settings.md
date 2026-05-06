# Settings

Regenerator 2000 has two types of settings (like most modern editors):

- Settings
- Document Settings

## Program Settings

The Settings dialog allows you to configure global preferences for the application. It can be accessed via `File -> Settings` or using the shortcut ++alt+p++ or ++ctrl+comma++.

```text
┌ Settings ----------------------------------------┐
│                                                  │
│  [X] Sync Blocks View                            │
│  [X] Sync Hex Dump View                          │
│  [ ] Sync Sprites View                           │
│  [ ] Sync Charset View                           │
│  [ ] Sync Bitmap View                            │
│                                                  │
│  [X] Open the latest file on startup             │
│  [X] Default is Unexplored (for new files)       │
│  [X] Check for updates                           │
│                                                  │
│  Entropy Warning Threshold: < 7.5 >              │
│                                                  │
│  Theme: < Dracula >                              │
│                                                  │
└──────────────────────────────────────────────────┘
```

### Options

#### Sync View Options

The following options control whether different views automatically
synchronize their cursor with the main Disassembly View.
When enabled, navigating in the Disassembly View will update the others to show
the corresponding memory location. Listed in keyboard shortcut order (++ctrl+1++ through ++ctrl+5++).

1. **Sync Blocks View** (++ctrl+1++): Synchronizes the Blocks view.
2. **Sync Hex Dump View** (++ctrl+2++): Synchronizes the Hex Dump view.
3. **Sync Sprites View** (++ctrl+3++): Synchronizes the Sprites view.
4. **Sync Charset View** (++ctrl+4++): Synchronizes the Charset view.
5. **Sync Bitmap View** (++ctrl+5++): Synchronizes the Bitmap view.

#### General Options

6. **Open the latest file on startup**
       - When enabled, the application will automatically open the
         last project you were working on when it starts up. This allows you to quickly
         resume your work.

7. **Default is Unexplored (for new files)**
       - When enabled, newly loaded files will treat all memory regions as
         Undefined/Unexplored rather than assuming they are Code. This is useful
         when you want to start disassembly from scratch without any automatic assumptions.

8. **Check for updates**
       - When enabled, the application checks for new releases on startup and displays
         the latest available version in the top-right corner if an update is available.

#### Advanced Options

9. **Entropy Warning Threshold**
       - Sets the Shannon entropy value above which the *entire binary file* is
         flagged as potentially encrypted or compressed during initial load. Use `Left`/`Right`
         to decrease/increase the value in 0.1 increments (range 0.0–8.0).

10. **Theme**
        - Allows you to choose the visual theme of the application.
          There are 9 built-in themes available. You can change themes in two ways:

          - **Quick cycle**: Press `Left`/`Right` to cycle through themes with instant live preview.
          - **Full list**: Press `Enter` to open the theme selector popup and pick from the list.

          Available themes: Solarized Dark, Solarized Light, Dracula (default), Gruvbox Dark,
          Gruvbox Light, Monokai, Nord, Catppuccin Mocha, Catppuccin Latte.
         See [Themes](themes.md) for screenshots and details.

## Document Settings

You can customize how Regenerator 2000 analyzes the binary and exports the code by accessing the **Document Settings**
dialog (Shortcut: ++alt+d++ or ++ctrl+shift+d++).

```text
┌ Document Settings ─────────────────────────────────────────────────────┐
│                                                                        │
│  [ ] Display External Labels at top                                    │
│  [x] Preserve long bytes (@w, +2, .abs, etc)                           │
│  [x] BRK single byte                                                   │
│  [ ] Patch BRK                                                         │
│  [ ] Use Illegal Opcodes                                               │
│  [x] Auto-generate Labels & Cross-refs                                 │
│                                                                        │
│  Description:                                                          │
│  (empty)                                                               │
│                                                                        │
│  Max X-Refs: < 5 >                                                     │
│  Arrow Columns: < 6 >                                                  │
│  Text Line Limit: < 40 >                                               │
│  Words/Addrs per line: < 5 >                                           │
│  Bytes per line: < 8 >                                                 │
│  Fill run threshold: < 8 >                                             │
│  Assembler: < 64tass >                                                 │
│  System: < Commodore 64 >                                              │
│  System Labels:                                                        │
│    [x] KERNAL                                                          │
│    [ ] BASIC                                                           │
│    [ ] I/O                                                             │
│  [x] Exclude well-known addresses from symbolic analysis               │
│  [x] Show system comments                                              │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

!!! Note

    The **System Labels**, **Exclude well-known addresses**, and **Show system comments** options are
    system-dependent and only appear when the selected system provides the corresponding data.

### Options

1. **Display External Labels at top**
       - **Description**: If enabled, generates labels, including external labels in the disassembly view. The exported
         file will contain all labels, regardless of this option.

2. **Preserve long bytes**
       - **Description**: Ensures that instructions using absolute addressing (3 bytes) are not optimized by the assembler
         into zero-page addressing (2 bytes) upon re-assembly. It adds prefixes like `@w`, `+2`, or `.abs` depending on the
         selected assembler to maintain the exact byte count of the original binary.

         This is useful to preserve the original byte count of the binary, for example, when disassembling a binary that
         contains absolute addresses.

3. **BRK single byte**
       - **Description**: Treats the `BRK` instruction as a 1-byte instruction. By default, this is enabled as most C64
         code historically used `BRK` as a 1-byte instruction in practice, even though the 6502 architecture technically
         treats it as a 2-byte instruction (the instruction itself followed by a padding/signature byte). Disable this
         if your code specifically relies on the 2nd byte after a `BRK`.

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
       - **Description**: If `BRK single byte` is disabled, this option ensures that the exported
         assembly code correctly includes the padding byte after `BRK`, preserving the original program structure on
         assemblers that might otherwise treat `BRK` as a single byte.
       - Notice that not all assemblers support the "Patch BRK" disabled.
       - This option is disabled (greyed out) when `BRK single byte` is enabled, or when the assembler is **KickAssembler**
         or **ca65**.

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

         When "Use Illegal Opcodes" is enabled, the disassembly might look like the following:

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
         sta a_0002
         dcp a_0002          ; x-ref: $082b
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

6. **Auto-generate Labels & Cross-refs**
       - **Description**: When enabled, the application will automatically perform a code analysis
         to generate auto-labels, cross-references, and other analysis data when the project is loaded or
         as needed.

7. **Description**
       - **Description**: A short free-form description or note for this document/binary. Press `Enter` to start editing; press `Enter` again to save or `Esc` to cancel.

8. **Max X-Refs**
       - **Description**: The maximum number of Cross-References (addresses that call/jump to a location) to display in the
         side comments for any given line.

9. **Arrow Columns**
       - **Description**: The number of character columns reserved on the left side of the disassembly view for drawing
         control flow arrows (branches and jumps). Increasing this can make complex control flow easier to read.

10. **Text Line Limit**
        - **Description**: The maximum number of characters to display on a single line for Text block types before wrapping
          or truncating.

11. **Words/Addrs per line**
        - **Description**: Controls how many 16-bit values (Words or Addresses) are displayed on a single line when using
          that Block Type. Range: 1-8.

12. **Bytes per line**
        - **Description**: Controls how many 8-bit values (Bytes) are displayed on a single line when using the Byte Block
          Type. Range: 1-40.

13. **Fill run threshold**
        - **Description**: When a `Byte` block contains a run of **N or more consecutive identical bytes**, the disassembler
          automatically collapses the entire run into a single assembler fill directive instead of emitting one `.byte` per
          value. Set to `0` to disable the optimization entirely (default: 8).

          The directive name varies by assembler:

          | Assembler     | Directive syntax            |
          |---------------|-----------------------------|
          | 64tass        | `.fill count, value`        |
          | ACME          | `!fill count, value`        |
          | KickAssembler | `.fill count, value`        |
          | ca65          | `.res count, value`         |

          **Example** — 16 consecutive `$00` bytes (64tass format):

          ```asm
          ; Without fill run threshold (or threshold > 16):
          .byte $00, $00, $00, $00, $00, $00, $00, $00
          .byte $00, $00, $00, $00, $00, $00, $00, $00

          ; With fill run threshold <= 16:
          .fill 16, $00
          ```

          Runs are only coalesced within a single `Byte` block. A label, cross-reference,
          splitter, or comment mid-block will break the run at that boundary.

14. **Assembler**
        - **Description**: Selects the target assembler syntax for export. Supported assemblers include **64tass**,
          **ACME**, **KickAssembler**, and **ca65**. Changing this updates the syntax used in the disassembly view to match
          the target. Press `Enter` to open the assembler selector popup, or use `Left`/`Right` to cycle.

15. **System**
        - **Description**: Defines the target hardware system (e.g., Commodore 64). This helps the analyzer identify
          system-specific memory maps, hardware registers (like VIC-II or SID), and ROM routines.
          Press `Enter` to open the system selector popup, or use `Left`/`Right` to cycle.
          Changing the system resets the enabled **System Labels** features.

16. **System Labels** *(system-dependent)*
        - **Description**: A group of checkboxes that control which system label sets are loaded for the current system.
          For example, on the Commodore 64 the available sets include **KERNAL**, **BASIC**, **I/O**, etc.
          Each set provides pre-defined labels for well-known addresses (ROM entry points, hardware registers, etc.).
          Toggle a set with `Space` or `Enter`. The available sets depend on the selected **System**.

17. **Exclude well-known addresses from symbolic analysis** *(system-dependent)*
        - **Description**: When enabled, addresses that are covered by the system's system label definitions are excluded
          from the analyzer's code-walking pass. This prevents the analyzer from chasing into ROM routines or hardware
          registers, which can reduce false positives in the disassembly. Only appears for systems that define excluded
          address ranges in their system config.

18. **Show system comments** *(system-dependent)*
        - **Description**: When enabled, pre-defined system comments (e.g. hardware register descriptions like
          "Border Color" for `$D020` on the C64) are displayed as side comments in the disassembly view.
          Only appears for systems that include system comments in their system config.
