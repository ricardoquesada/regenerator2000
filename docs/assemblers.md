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

## ca65

**ca65** is the macro assembler included with the **cc65** compiler suite. It is a powerful tool often used for larger projects requiring linking.

- **Website**: [https://cc65.github.io/](https://cc65.github.io/)

### Command Line
Regenerator 2000 generates a single assembly file that can be assembled and linked in one step using the `cl65` utility (included with cc65).

```bash
cl65 -t c64 -C c64-asm.cfg -o output.prg input.asm
```

- `-t c64`: Set the target system to Commodore 64 (sets up default memory configuration).
- `-C c64-asm.cfg`: Uses the default configuration file for C64 assembly.
- `-o`: Specify the output filename.
