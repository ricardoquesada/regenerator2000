# Installation and usage

## Installation

### Pre-compiled binaries

Get pre-compiled binaries for Linux, macOS and Windows form
here: https://github.com/ricardoquesada/regenerator2000/releases/latest

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
regenerator2000 [path/to/file.prg]
```

Supported file formats:

* `.prg`: the common Commodore 8-bit program, where the first two bytes indicates the start address.
* `.crt`: Commodore 64 cartridge files. It parses the CHIP packets and maps them into memory.
* `.t64`: Commodore 64 tape image files. It extracts the first program found in the container.
* `.vsf`: VICE snapshot files. It extracts the 64KB RAM and uses the Program Counter (PC) as the start address.
* `.bin` and `.raw`: pure binary files. Requires that the user sets the origin manually. Menu -> Edit -> Change Origin
* `.regen2000proj`: Regenerator 2000 project file

### Recommended Terminals

To ensure the best experience, especially regarding keyboard shortcuts and rendering, we recommend using a modern
terminal.

| Platform    | Recommended Terminals                                                                                                                |
|:------------|:-------------------------------------------------------------------------------------------------------------------------------------|
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

