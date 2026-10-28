# Settings

Regenerator 2000 provides two categories of settings:

-   Program Settings
-   Document Settings

## Program Settings

The Settings dialog configures global application preferences. Select `File -> Settings` or press ++alt+p++ (or ++ctrl+comma++) to open this dialog.

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

These options synchronize the secondary view cursors with the main Disassembly View. If you enable an option, cursor movement in the Disassembly View updates the active secondary view to the same memory address. The options appear in keyboard shortcut order (++ctrl+1++ through ++ctrl+5++):

1.  **Sync Blocks View** (++ctrl+1++): Synchronizes the Blocks view.
2.  **Sync Hex Dump View** (++ctrl+2++): Synchronizes the Hex Dump view.
3.  **Sync Sprites View** (++ctrl+3++): Synchronizes the Sprites view.
4.  **Sync Charset View** (++ctrl+4++): Synchronizes the Charset view.
5.  **Sync Bitmap View** (++ctrl+5++): Synchronizes the Bitmap view.

#### General Options

6.  **Open the latest file on startup**

    -   Opens the most recent project automatically when the application starts.

7.  **Default is Unexplored (for new files)**

    -   Marks all memory regions in new files as `Undefined` (Unexplored) instead of `Code`. Use this option to disassemble a binary manually without initial assumptions.

8.  **Check for updates**

    -   Checks for new releases at startup and displays the latest version in the top-right corner when an update is available.

#### Advanced Options

9.  **Entropy Warning Threshold**

    -   Sets the Shannon entropy threshold (range `0.0` to `8.0`) that triggers a compression or encryption warning during file load. Press `Left` or `Right` to adjust the value in `0.1` increments.

10. **Theme**

    -   Selects the visual color theme from 9 built-in themes. Change themes with one of two methods:

        -   **Quick cycle**: Press `Left` or `Right` to cycle through themes with a live preview.
        -   **Full list**: Press `Enter` to open the theme selector popup and select a theme from the list.

        Available themes include Solarized Dark, Solarized Light, Dracula (default), Gruvbox Dark, Gruvbox Light, Monokai, Nord, Catppuccin Mocha, and Catppuccin Latte. See [Themes](themes.md) for screenshots and custom theme details.

## Document Settings

The **Document Settings** dialog configures binary analysis and code export rules for the active project. Press ++alt+d++ or ++ctrl+shift+d++ to open this dialog.

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
│  Target System: < Commodore 64 >                                       │
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

    The **System Labels**, **Exclude well-known addresses**, and **Show system comments** options depend on the selected target system. These options appear only when the target system provides the corresponding asset data.

### Options

1.  **Display External Labels at top**

    -   **Description**: Displays external label definitions at the top of the Disassembly View. Exported assembly files always include all external labels regardless of this setting.

2.  **Preserve long bytes**

    -   **Description**: Keeps 3-byte absolute instructions at 3 bytes during re-assembly. Without this option, the assembler can optimize them into 2-byte zero-page instructions. The formatter adds explicit width prefixes (`@w`, `+2`, or `.abs`) to preserve the exact byte count of the original binary.

3.  **BRK single byte**

    -   **Description**: Treats `BRK` as a 1-byte instruction (enabled by default). Although the 6502 hardware increments the return address past a 2nd padding byte, most C64 programs treat `BRK` as 1 byte. If the binary uses the 2nd signature byte after `BRK`, clear this option.

        When `BRK single byte` is enabled, the disassembler formats four `$00` bytes as:

        ```asm
        ; These bytes will be disassembled as:
        ; $00, $00, $00, $00
        ; Each BRK consumes only one byte
        $c000   brk
        $c001   brk
        $c002   brk
        $c003   brk
        ```

