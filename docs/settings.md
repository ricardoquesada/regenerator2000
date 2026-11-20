---
icon: material/cog-outline
description: Configure global Program Settings (view sync, entropy warning, themes) and per-project Document Settings (BRK handling, illegal opcodes, formatting).
---

# Settings

Regenerator 2000 splits configuration into global **Program Settings** (saved across sessions) and per-file **Document Settings** (saved inside `.regen2000proj`).

---

## Program Settings

Open with **File → Settings** or press ++alt+p++ (or ++ctrl+comma++).

### View Sync

Clicking or navigating inside **Hex Dump**, **Sprites**, **Charset**, or **Bitmap** always moves the **Disassembly View** cursor to match. Enabling any sync toggle below makes navigation **two-way**, keeping that companion view aligned as you move through the disassembly:

| Setting | Shortcut | Synchronizes |
| :--- | :--- | :--- |
| **Sync Blocks View** | ++ctrl+1++ | Blocks structural overview pane |
| **Sync Hex Dump View** | ++ctrl+2++ | Hex dump pane |
| **Sync Sprites View** | ++ctrl+3++ | 24×21 sprite viewer pane |
| **Sync Charset View** | ++ctrl+4++ | 8×8 character tile pane |
| **Sync Bitmap View** | ++ctrl+5++ | Full-screen bitmap pane |

### General, Entropy & Theme

| Setting | Control | Behavior |
| :--- | :--- | :--- |
| **Open the latest file on startup** | Checkbox | Reopens your most recent project automatically on launch. |
| **Default is Unexplored (for new files)** | Checkbox | Marks all newly loaded bytes as `Undefined` instead of `Code` for manual tracing. |
| **Check for updates** | Checkbox | Checks for new releases at startup and notes newer versions in the top-right corner. |
| **Entropy Warning Threshold** | ++left++ / ++right++ (`0.1` steps, `0.0`–`8.0`) | Flags likely packed or encrypted binaries on load when Shannon entropy meets or exceeds the threshold (default `7.4`). |
| **Theme** | ++left++ / ++right++ (preview) · ++enter++ (list) | Picks from 9 built-in 24-bit color themes (default **Dracula**). See [Themes](themes.md). |

---

## Document Settings

Open with **File → Document Settings** or press ++alt+d++ (or ++ctrl+shift+d++).

### Disassembly & Assembly Rules

