# Supported Assemblers

Regenerator 2000 can generate assembly code compatible with several popular 6502 assemblers.
This page lists the supported assemblers and the typical commands used to assemble the generated output.

## 64tass

**64tass** is a multi-pass optimizing macro assembler for the 65xx series of processors. It is known for its speed and advanced features.

- **Website**: [https://sourceforge.net/projects/tass64/](https://sourceforge.net/projects/tass64/)

### Command Line

To assemble a file generated for 64tass:

```bash
64tass -o output.prg input.asm
```

- `-o`: Specify the output filename.

## ACME

**ACME** is a cross-assembler that supports the 6502, 6510, 65816, and other processors. It has been a standard in the C64 scene for many years.

- **Website**: [https://sourceforge.net/projects/acme-crossass/](https://sourceforge.net/projects/acme-crossass/)

### Command Line

To assemble a file generated for ACME:

```bash
acme -f cbm -o output.prg input.asm
```

- `-f cbm`: Set the output format to CBM (includes load address).
- `-o`: Specify the output filename.

## KickAssembler

**KickAssembler** is a popular Java-based assembler that features a powerful scripting language. It requires a Java Runtime Environment (JRE).

- **Website**: [http://www.kickassembler.com/](http://www.kickassembler.com/)

### Command Line

To assemble a file generated for KickAssembler:

```bash
java -jar kickass.jar input.asm
```

KickAssembler automatically produces a `.prg` file by default.

!!! tip "Environment Variable"

    Because KickAssembler is a `.jar` file (not a standalone binary), `--verify` needs to know its
    absolute path. Set the **`KICKASS_JAR`** environment variable to the full path of the jar:

    === "macOS / Linux"

        ```bash
        export KICKASS_JAR="$HOME/bin/KickAss.jar"
        ```

        Add the line above to your `~/.zshrc`, `~/.bashrc`, or equivalent to make it permanent.

    === "Windows"

        ```powershell
        setx KICKASS_JAR "C:\Tools\KickAssembler\KickAss.jar"
        ```

    If `KICKASS_JAR` is not set, Regenerator 2000 falls back to `KickAss.jar` in the working
    directory.

## ca65

**ca65** is the macro assembler included with the **cc65** compiler suite. It is a powerful tool often used for larger projects requiring linking.

- **Website**: [https://cc65.github.io/](https://cc65.github.io/)

### Command Line

Regenerator 2000 generates a single assembly file that can be assembled and linked in one step using the `cl65` utility (included with cc65).

```bash
cl65 -t c64 -C c64-asm.cfg -o output.prg input.asm
```

- `-t c64`: Set the target platform to Commodore 64 (sets up default memory configuration).
- `-C c64-asm.cfg`: Uses the default configuration file for C64 assembly.
- `-o`: Specify the output filename.

As long as the origin is `$0801`, the generated assembler will work Ok.

This is because, the config file [c64-asm.cfg][c64-asm.cfg] assumes that the start address is at `$0801`:

```text
FEATURES {
    STARTADDRESS: default = $0801;
}
SYMBOLS {
    __LOADADDR__: type = import;
}
MEMORY {
    ZP:       file = "", start = $0002,  size = $00FE,      define = yes;
    LOADADDR: file = %O, start = %S - 2, size = $0002;
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

If you are disassembling a file that has another origin, you will need to create your own config file.
Just copy-paste the `c64-asm.cfg`, and make the needed changes. See ld65 "Configuration Files" section
in the [cc65 documentation][cc65-docs].

[c64-asm.cfg]: https://github.com/cc65/cc65/blob/master/cfg/c64-asm.cfg
[cc65-docs]: https://cc65.github.io/doc/ld65.html#s5

---

## Assembler Comparison

Regenerator 2000 abstracts away many assembler-specific differences, but some features use different directives depending on the target.

### Fill Directives

When a range of memory contains identical values (and exceeds the **Fill run threshold** in Document Settings), Regenerator 2000 emits a fill directive instead of individual bytes.

| Assembler      | Directive       | Example syntax        |
| :------------- | :-------------- | :-------------------- |
| 64tass         | `.fill`         | `.fill 10, $00`       |
| ACME           | `!fill`         | `!fill 10, $00`       |
| ca65           | `.res`          | `.res 10, $00`        |
| KickAssembler  | `.fill`         | `.fill 10, $00`       |

### Scope Directives

Scopes allow you to reuse label names (like `loop` or `next`) in different parts of the program without conflicts.

| Assembler      | Start Directive | End Directive    | Notes                    |
| :------------- | :-------------- | :--------------- | :----------------------- |
| 64tass         | `.block`        | `.endblock`      |                          |
| ACME           | `!zone`         |                  | Limited scope support    |
| ca65           | `.proc`         | `.endproc`       |                          |
| KickAssembler  | `.namespace`    | `.endnamespace`  |                          |

---

## Verifying Exports

Regenerator 2000 includes a **roundtrip verification** mode that exports your project to all four assemblers,
assembles each output, and compares the result byte-for-byte against the original binary. This ensures your
disassembly is accurate and complete.

### Usage

```bash
regenerator2000 --verify my_project.regen2000proj
```

The `--verify` flag implies `--headless` (no TUI is started). Only `.regen2000proj` files are supported.

### Requirements

All four assemblers must be installed and available in your `PATH`:

| Assembler      | Binary / Command | Notes                                                    |
| :------------- | :--------------- | :------------------------------------------------------- |
| 64tass         | `64tass`         | Must be in `PATH`                                        |
| ACME           | `acme`           | Must be in `PATH`                                        |
| ca65 (cc65)    | `cl65`           | Must be in `PATH`                                        |
| KickAssembler  | `java -jar ...`  | Requires `KICKASS_JAR` env var (see [above](#kickassembler)) |

If an assembler is not found, it is **skipped** (not counted as a failure). At least one assembler must
be available for verification to succeed.

### Example Output

```text
Roundtrip Export Verification
=============================
  ✓ 64tass — byte-identical (2049 bytes)
  ✓ ACME — byte-identical (2049 bytes)
  ✓ ca65 — byte-identical (2049 bytes)
  ✓ KickAssembler — byte-identical (2049 bytes)

✓ All roundtrip verifications passed.
```
