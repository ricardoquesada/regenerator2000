# Regenerator 2000 Talk and Demo Notes

This directory holds the [presenterm](https://mfontanini.github.io/presenterm/) slide deck (`transmission64_2026.md`) and a 20-minute live demo guide for Transmission64 2026.

## Slides (~5 min)

Use a terminal with inline image support (Ghostty, Kitty, iTerm2, or WezTerm) so screenshots and the logo render cleanly.

```bash
# Run from the repository root (or inside docs/presentation/)
presenterm docs/presentation/transmission64_2026.md

# Presentation mode (hides the slide index until needed)
presenterm -p docs/presentation/transmission64_2026.md
```

### Speaker Notes in a Second Terminal

Every slide in `transmission64_2026.md` has `<!-- speaker_note: ... -->` notes. Run two terminals to keep speaker notes on your laptop while the projector shows the slides:

```bash
# Terminal 1 (projector or shared screen)
presenterm -P docs/presentation/transmission64_2026.md

# Terminal 2 (laptop screen for speaker notes)
presenterm -l docs/presentation/transmission64_2026.md
```

### `presenterm` Controls

| Key | Action |
| :--- | :--- |
| `l` / `Right` / `Space` | Next slide |
| `h` / `Left` / `Backspace` | Previous slide |
| `gg` / `G` | Jump to first or last slide |
| `<number>G` | Jump to slide `<number>` (for example, `4G`) |
| `Ctrl+P` | Open slide index |
| `q` | Quit |

## Live Demo (~20 min)

After Slide 12 ("Live Demo Time!"), open a terminal window (`120x35` or larger) with a retro font. Cover these five sections in order.

### 1. Startup, Unpacker, and Layout (~3 min)

1. Open a packed `.prg` (for example, Exomizer, TSCrunch, or ByteBoozer). You can also pick a file from a `.d64` or `.crt` image in the container picker.
   - Note the fast startup (`< 10 ms`).
   - Show the built-in 6502 unpacker. It runs the decruncher in a sandbox, removes the depacker stub, and moves the cursor to the entry point.
2. Show the minimap bar at the top of the screen. It colors Code blue, Data green or yellow, and Undefined grey.
   - Press `Alt+1` to toggle the Blocks view on the right.
   - Press `Tab` to focus the Blocks view.
   - Press `Enter` to jump to an unanalyzed block, or click the minimap bar with the mouse.

### 2. Graphics and Text Assets (~4 min)

1. Press `Alt+2` to open the Hex Dump view next to the disassembly.
   - Note how byte-value colors make zeros, `$FF` runs, and repeated patterns clear.
   - Press `Tab` to focus the Hex Dump view.
   - Press `m` to cycle PETSCII and Screencode modes until a scrolltext or highscore table appears.
   - Press `Enter` to jump the disassembly cursor to that address.
   - Select the text with `Shift+V` and `j`/`k`, or drag with the mouse.
   - Press `p` for PETSCII or `s` for Screencode to format the block.
2. Inspect the graphics views.
   - Press `Alt+3` to open the Sprites view. Scroll the view. Press `m` to toggle multicolor mode.
   - Click a sprite or press `Enter` to jump to its address in the disassembly.
   - Press `Alt+4` to inspect the 8x8 Charset view. Press `m` for multicolor mode.
   - Press `Alt+5` to open the Bitmap view. It renders 320x200 hi-res and 160x200 multicolor graphics with Kitty, iTerm2, or Sixel (or Halfblocks fallback).

### 3. Keyboard Disassembly (~6 min)

1. Return to the entry point and trace control flow.
   - Press `Enter` on `jsr` or `jmp` instructions to jump to the target.
   - Press `Backspace` to return to the previous location.
   - Show the control-flow arrows in the left margin for local branches and loops.
2. Reclassify blocks and show undo.
   - Find raw bytes that are code (for example, an unreferenced raster IRQ handler).
   - Press `c` to mark code, or press `d` to run flow disassembly.
   - Press `b` on a code block to convert it to bytes.
   - Press `u` to undo. Press `Ctrl+R` to redo.
3. Show 6502 helpers for pointers, split tables, scopes, and cross-references.
   - Find an IRQ routine that loads a low byte and high byte into `$0314`/`$0315` or `$FFFE`/`$FFFF`.
   - Select both immediate loads to link `#<label` and `#>label`. Press `[` when the low byte comes first, or `]` when the high byte comes first.
   - Find a split Lo/Hi jump table or raster table. Select it in Visual Mode (`Shift+V`). Press `<` for a Lo/Hi address table or `>` for a Hi/Lo address table.
   - Press `l` on a subroutine to rename it (for example, `raster_irq_top`).
   - Press `;` to add a side comment.
   - Press `i` to cycle immediate formats (`$hex`, `dec`, `%bin`).
   - Press `r` to wrap a subroutine in a lexical scope to keep local labels separate.
   - Press `Ctrl+X` on a hardware register or zero-page pointer to inspect its cross-references (`Read`, `Write`, `Call`, `Jump`).

### 4. Live VICE Debugger (~4 min)

1. Start `x64sc -binarymonitor demo.prg` alongside the terminal.
   - Press `Alt+6` to open the Debugger panel, or press `Alt+U` and select `Connect to VICE...`.
2. Step instructions and inspect hardware state.
   - Move to the raster IRQ routine in the Disassembly view. Press `F2` to set a breakpoint.
   - Wait for VICE to pause at the breakpoint. Show the CPU registers, stack, VIC-II and CIA registers, and `$00`/`$01` banking port.
   - Press `F7` (Step Into) and `F8` (Step Over) to show delta highlighting on changed registers and hardware state.
   - Press `Tab` to focus the Debugger panel.
   - Press `m` and enter `0400` or `D020`. A live 64-byte memory dump updates on each step.
   - Press `F9` to resume execution.

### 5. Export and Verification (~3 min)

1. Press `Alt+D` to open Document Settings.
   - Switch the target assembler between 64tass, ACME, KickAssembler, and ca65.
   - Show the disassembly syntax update in place.
2. Export and verify the rebuild.
   - Press `Ctrl+S` to save the project.
   - Press `Ctrl+E` to export the `.asm` file.
   - Run this command in a shell or split pane:

     ```bash
     regenerator2000 --verify demo.regen2000proj
     ```

   - Show that Regenerator 2000 exports the `.asm` file, runs the assembler, and verifies a byte-for-byte match with the original binary.
   - Take audience questions.
