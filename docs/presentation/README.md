# Regenerator 2000 — Talk & Live Demo Guide

This directory contains the [presenterm](https://mfontanini.github.io/presenterm/) slide deck (`transmission64_2026.md`) and a step-by-step **20-Minute Live Demo Playbook** designed for **Transmission64 2026**.

---

## Running the Slides (`~5 Minutes`)

Use a terminal with inline image support (**Ghostty**, **Kitty**, **iTerm2**, or **WezTerm**) so the embedded screenshots and logo render at full resolution:

```bash
# Run the presentation from the repository root (or inside docs/presentation/)
presenterm docs/presentation/transmission64_2026.md

# Or in presentation mode (hides slide index until needed, full-screen focus)
presenterm -p docs/presentation/transmission64_2026.md
```

### Optional: Speaker Notes in a Second Terminal Window

Every slide in `transmission64_2026.md` includes `<!-- speaker_note: ... -->` talking points. To view them live on a secondary monitor or split pane while presenting:

```bash
# Terminal 1 (Projector / Shared Screen):
presenterm -P docs/presentation/transmission64_2026.md

# Terminal 2 (Laptop Screen / Speaker Notes):
presenterm -l docs/presentation/transmission64_2026.md
```

### Useful `presenterm` Controls During the Talk

| Key | Action |
| :--- | :--- |
| `l` / `Right` / `Space` | Next slide |
| `h` / `Left` / `Backspace` | Previous slide |
| `gg` / `G` | Jump to first / last slide |
| `<number>G` | Jump directly to slide `<number>` (e.g., `4G`) |
| `Ctrl+P` | Open slide index modal |
| `q` | Quit presentation |

---

## 20-Minute Live Demo Playbook

After Slide 9 (**Live Demo Time!**), switch to a pre-sized terminal window (`120x35` or larger with a crisp retro-friendly font) to walk through these 5 acts.

### Act 1: Instant Startup, Unpacking & Orientation (`~3 mins`)

1. **Launch a Packed Binary (or `.d64` / `.crt`)**:
   - Open a classic packed intro or game `.prg` (e.g., an Exomizer, TSCrunch, or ByteBoozer packed file) or pick a file from a `.d64` disk image via the container picker.
   - **Talking point**: Point out how fast it launches (`< 10 ms`) and how the built-in 2-phase 6502 unpacker automatically runs the decruncher in a sandbox, strips the depacker stub, and drops the cursor straight onto the real unpacked entry point.
2. **Minimap & Blocks View (`Alt+1`)**:
   - Point out the **Minimap bar** at the top of the screen showing Code (Blue), Data (Green/Yellow), and Undefined (Grey) across the binary.
   - Press `Alt+1` to toggle the **Blocks View** on the right, use `Tab` to focus it, jump to an unanalyzed block with `Enter`, or click directly on the Minimap bar with the mouse.

### Act 2: Hunting Graphics & Text Assets (`~4 mins`)

1. **Hex Dump & Text Decoding (`Alt+2`)**:
   - Press `Alt+2` to open the side-by-side **Hex Dump**.
   - Highlight the value-based byte coloring (zeros, `$FF`s, and repeating patterns jump out visually).
   - Press `Tab` to focus the Hex Dump and press `m` to cycle between **PETSCII** and **Screencode** text until you spot a scrolltext or highscore table.
   - Press `Enter` to jump the Disassembly cursor to that exact address, then select the text (`Shift+V` + `j`/`k` or mouse drag) and press `p` (PETSCII) or `s` (Screencode) to format it!
2. **Sprites (`Alt+3`), Charset (`Alt+4`), and Bitmap (`Alt+5`)**:
   - Press `Alt+3` to open the **Sprites View**. Scroll through memory, press `m` to toggle **Multicolor mode**, and click/press `Enter` on a sprite to jump to its address in the disassembly.
   - Press `Alt+4` to inspect the **8×8 Charset** (`m` for multicolor).
   - Press `Alt+5` to show the **Bitmap View** rendering full 320×200 hi-res / 160×200 multicolor graphics right inside the terminal using Unicode quadrant blocks.

### Act 3: High-Speed Keyboard Disassembly (`~6 mins`)

1. **Following Control Flow**:
   - Return to the entry point. Follow `jsr` / `jmp` instructions by pressing `Enter`, and pop back with `Backspace`.
   - Show the left-margin **control-flow arrows** for local branches and loops.
2. **Single-Key Formatting & Undo**:
   - Find a block of raw bytes that is actually code (e.g., an unreferenced raster IRQ handler) and press `c` (or `d` to flow-disassemble).
   - Intentionally press `b` on code to turn it into bytes, then press `u` (**Undo**) to show instant, fearless undo/redo (`Ctrl+R`).
3. **6502-Specific Superpowers (Immediates, Split Tables, Scopes & Enums)**:
   - Find an IRQ setup loading `#<irq` and `#>irq` into `$0314`/`$0315` or `$FFFE`/`$FFFF`. Press `[` on the low byte and `]` on the high byte to link them into symbolic `#<label` / `#>label` operands!
   - Find a split Lo/Hi jump or raster table, select it in Visual Mode (`Shift+V`), and press `<` (Lo/Hi Address Table) or `>` (Hi/Lo Address Table).
   - Press `l` on a subroutine to give it a meaningful name (e.g., `raster_irq_top`), `;` to add a side comment, `i` to toggle immediate format (`$hex` / `dec` / `%bin`), and `r` to wrap a subroutine in a lexical scope so local labels stay clean.
   - Press `Ctrl+X` to inspect all typed **Cross-References** (`Read`, `Write`, `Call`, `Jump`) to a hardware register or zero-page pointer.

### Act 4: Live VICE Debugging (`~4 mins`)

1. **Connect to VICE**:
   - Have `x64sc -binarymonitor demo.prg` running alongside the terminal.
   - Press `Alt+6` to open the **Debugger Panel** (or connect via `Alt+U` → **Connect to VICE...**).
2. **Breakpoints, Stepping & Hardware State**:
   - Navigate to the raster IRQ routine in the Disassembly pane and press `F2` to set a breakpoint.
   - Show the Debugger pane pausing at the breakpoint: point out the **CPU registers**, **Stack**, **VIC-II / CIA registers**, and **`$00`/`$01` banking port**.
   - Step a few instructions with `F7` (Step Into) and `F8` (Step Over) to show **delta highlighting** (changed registers and hardware state lighting up).
   - Focus the Debugger pane (`Tab`), press `m`, and enter `0400` or `D020` to watch a live 64-byte **Memory Dump** update on each step. Press `F9` to resume execution.

### Act 5: Multi-Assembler Export & Byte-Exact Verification (`~3 mins`)

1. **Switch Target Assembler Live (`Alt+D`)**:
   - Press `Alt+D` (**Document Settings**) and switch the target assembler between **64tass**, **ACME**, **KickAssembler**, and **ca65** — watch the disassembly syntax update immediately in place!
2. **Export & Roundtrip Verify (`Ctrl+E` & `--verify`)**:
   - Save the project (`Ctrl+S`) and export to `.asm` (`Ctrl+E`).
   - Drop to the shell (or a split pane) and run:
     ```bash
     regenerator2000 --verify demo.regen2000proj
     ```
   - Show that Regenerator 2000 exports the `.asm`, runs the real assembler, and confirms a **100% byte-for-byte match** against the original binary.
   - Wrap up by taking audience questions!
