# Installation and Usage

## Installation

### Pre-compiled Binaries

Download pre-compiled binaries for Linux, macOS, and Windows from the [latest release page](https://github.com/ricardoquesada/regenerator2000/releases/latest).

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

### Quick Start with AI Agents

Use the [regenerator2000-template](https://github.com/ricardoquesada/regenerator2000-template) repository to analyze binaries with an AI agent (such as Antigravity or Claude Code). This repository includes the required skills and agent permissions so you can analyze binaries immediately.

## Usage

Start the application with an optional file path:

```bash
regenerator2000 [OPTIONS] [FILE]
```

### Supported File Formats

- `.prg`: Standard Commodore 8-bit program file. If the file includes a BASIC stub, Regenerator 2000 parses the `SYS` address as the entry point.
- `.crt`: Commodore 64 cartridge file. Regenerator 2000 parses `CHIP` packets, maps them into memory, and supports bank selection.
- `.d64`: Commodore 64 disk image file (up to 42 tracks). Select a `.prg` file from the disk container to load.
- `.d71`: Commodore 128/1571 double-sided disk image file (up to 80 tracks). Select a `.prg` file from the disk container to load.
- `.d81`: Commodore 1581 disk image file (80 tracks). Select a `.prg` file from the disk container to load.
- `.t64`: Commodore 64 tape image file. Select a `.prg` file from the tape container to load.
- `.vsf`: VICE snapshot file. Regenerator 2000 extracts the 64 KB RAM image and sets the Program Counter (PC) as the entry point.
- `.dis65`: 6502bench SourceGen project file.
- `.bin` and `.raw`: Raw binary files. Use the **Import Context** dialog to set the System, Origin, and Entry Point.
- `.regen2000proj`: Regenerator 2000 project file.

### Supported Options

- `--help`: Display the help message with all available options and file types.
- `--version`: Display the current version of Regenerator 2000.
- `--import_lbl <PATH>`: Import VICE labels from the specified file.
- `--export_lbl <PATH>`: Export labels to the specified file after analysis or import.
- `--export_asm <PATH>`: Export assembly code to the specified file. Headless mode accepts only `.regen2000proj` project files.
- `--export_html <PATH>`: Export HTML to the specified file. Headless mode accepts only `.regen2000proj` project files.
- `--assembler <NAME>`: Override the assembler format for export (`64tass`, `acme`, `ca65`, or `kick`). If you omit this flag, Regenerator 2000 uses the saved project setting.
- `--headless`: Run in headless mode without the TUI. This mode accepts only `.regen2000proj` project files and cannot run with `--mcp-server`.
- `--verify`: Verify export roundtrip (export, assemble, and compare) for all four assemblers. This flag enables `--headless` automatically. See [Assemblers](assemblers.md) for details.
- `--mcp-server`: Run the MCP server over HTTP on port 3000. Headless mode accepts only `.regen2000proj` project files. This flag cannot run with `--headless`. See [MCP Integration](mcp.md) for details.
- `--mcp-server-stdio`: Run the MCP server over stdio in headless mode.
- `--vice <HOST:PORT>`: Connect to the VICE binary monitor at startup (for example, `--vice localhost:6502`). See [Debugger](debugger.md) for details.
- `--dump-system-config-files <PATH>`: Write all built-in system configuration files (`system-*.toml`) to the specified directory and exit. Place modified files in the [application configuration directory](#system-config-files) to override or extend system definitions.
- `--dump-theme-files <PATH>`: Write all built-in theme files (`theme-*.toml`) to the specified directory and exit. Place modified files in the [configuration directory](themes.md#custom-themes) to create custom themes. See [Custom Themes](#custom-themes) for details.
- `--dump-enum-files <PATH>`: Write all built-in enum files (`enum-*.toml`) to the specified directory and exit. Place modified files in the [configuration directory](enums.md) to create custom enums. See [Custom Enums](enums.md) for details.

### Recommended Terminals

!!! Note

    Use a modern terminal to ensure full support for keyboard shortcuts and rendering.

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

## System Config Files

Regenerator 2000 includes built-in definitions for 13 different 6502-based systems. See the [Systems](systems.md) page for the complete list of supported systems and instructions to configure custom definitions.

## Custom Themes

Regenerator 2000 supports custom color themes through TOML files. See the [Themes](themes.md#custom-themes) page for instructions to create and configure custom themes.