4.  **Patch BRK**

    -   **Description**: Emits an explicit `.byte` directive for the padding byte after `BRK` when `BRK single byte` is disabled. This preserves the binary layout on assemblers that assemble `BRK` as a 1-byte instruction.
    -   Not all assemblers support 2-byte `BRK #$xx` syntax when `Patch BRK` is disabled.
    -   This option is inactive when `BRK single byte` is enabled, or when the assembler is **KickAssembler** or **ca65**.

        When `Patch BRK` is enabled, the disassembler formats four `$00` bytes as:

        ```asm
        ; These bytes will be disassembled as:
        ; $00, $00, $00, $00
        ; Each BRK consumes two bytes (BRK + byte data)
        $c000   brk
        $c001   .byte $00
        $c002   brk
        $c003   .byte $00
        ```

        When `Patch BRK` is disabled, the disassembler formats four `$00` bytes as:

        ```asm
        ; These bytes will be disassembled as:
        ; $00, $00, $00, $00
        ; Each BRK consumes two bytes
        $c000   brk #$00
        $c002   brk #$00
        ```

5.  **Use Illegal Opcodes**

    -   **Description**: Decodes undocumented (illegal) 6502 opcodes as instructions. If you disable this option, the disassembler emits raw `.byte` directives for undocumented opcodes.

        When `Use Illegal Opcodes` is disabled, the disassembly appears as follows:

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

        When `Use Illegal Opcodes` is enabled, the disassembly appears as follows:

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

6.  **Auto-generate Labels & Cross-refs**

    -   **Description**: Runs the code analyzer automatically on project load and state changes to generate labels and cross-references.

7.  **Description**

    -   **Description**: Stores a short text note for the project. Press `Enter` to edit the text. Press `Enter` again to save. Press `Esc` to cancel.

8.  **Max X-Refs**

    -   **Description**: Sets the maximum number of cross-reference addresses shown in the side comment of a single line.

9.  **Arrow Columns**

    -   **Description**: Sets the number of columns reserved on the left margin of the Disassembly View for branch and jump arrows. Increase this value to display deeply nested control flow.

10. **Text Line Limit**

    -   **Description**: Sets the maximum number of characters emitted on a single line for `PETSCII Text` and `Screencode Text` blocks.

11. **Words/Addrs per line**

    -   **Description**: Sets the number of 16-bit values (`Word` or `Address`) displayed on a single line (range `1` to `8`).

12. **Bytes per line**

    -   **Description**: Sets the number of 8-bit values (`Byte`) displayed on a single line (range `1` to `40`).

13. **Fill run threshold**

    -   **Description**: Collapses `N` or more consecutive identical bytes inside a `Byte` block into a single assembler fill directive. Set this value to `0` to disable fill compression (default: `8`).

        Each assembler uses a specific fill directive syntax:

        | Assembler | Directive syntax |
        |---|---|
        | 64tass | `.fill count, value` |
        | ACME | `!fill count, value` |
        | KickAssembler | `.fill count, value` |
        | ca65 | `.res count, value` |

        **Example** — 16 consecutive `$00` bytes (64tass format):

        ```asm
        ; Without fill run threshold (or threshold > 16):
        .byte $00, $00, $00, $00, $00, $00, $00, $00
        .byte $00, $00, $00, $00, $00, $00, $00, $00

        ; With fill run threshold <= 16:
        .fill 16, $00
        ```

        The formatter combines runs only within a single `Byte` block. Labels, cross-references, splitters, or comments inside the block split the run at that address.

14. **Assembler**

    -   **Description**: Selects the target assembler syntax (**64tass**, **ACME**, **KickAssembler**, or **ca65**) for the Disassembly View and `.asm` export. Press `Enter` to open the selector popup. Press `Left` and `Right` to cycle.

15. **Target System**

    -   **Description**: Selects the target hardware platform (such as Commodore 64) to load platform-specific memory maps, hardware registers, and ROM routines. Press `Enter` to open the selector popup. Press `Left` and `Right` to cycle. If you change the target system, Regenerator 2000 resets the active **System Labels** selections.

16. **System Labels** *(system-dependent)*

    -   **Description**: Selects which predefined system label sets (such as **KERNAL**, **BASIC**, or **I/O**) load for the active target system. Press `Space` or `Enter` to toggle a label set.

17. **Exclude well-known addresses from symbolic analysis** *(system-dependent)*

    -   **Description**: Excludes predefined system label ranges from the code-walking pass of the analyzer. This prevents false-positive labels inside ROM routines and hardware register ranges.

18. **Show system comments** *(system-dependent)*

    -   **Description**: Displays predefined hardware and ROM descriptions (such as "Border Color" for `$D020` on the C64) as side comments in the Disassembly View.
