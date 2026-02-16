# Installation and usage

## Installation

### Pre-compiled binaries

Get pre-compiled binaries for Linux, macOS and Windows form
here: <https://github.com/ricardoquesada/regenerator2000/releases/latest>

### From Crates.io

```bash
cargo install regenerator2000
```

### From Source

```bash
git clone https://github.com/ricardoquesada/regenerator2000.git
cd regenerator2000
cargo install --path .
```

## Usage

Start the application with an optional file to load:

```bash
regenerator2000 [OPTIONS] [FILE]
```

### Supported file formats

- `.prg`: the common Commodore 8-bit program, where the first two bytes indicates the start address.
- `.crt`: Commodore 64 cartridge files. It parses the CHIP packets and maps them into memory. Supports bank selection.
- `.d64`: Commodore 64 disk image files (35/40 tracks). It allows the user to pick a `.prg` file from the disk container.
- `.d71`: Commodore 64 disk image files (70 tracks, double-sided). It allows the user to pick a `.prg` file from the disk container.
- `.d81`: Commodore 64 disk image files (80 tracks). It allows the user to pick a `.prg` file from the disk container.
- `.t64`: Commodore 64 tape image files. It allows the user to pick a `.prg` from the container.
- `.vsf`: VICE snapshot files. It extracts the 64KB RAM and uses the Program Counter (PC) as the start address.
- `.bin` and `.raw`: pure binary files. Requires that the user sets the origin manually. Menu -> Edit -> Change Origin
- `.regen2000proj`: Regenerator 2000 project file

### Supported options

- `--help`: Displays the help message listing all available options and supported file types.
- `--version`: Displays the current version of Regenerator 2000.
- `--import_lbl <PATH>`: Import VICE labels from the specified file.
- `--export_lbl <PATH>`: Export labels to the specified file (after analysis/import).
- `--export_asm <PATH>`: Export assembly to the specified file (after analysis/import).
- `--headless`: Run in headless mode (no TUI), useful for batch processing.
- `--mcp-server`: Run MCP server (HTTP on port 3000). See [MCP Integration](mcp.md) for details.
- `--mcp-server-stdio`: Run MCP server via stdio (headless mode).

### Recommended Terminals

!!! Note

    To ensure the best experience, especially regarding keyboard shortcuts and rendering, we recommend using a modern
    terminal.

| Platform    | Recommended Terminals                                                                                                                |
| :---------- | :----------------------------------------------------------------------------------------------------------------------------------- |
| **Windows** | [Windows Terminal][windows_terminal_url], [Alacritty][alacritty_url], [WezTerm][wezterm_url]                                         |
| **macOS**   | [iTerm2][iterm2_url], [Ghostty][ghostty_url], [Alacritty][alacritty_url], [kitty][kitty_url], [WezTerm][wezterm_url]                 |
| **Linux**   | [Ghostty][ghostty_url], [Alacritty][alacritty_url], [kitty][kitty_url], [WezTerm][wezterm_url], [GNOME Terminal][gnome_terminal_url] |

[alacritty_url]: https://alacritty.org/
[ghostty_url]: https://ghostty.org/
[gnome_terminal_url]: https://wiki.gnome.org/Apps/Terminal
[iterm2_url]: https://iterm2.com/
[kitty_url]: https://sw.kovidgoyal.net/kitty/
[wezterm_url]: https://wezfurlong.org/wezterm/
[windows_terminal_url]: https://github.com/microsoft/terminal
