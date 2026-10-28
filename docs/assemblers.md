# Supported Assemblers

Regenerator 2000 generates assembly code for four 6502 assemblers. This page lists the supported assemblers and the commands to assemble the exported output.

## 64tass

**64tass** is a multi-pass optimizing macro assembler for the 65xx processor family.

- **Website**: [https://sourceforge.net/projects/tass64/](https://sourceforge.net/projects/tass64/)

### Command Line

Assemble a file generated for 64tass:

```bash
64tass -o output.prg input.asm
```

- `-o`: Specify the output filename.

## ACME

**ACME** is a cross-assembler for the 6502, 6510, and 65816 processors.

- **Website**: [https://sourceforge.net/projects/acme-crossass/](https://sourceforge.net/projects/acme-crossass/)

### Command Line

Assemble a file generated for ACME:

```bash
acme -f cbm -o output.prg input.asm
```

- `-f cbm`: Set the output format to CBM (which includes the 2-byte load address).
- `-o`: Specify the output filename.

## KickAssembler

**KickAssembler** is a Java-based 6502 assembler with a built-in scripting language. It requires a Java Runtime Environment (JRE).

- **Website**: [http://www.kickassembler.com/](http://www.kickassembler.com/)

### Command Line

Assemble a file generated for KickAssembler:

```bash
java -jar kickass.jar input.asm
```

KickAssembler produces a `.prg` output file by default.

!!! tip "Environment Variable"

    Because KickAssembler is a `.jar` file, `--verify` requires its file path. Set the **`KICKASS_JAR`** environment variable to the full path of `KickAss.jar`:

    === "macOS / Linux"

        ```bash
        export KICKASS_JAR="$HOME/bin/KickAss.jar"
        ```

        Add this line to `~/.zshrc` or `~/.bashrc` to persist the setting.

    === "Windows"

        ```powershell
        setx KICKASS_JAR "C:\Tools\KickAssembler\KickAss.jar"
        ```

    If you do not set `KICKASS_JAR`, Regenerator 2000 searches for `KickAss.jar` in the current working directory.

## ca65

**ca65** is the macro assembler included in the **cc65** compiler suite.

- **Website**: [https://cc65.github.io/](https://cc65.github.io/)

### Command Line

Regenerator 2000 generates a single assembly file. Use the `cl65` utility from cc65 to assemble and link the file in one step:

```bash
cl65 -t c64 -C c64-asm.cfg -o output.prg input.asm
```

- `-t c64`: Set the target platform to Commodore 64.
- `-C c64-asm.cfg`: Use the default linker configuration file for C64 assembly.
- `-o`: Specify the output filename.

If the program origin is `$0801`, this command works without modification. The default configuration file [c64-asm.cfg][c64-asm.cfg] sets the start address to `$0801`:

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

If your binary uses a different origin, create a custom configuration file. Copy `c64-asm.cfg` and update `STARTADDRESS`. See the "Configuration Files" section in the [ld65 documentation][cc65-docs].

[c64-asm.cfg]: https://github.com/cc65/cc65/blob/master/cfg/c64-asm.cfg
[cc65-docs]: https://cc65.github.io/doc/ld65.html#s5

---

## Assembler Comparison

Regenerator 2000 translates disassembly structures into the syntax required by each target assembler.

### Fill Directives

If a memory range contains identical values and exceeds the **Fill run threshold** in Document Settings, Regenerator 2000 emits a fill directive:

| Assembler      | Directive       | Example syntax        |
| :------------- | :-------------- | :-------------------- |
| 64tass         | `.fill`         | `.fill 10, $00`       |
| ACME           | `!fill`         | `!fill 10, $00`       |
| ca65           | `.res`          | `.res 10, $00`        |
| KickAssembler  | `.fill`         | `.fill 10, $00`       |

### Scope Directives

Scopes isolate local label names (such as `loop` or `next`) so you can reuse them across routines without name collisions:

| Assembler      | Start Directive | End Directive    | Notes                    |
| :------------- | :-------------- | :--------------- | :----------------------- |
| 64tass         | `.block`        | `.bend`          |                          |
| ACME           |                 |                  | Not supported (ignored)  |
| ca65           | `.proc`         | `.endproc`       |                          |
| KickAssembler  | `{`             | `}`              | Label precedes `{`       |

---

## Verifying Exports

Regenerator 2000 includes a **roundtrip verification** mode. This mode exports your project to all four assemblers, assembles each file, and compares the output byte-for-byte against the original binary.

### Usage

```bash
regenerator2000 --verify my_project.regen2000proj
```

The `--verify` flag enables `--headless` automatically and accepts only `.regen2000proj` files.

### Requirements

Install the target assemblers and add them to your `PATH`:

| Assembler      | Binary / Command | Notes                                                        |
| :------------- | :--------------- | :----------------------------------------------------------- |
| 64tass         | `64tass`         | Must be in `PATH`                                            |
| ACME           | `acme`           | Must be in `PATH`                                            |
| ca65 (cc65)    | `cl65`           | Must be in `PATH`                                            |
| KickAssembler  | `java -jar ...`  | Requires `KICKASS_JAR` env var (see [above](#kickassembler)) |

If an assembler is missing, Regenerator 2000 skips it. A missing assembler does not cause the verification run to fail. At least one assembler must be available for verification to succeed.

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
