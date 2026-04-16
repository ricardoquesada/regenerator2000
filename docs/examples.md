# Disassembly Examples

Welcome to the Regenerator 2000 examples documentation. These HTML disassemblies are generated directly by Regenerator 2000 and showcase the auto-analysis, layout parity, and modern styling capabilities of our HTML exporter.

Below are the main examples available in this project:

## Moving Tubes - Commodore 64

![screenshot](examples/c64_moving_tubes_screenshot.png)

A disassembly of [Moving Tubes][moving_tubes] intro, by Laxity.

!!! example "Disassembly"

    * [🔗 c64_moving_tubes_lxt.html](examples/c64_moving_tubes_lxt.html)

Great source to learn about different techniques (intermediate level):

- Clean code.
- How a SID player works: the SID player was disassembled in detail.
- Raster IRQ techniques
- Different tips & tricks:
    - Convert `JMP` into `BIT`, and other self-modifying code techniques.
    - Double call with `JSR` to next line
    - Background logo color in sync with SID music
    - Sine table for scrolling speed
    - Variable-width chars in scroller

Source:

- CSDB: [Moving Tubes (Laxity Intro #145)][moving_tubes]
- The original intro was packed. The disassembly contains the unpacked version.
  Unpacked with [Unp64][unp64]).

[moving_tubes]: https://csdb.dk/release/?id=259330
[unp64]: https://csdb.dk/release/?id=260619

---

## Lode Runner - Commodore PET

![screenshot](examples/pet_lode_runner_screenshot.png)

A disassembly of [Lode Runner][lode_runner_site] clone for Commodore PET, by jimbo.

!!! example "Disassembly"

    * 🔗 [pet_loderunner.html](examples/pet_loderunner.html)

Main take aways:

- Clean code.
- Great place to learn about the PET programming.

Source:

- Itchio: [Lode Runner for the PET by jimbo][lode_runner_site]
- Since the game was not packed, this is a good example of how the disassembly looks like without any post-processing.

[lode_runner_site]: https://jimbo.itch.io/lode-runner-clone-for-commodore-pet

---

## Burnin' Rubber - Commodore 64

![screenshot](examples/c64_burnin_rubber_screenshot.png)

A disassembly of the Commodore 64 ["Burnin' Rubber"][burnin_rubber_info] game.

!!! example "Disassembly"

    * 🔗 [c64_burnin_rubber.html](examples/c64_burnin_rubber.html)

Main take aways:

- The code was written in a [monitor][monitor], not with an assembler. Evidence:
    - Dead code / Dead tables: Although not uncommon to have some dead code / tables in a program, it is a lot more common when using a [monitor].
    - There is a `.T0400,07FF,2C00` [monitor] command [in the code][c64_burning_rubber_monitor_cmd]
    - Use of `JMP` opcodes in different places that are typical of monitor-based code, in contrast to the preferred use of `JSR`+`RTS` or conditional branches in assembler-written code
    - Lack of a "clean" high level architecture
- Good for learning how early C64 games were programmed using a [monitor], but not a good place to learn modern best practices.
    - Trivia: It is not possible to add comments when using a [monitor], so, the developers might have used other tools, like a notebook, to document the code ([that's what I used to do when I was a kid!][chardef_scan]).

Source:

- TAP file: [Burnin' Rubber.tap][burnin_rubber_tap]
- The game was taken from the original TAP source.
- The game was encrypted. Part of the game code was in the loader, and part was in the main program.
- The diassembly contains a single file that includes the decrypted main program with part of the loader code.
- Part of the encrypted code is still present, but not used.
- Run it with with: `SYS 4752`

[burnin_rubber_tap]: https://archive.org/download/Ultimate_Tape_Archive_V5/Ultimate_Tape_Archive_V5.zip/Ultimate_Tape_Archive_V5.0%2FBurnin%27_Rubber_%281983_Audiogenic_Ltd.%29_%5B5346%5D%2FBurnin%27_Rubber.tap
[burnin_rubber_info]: https://www.c64-wiki.com/wiki/Burnin_Rubber>
[monitor]: https://www.c64-wiki.com/wiki/Machine_Code_Monitor
[c64_burning_rubber_monitor_cmd]: examples/c64_burnin_rubber.html#L2BE0
[chardef_scan]: https://github.com/ricardoquesada/c64-c128-erasoft/blob/master/scans/chardef.pdf

---
