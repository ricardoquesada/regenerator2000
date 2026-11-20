---
icon: material/code-braces
description: Export and roundtrip-verify byte-identical 6502 assembly for 64tass, ACME, KickAssembler, and ca65.
---

# Supported Assemblers

Regenerator 2000 exports ready-to-assemble source code for four 6502 assemblers—and can verify byte-for-byte parity against your original binary automatically.

| Assembler | Build Command | Notes |
| :--- | :--- | :--- |
| **[64tass][tass_url]** | `64tass -o output.prg input.asm` | Supports scopes (`.block`) and `.encode` directives |
| **[ACME][acme_url]** | `acme -f cbm -o output.prg input.asm` | `-f cbm` prepends the 2-byte PRG load address |
| **[KickAssembler][kick_url]** | `java -jar kickass.jar input.asm` | Requires JRE; set `KICKASS_JAR` for `--verify` |
| **[ca65 (cc65)][ca65_url]** | `cl65 -t c64 -C c64-asm.cfg -o output.prg input.asm` | Custom linker `.cfg` needed if origin ≠ `$0801` |

[tass_url]: https://sourceforge.net/projects/tass64/
[acme_url]: https://sourceforge.net/projects/acme-crossass/
[kick_url]: http://www.kickassembler.com/
[ca65_url]: https://cc65.github.io/

---

## Per-Assembler Setup

=== "64tass"

    **64tass** is a multi-pass optimizing macro assembler for the 65xx family ([homepage][tass_url]).

    ```bash
    64tass -o output.prg input.asm
    ```

    - `-o`: Output `.prg` file path.

=== "ACME"

    **ACME** is a fast cross-assembler targeting 6502, 6510, and 65816 ([homepage][acme_url]).

    ```bash
    acme -f cbm -o output.prg input.asm
    ```

    - `-f cbm`: Emits CBM `.prg` format (including the 2-byte load header).
    - `-o`: Output `.prg` file path.

=== "KickAssembler"

    **KickAssembler** is a Java-based 6502 assembler with built-in scripting ([homepage][kick_url]).

    ```bash
    java -jar kickass.jar input.asm
    ```

    KickAssembler outputs a `.prg` file automatically.

    !!! tip "Setting `KICKASS_JAR` for `--verify`"

        Because KickAssembler runs from a `.jar`, set `KICKASS_JAR` (otherwise Regenerator 2000 looks in the current working directory):

        === "macOS / Linux"

            ```bash
            export KICKASS_JAR="$HOME/bin/KickAss.jar"
            ```

        === "Windows"

            ```powershell
            setx KICKASS_JAR "C:\Tools\KickAssembler\KickAss.jar"
            ```

=== "ca65"

    **ca65** ships with the **cc65** suite ([homepage][ca65_url]). Assemble and link Regenerator 2000's single-file output with `cl65`:

    ```bash
    cl65 -t c64 -C c64-asm.cfg -o output.prg input.asm
    ```

    If your program's origin is `$0801`, cc65's stock [c64-asm.cfg][c64-asm.cfg] works directly; for any other origin, copy it and adjust `STARTADDRESS` (see the [ld65 docs][cc65-docs]).

    /// details | Default `c64-asm.cfg` Linker Config
        type: example

    ```sh title="c64-asm.cfg"
    FEATURES {
        STARTADDRESS: default = $0801; # (1)!
    }
    SYMBOLS {
        __LOADADDR__: type = import;
    }
    MEMORY {
        ZP:       file = "", start = $0002,  size = $00FE,      define = yes;
        LOADADDR: file = %O, start = %S - 2, size = $0002; # (2)!
        MAIN:     file = %O, start = %S,     size = $D000 - %S;
    }
    SEGMENTS {
        ZEROPAGE: load = ZP,       type = zp,  optional = yes;
        LOADADDR: load = LOADADDR, type = ro;
        EXEHDR:   load = MAIN,     type = ro,  optional = yes;
        CODE:     load = MAIN,     type = rw;
        RODATA:   load = MAIN,     type = ro,  optional = yes;
        DATA:     load = MAIN,     type = rw,  optional = yes;
        BSS:      load = MAIN,     type = bss, optional = yes, define = yes;
    }
    ```

    1. Change `$0801` (or pass `--start-addr`) if your binary loads elsewhere.
    2. Reserves the 2-byte `.prg` load-address header immediately before `MAIN`.
    ///

[c64-asm.cfg]: https://github.com/cc65/cc65/blob/master/cfg/c64-asm.cfg
[cc65-docs]: https://cc65.github.io/doc/ld65.html#s5

---

## Assembler Syntax Comparison

### Fill Directives

Contiguous runs of identical bytes meeting the **Fill run threshold** in [Document Settings](settings.md) emit fill directives:

| Assembler | Directive | Example (`10` zero bytes) |
| :--- | :--- | :--- |
| **64tass** | `.fill` | `.fill 10, $00` |
| **ACME** | `!fill` | `!fill 10, $00` |
| **ca65** | `.res` | `.res 10, $00` |
| **KickAssembler** | `.fill` | `.fill 10, $00` |

### Scope Directives

Scopes isolate local label names inside subroutines (see [Block Types — Scopes](blocks.md#scopes)):

| Assembler | Start Directive | End Directive | Notes |
| :--- | :--- | :--- | :--- |
| **64tass** | `.block` | `.bend` | Scope label precedes `.block` |
| **ACME** | — | — | Not supported (omitted on export) |
| **ca65** | `.proc` | `.endproc` | Unnamed scopes default to `unnamed_scope` |
| **KickAssembler** | `{` | `}` | Scope label precedes `{` |

---

## Verifying Exports (`--verify`)

Run roundtrip verification to export your `.regen2000proj` to every installed assembler, reassemble each file, and verify byte-for-byte parity with the original binary:

```bash
regenerator2000 --verify my_project.regen2000proj
```

*(Enables `--headless` automatically. Any assembler missing from `PATH`—`64tass`, `acme`, `cl65`, or `KICKASS_JAR`—is skipped cleanly as long as at least one assembler runs.)*

```text title="Sample output"
Roundtrip Export Verification
=============================
  ✓ 64tass — byte-identical (2049 bytes)
  ✓ ACME — byte-identical (2049 bytes)
  ✓ ca65 — byte-identical (2049 bytes)
  ✓ KickAssembler — byte-identical (2049 bytes)

✓ All roundtrip verifications passed.
```
