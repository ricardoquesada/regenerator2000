---
icon: material/download
description: Install Regenerator 2000 via pre-compiled binaries, Cargo, or source, and explore supported Commodore file formats and CLI options.
---

# Installation and Usage

## Installation

=== "Pre-compiled Binaries"

    Download pre-compiled binaries for **Linux**, **macOS**, and **Windows** from the [latest GitHub Release](https://github.com/ricardoquesada/regenerator2000/releases/latest) and place `regenerator2000` in your `PATH`.

=== "Crates.io (Cargo)"

    ```bash
    cargo install regenerator2000
    ```

=== "From Source"

    ```bash
    git clone https://github.com/ricardoquesada/regenerator2000.git
    cd regenerator2000
    cargo install --path .
    ```

!!! tip "Quick Start with AI Agents"

    Analyzing binaries with Antigravity or Claude Code? Start from the [regenerator2000-template](https://github.com/ricardoquesada/regenerator2000-template) repository—it comes preconfigured with the required agent skills and MCP permissions.

---

## Usage

Launch Regenerator 2000 with an optional file path or flags:

```bash
regenerator2000 [OPTIONS] [FILE]
```

### Supported File Formats

| Format | Behavior on Load |
| :--- | :--- |
| `.prg` | Standard Commodore program. Parses BASIC `SYS` stubs automatically as the entry point. |
| `.crt` | C64 cartridge image. Parses `CHIP` packets into memory and supports bank selection. |
| `.d64` | 35/40/42-track C64 disk image. Opens a picker to choose a `.prg` from the container. |
| `.d71` | 70/80-track C128/1571 double-sided disk image with `.prg` container picker. |
| `.d81` | 80-track 1581 disk image with `.prg` container picker. |
| `.t64` | C64 tape container image with `.prg` picker. |
| `.vsf` | VICE snapshot. Extracts the 64 KB RAM image and sets `PC` as the entry point. |
| `.dis65` | Imports 6502bench SourceGen project files. |
| `.bin`, `.raw` | Raw binary without load header. Prompts for System, Origin, and Entry Point via **Import Context**. |
| `.regen2000proj` | Regenerator 2000 project file containing binary, labels, comments, blocks, and undo history. |

### Command-Line Options

#### Files & Labels

`--help`
:   Print all options, file types, and your platform's configuration directory path.

`--version`
:   Print the current Regenerator 2000 version.

`--import_lbl <PATH>`
:   Import VICE `.lbl` symbol labels from `<PATH>`.

`--export_lbl <PATH>`
:   Export symbols to a VICE `.lbl` file after loading or analysis.

#### Headless Export & Verification

`--headless`
:   Run without the TUI (requires a `.regen2000proj` file; cannot combine with `--mcp-server`).

`--export_asm <PATH>`
:   Export assembly source to `<PATH>` (headless mode accepts `.regen2000proj` only).

`--export_html <PATH>`
:   Export interactive HTML disassembly with clickable X-Refs to `<PATH>`.

`--assembler <NAME>`
:   Override the target assembler (`64tass`, `acme`, `ca65`, or `kick`). Defaults to the saved project setting.

`--verify`
:   Run roundtrip export verification across all available assemblers (implies `--headless`). See [Target Assemblers](assemblers.md).

#### MCP & Live Debugging

`--mcp-server`
:   Start the HTTP/SSE MCP server on port `3000` (cannot combine with `--headless` on non-project inputs; see [MCP Integration](mcp.md)).

`--mcp-server-stdio`
:   Start the headless MCP server over stdio for subprocess clients.

`--vice <HOST:PORT>`
:   Auto-connect to the VICE binary monitor at startup (e.g. `localhost:6502`). See [Live Debugger](debugger.md).

#### Dump Built-in Configs

`--dump-system-config-files <PATH>`
:   Write built-in `system-*.toml` definitions to `<PATH>` and exit. See [Target Systems](systems.md#custom-systems).

`--dump-theme-files <PATH>`
:   Write built-in `theme-*.toml` palettes to `<PATH>` and exit. See [Themes](themes.md#custom-themes).

`--dump-enum-files <PATH>`
:   Write built-in `enum-*.toml` hardware constants to `<PATH>` and exit. See [Custom Enums](enums.md#custom-global-enums).

---

## Recommended Terminals

!!! note

    Use a modern true-color terminal so themes, bitmap graphics protocols, and `Ctrl`/`Alt` chords render accurately.

| Platform | Recommended Terminals |
| :--- | :--- |
| **macOS** | [iTerm2][iterm2_url], [Ghostty][ghostty_url], [kitty][kitty_url], [WezTerm][wezterm_url] |
| **Linux** | [Ghostty][ghostty_url], [kitty][kitty_url], [WezTerm][wezterm_url] |
| **Windows** | [Windows Terminal][windows_terminal_url], [WezTerm][wezterm_url] |

[ghostty_url]: https://ghostty.org/
[iterm2_url]: https://iterm2.com/
[kitty_url]: https://sw.kovidgoyal.net/kitty/
[wezterm_url]: https://wezfurlong.org/wezfurlong.html
[windows_terminal_url]: https://github.com/microsoft/terminal