| Setting | Effect |
| :--- | :--- |
| **Display External Labels at top** | Shows external equates at the top of the Disassembly View (exported `.asm` files always include them). |
| **Preserve long bytes** | Emits explicit width prefixes (`@w`, `+2`, `.abs`) so 3-byte absolute instructions targeting zero page do not shrink to 2-byte instructions on reassembly. |
| **BRK single byte** *(default on)* | Treats `BRK` as 1 byte (`$00`) instead of 2 bytes. Clear if your binary embeds a signature byte after `BRK` (see [BRK Handling](#brk-handling)). |
| **Patch BRK** | When `BRK single byte` is off, emits an explicit `.byte` for the padding byte so assemblers that emit 1-byte `BRK` stay byte-identical. Inactive for **KickAssembler** and **ca65**. |
| **Use Illegal Opcodes** | Decodes undocumented 6502 instructions (`lax`, `sax`, `dcp`, `anc`, etc.) instead of raw `.byte` lines (see [Illegal Opcodes](#illegal-undocumented-opcodes)). |
| **Auto-generate Labels & Cross-refs** | Runs auto-analysis on load and after structural edits to keep auto-labels and X-Refs fresh. |
| **Description** | Project note stored in `.regen2000proj` (press ++enter++ to edit, ++enter++ to save, ++escape++ to cancel). |

### Layout & Formatting

| Setting | Range / Default | Effect |
| :--- | :--- | :--- |
| **Max X-Refs** | Default `5` | Maximum caller addresses printed in a line's inline `; x-ref:` comment. |
| **Arrow Columns** | Default `6` | Columns reserved in the left margin for nested jump and branch arrows. |
| **Text Line Limit** | Default `40` | Maximum characters emitted per line in `PETSCII Text` and `Screencode Text` blocks. |
| **Words/Addrs per line** | `1`–`8` (default `5`) | Number of 16-bit entries grouped on a single `Word` or `Address` line. |
| **Bytes per line** | `1`–`40` (default `8`) | Number of 8-bit entries grouped on a single `Byte` line. |
| **Fill run threshold** | `0` = off (default `8`) | Collapses runs of `N`+ identical bytes in a `Byte` block into an assembler fill directive (see [Fill Run Threshold](#fill-run-threshold)). |

### Target System & Labels

| Setting | Control | Behavior |
| :--- | :--- | :--- |
| **Assembler** | ++left++ / ++right++ · ++enter++ (list) | Sets disassembly and export syntax (`64tass`, `ACME`, `KickAssembler`, or `ca65`). |
| **Target System** | ++left++ / ++right++ · ++enter++ (list) | Selects hardware memory map and ROM definitions (resets active **System Labels** when changed). |
| **System Labels** *(system-dependent)* | ++space++ or ++enter++ | Toggles individual built-in label groups (`KERNAL`, `BASIC`, `Lower Page`, etc.). |
| **Exclude well-known addresses** *(system-dependent)* | Checkbox | Skips noisy ROM/register ranges during symbolic code-walking analysis. |
| **Show system comments** *(system-dependent)* | Checkbox | Shows built-in hardware and ROM descriptions (e.g. `Border Color` on `$D020`) as side comments. |

!!! note "System-Dependent Options"

    **System Labels**, **Exclude well-known addresses**, and **Show system comments** appear only when the selected [Target System](systems.md) defines those asset sections.

---

## Deep-Dive Examples

### BRK Handling

Compare how four consecutive `$00` bytes disassemble under each `BRK` mode:

=== "BRK single byte (Default)"

    Each `$00` decodes as a standalone 1-byte `brk`:

    ```ca65
    $c000   brk
    $c001   brk
    $c002   brk
    $c003   brk
    ```

=== "2-byte + Patch BRK"

    Consumes 2 bytes per `BRK` and emits an explicit `.byte` for the signature/padding byte:

    ```ca65
    $c000   brk
    $c001   .byte $00 ; (1)!
    $c002   brk
    $c003   .byte $00
    ```

    1. Guarantees assemblers that emit 1-byte `brk` reassemble byte-identically.

=== "2-byte (Patch BRK off)"

    Formats the padding byte as an immediate operand (`brk #$00`, only on assemblers that support 2-byte `brk` syntax):

    ```ca65
    $c000   brk #$00
    $c002   brk #$00
    ```

### Illegal (Undocumented) Opcodes

=== "Enabled (Undocumented 6502)"

    Decodes undocumented instructions (`lax`, `dcp`, `sax`, `anc`) cleanly with full symbol and X-Ref tracking:

    ```ca65 hl_lines="2 7 9 11 14"
            sei
            lax #$00
            stx $d020        ; Border Color
            stx $d021        ; Background Color 0
            lda $d012        ; x-ref: $081b, $083b; Raster Position
            cmp #$60
            lax $0900,y      ; x-ref: $0833
            lda #$04
            dcp a_0002       ; x-ref: $082b
            bne $0829
            sax $0400
            iny
            lda #$80
            anc #$ff
    ```

=== "Disabled (Standard 6502)"

    Unsupported opcodes fall back to raw `.byte` directives and can misalign subsequent instructions:

    ```ca65 hl_lines="2-4 7-9 11-12 14-15"
            sei
            .byte $ab        ; Invalid or partial instruction
            brk
            .byte $8e
            jsr $8ed0
            lda $d012        ; Raster Position
            .byte $bf        ; Invalid or partial instruction
            brk
            .byte $09
            lda #$04
            .byte $c7        ; Invalid or partial instruction
            .byte $02        ; Invalid or partial instruction
            bne $0829
            .byte $8f        ; Invalid or partial instruction
            .byte $0b        ; Invalid or partial instruction
    ```

### Fill Run Threshold

When `N` or more consecutive identical bytes appear in a single `Byte` block (default `8`), Regenerator 2000 emits a compact fill directive:

| Assembler | Directive | 16 Zero Bytes Output |
| :--- | :--- | :--- |
| **64tass** | `.fill count, value` | `.fill 16, $00` |
| **ACME** | `!fill count, value` | `!fill 16, $00` |
| **KickAssembler** | `.fill count, value` | `.fill 16, $00` |
| **ca65** | `.res count, value` | `.res 16, $00` |

```ca65 title="16 consecutive $00 bytes (64tass)"
; Threshold disabled (0) or > 16:
.byte $00, $00, $00, $00, $00, $00, $00, $00
.byte $00, $00, $00, $00, $00, $00, $00, $00

; Threshold <= 16:
.fill 16, $00 ; (1)!
```

1. Fill compression stops at block boundaries—labels, cross-references, splitters, or comments inside the run split it automatically.
