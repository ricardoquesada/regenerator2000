# Installation and usage

## Installation

### Pre-compiled binaries

Get pre-compiled binaries for Linux, macOS and Windows from
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
- `.dis65`: 6502bench SourceGen project file.
- `.bin` and `.raw`: pure binary files. Requires that the user sets the origin manually. Menu -> Edit -> Change Origin
- `.regen2000proj`: Regenerator 2000 project file

### Supported options

- `--help`: Displays the help message listing all available options and supported file types.
- `--version`: Displays the current version of Regenerator 2000.
- `--import_lbl <PATH>`: Import VICE labels from the specified file.
- `--export_lbl <PATH>`: Export labels to the specified file (after analysis/import).
- `--export_asm <PATH>`: Export assembly to the specified file (after analysis/import).
- `--export_html <PATH>`: Export HTML to the specified file (after analysis/import).
- `--assembler <NAME>`: Override the assembler format used for export. Valid values: `64tass`, `acme`, `ca65`, `kick`. If omitted, the project's saved setting is used.
- `--headless`: Run in headless mode (no TUI), useful for batch processing.
- `--verify`: Verify export roundtrip (export → assemble → diff) for all 4 assemblers. Requires `--headless` (implied automatically). See [Assemblers](assemblers.md) for details.
- `--mcp-server`: Run MCP server (HTTP on port 3000). See [MCP Integration](mcp.md) for details.
- `--mcp-server-stdio`: Run MCP server via stdio (headless mode).
- `--vice <HOST:PORT>`: Auto-connect to the VICE binary monitor at startup (e.g. `--vice localhost:6502`). See [Debugger](debugger.md) for details.
- `--dump-system-config-files <PATH>`: Dump all built-in system config files (`system-*.json`) to the specified directory and exit. The files can then be edited and placed back in the [app config directory](#system-config-files) to override or extend platform definitions.

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

## System Config Files

Regenerator 2000 ships with built-in platform definitions (`system-*.json`) for every supported system.
You can **override** any of these definitions, or **add entirely new platforms**, by placing custom JSON
files in the application config directory.

### Config directory location

The config directory is determined by the [directories](https://crates.io/crates/directories) crate and
follows each operating system's standard:

| Platform    | Config directory                                              |
| :---------- | :------------------------------------------------------------ |
| **macOS**   | `~/Library/Application Support/regenerator2000/`             |
| **Linux**   | `~/.config/regenerator2000/`                                  |
| **Windows** | `C:\Users\<User>\AppData\Roaming\regenerator2000\config\`     |

!!! tip

    You can verify the path on your machine by running `regenerator2000 --help` — the config
    directory is printed near the end of the output.

### Override rules

- Files **must** be named `system-<name>.json` (e.g. `system-commodore_64.json`).
- A file in the config directory with the **same** `platform_name` as a built-in definition
  **replaces** the built-in entirely — the built-in version is never loaded.
- A file with a **new** `platform_name` is added as an additional platform.
- Name matching is case-sensitive; spaces in the filename should be replaced with underscores.

### Getting started

The easiest way to create a custom config is to start from the built-in files:

1. **Dump** all built-in configs to a directory:

    ```bash
    # macOS
    regenerator2000 --dump-system-config-files ~/Library/Application\ Support/regenerator2000/

    # Linux
    regenerator2000 --dump-system-config-files ~/.config/regenerator2000/

    # Windows (PowerShell)
    regenerator2000.exe --dump-system-config-files $env:APPDATA\regenerator2000\config\
    ```

    This writes every `system-*.json` to the destination folder and exits.

2. **Edit** the file(s) you want to customise with any text editor.

3. **Launch** Regenerator 2000 normally — your versions are picked up automatically.

### JSON schema reference

Every `system-*.json` file has the following structure:

```json
{
    "platform_name": "My Custom Platform",
    "enabled": true,
    "labels": {
        "GROUP_NAME": {
            "ADDR_HEX": "LABEL_NAME",
            ...
        },
        ...
    },
    "comments": {
        "ADDR_HEX": "Side comment text",
        ...
    },
    "excluded": [
        "START-END",
        ...
    ]
}
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `platform_name` | string | Display name shown in the Document Settings **Platform** selector. Must be unique across all configs. |
| `enabled` | bool | If `false`, the platform is hidden from the selector. Useful for WIP configs. |
| `labels` | object | Groups of address → label mappings. Each key is a **group name** (e.g. `"KERNAL"`, `"I/O"`) that appears as a toggle in Document Settings → **System Labels**. |
| `labels.GROUP_NAME` | object | Map of hex address strings to label names. Addresses use uppercase hex **without** the `$` prefix or `0x` prefix (e.g. `"D020"` not `"$D020"`). |
| `comments` | object | Map of hex addresses to side-comment strings. When enabled via **Show system comments** in Document Settings, these appear as side comments in the disassembly. |
| `excluded` | array of strings | Address ranges the analyzer should skip (e.g. `"D000-D031"`). Ranges use `START-END` notation in uppercase hex. Enables the **Exclude well-known addresses** checkbox in Document Settings. |

!!! note

    `labels`, `comments`, and `excluded` are all optional — you can omit any section you don't need.
    An empty `labels` object means no **System Labels** checkboxes appear; an empty `excluded` array
    means the **Exclude well-known addresses** checkbox is hidden.

### Example: creating a custom platform

Below is a minimal config for a hypothetical "Acme Computer" with a video chip at `$9000–$900F`:

```json
{
    "platform_name": "Acme Computer",
    "enabled": true,
    "labels": {
        "VIDEO": {
            "9000": "VID_CTRL",
            "9001": "VID_STATUS",
            "9002": "VID_HSCROLL",
            "9003": "VID_VSCROLL",
            "900E": "VID_BORDER_COLOR",
            "900F": "VID_BG_COLOR"
        }
    },
    "comments": {
        "9000": "Video Control Register",
        "9001": "Video Status (read-only)",
        "900E": "Border Color",
        "900F": "Background Color"
    },
    "excluded": [
        "9000-900F"
    ]
}
```

Save this as `system-acme_computer.json` in the config directory. Next time you launch
Regenerator 2000, **Acme Computer** will appear in the Platform selector.

### Tips

- **Back up** the original files before editing — you can always re-dump with `--dump-system-config-files`.
- **Disable** a config temporarily by setting `"enabled": false` instead of deleting the file.
- **Validate** your JSON before launching — a syntax error will cause the file to be silently
  skipped. Use `python3 -m json.tool system-acme_computer.json` or any online JSON validator.
- Each label **group** maps to a separate checkbox in Document Settings. Use meaningful group names
  like `"KERNAL"`, `"BASIC"`, `"I/O"`, or `"SOUND"` to let users toggle sets independently.
