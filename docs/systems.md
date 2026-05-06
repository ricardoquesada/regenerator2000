# Systems

Regenerator 2000 ships with built-in system definitions for a range of 6502-based systems.
Each system provides **system labels** (ROM entry points, hardware registers, zero-page
variables), **side comments**, and **excluded address ranges** that help produce cleaner
disassembly output.

You can select the target system in **File → Document Settings** (++ctrl+d++) under the
**System** selector.

## Supported Systems

### Enabled by default

These systems are visible in the System selector out of the box:

| System | Label Groups | Comments | Excludes |
| :--- | :--- | :---: | :---: |
| **Commodore 64** | BASIC, KERNAL, Lower Page | ✅ | ✅ |
| **Commodore 128** | KERNAL | ✅ | ✅ |
| **Commodore VIC-20** | BASIC, KERNAL, Lower Page | ✅ | ✅ |
| **Commodore Plus4** | KERNAL, Lower Page | ✅ | ✅ |
| **Commodore PET 2.0** | BASIC, KERNAL, Lower Page | ✅ | ✅ |
| **Commodore PET 4.0** | BASIC, KERNAL, Lower Page | ✅ | ✅ |
| **Commodore 1541** | — | ❌ | ❌ |

### Experimental (disabled by default)

These systems are included but disabled (`enabled = false`). To use them, dump the built-in
configs and set `enabled = true` (see [Custom Systems](#custom-systems) below):

| System | Label Groups | Comments | Excludes |
| :--- | :--- | :---: | :---: |
| **NES** | SYSTEM | ✅ | ❌ |
| **Apple II** | — | ✅ | ✅ |
| **Atari 8bit** | SYSTEM | ✅ | ✅ |
| **BBC Micro** | SYSTEM | ✅ | ✅ |
| **Oric 1.0** | SYSTEM | ✅ | ✅ |
| **Oric 1.1** | SYSTEM | ✅ | ✅ |

## Label Groups

Each system can define one or more **label groups** — named sets of address-to-label mappings.
In Document Settings, each group appears as a separate checkbox under **System Labels**, letting
users toggle them independently.

Common groups include:

- **KERNAL** — ROM entry points and kernel routines (enabled by default).
- **BASIC** — BASIC interpreter routines and tables.
- **Lower Page** — Zero-page and page-one variables.
- **SYSTEM** — Hardware registers and I/O addresses.

## Custom Systems

You can **override** any built-in system or **add entirely new systems** by placing a
`system-*.toml` file in the application config directory.

### Config directory location

| OS          | Config directory                                              |
| :---------- | :------------------------------------------------------------ |
| **macOS**   | `~/Library/Application Support/regenerator2000/`             |
| **Linux**   | `~/.config/regenerator2000/`                                  |
| **Windows** | `C:\Users\<User>\AppData\Roaming\regenerator2000\config\`     |

> [!TIP]
> You can verify the path on your machine by running `regenerator2000 --help` — the config
> directory is printed near the end of the output.

### Getting started

The easiest way to create a custom config is to start from the built-in files:

1. **Dump** all built-in configs to the config directory:

    ```bash
    # macOS
    regenerator2000 --dump-system-config-files ~/Library/Application\ Support/regenerator2000/

    # Linux
    regenerator2000 --dump-system-config-files ~/.config/regenerator2000/

    # Windows (PowerShell)
    regenerator2000.exe --dump-system-config-files $env:APPDATA\regenerator2000\config\
    ```

    This writes every `system-*.toml` to the destination folder and exits.

2. **Edit** the file(s) you want to customise with any text editor.

3. **Launch** Regenerator 2000 normally — your versions are picked up automatically.

### How it works

- Files must be named `system-<name>.toml` (e.g. `system-commodore_64.toml`).
- A file with the **same** `system_name` as a built-in definition **replaces** the built-in entirely.
- A file with a **new** `system_name` is added as an additional system.
- Name matching is case-sensitive; spaces in the filename should be replaced with underscores.
- Legacy `platform-*.toml` and `platform-*.json` files are still supported for backward compatibility.

### TOML schema reference

Every `system-*.toml` file has the following structure:

```toml
system_name = "My Custom System"
enabled = true
excluded = ["9000-900F"]

[labels.GROUP_NAME]
"ADDR_HEX" = "LABEL_NAME"

[comments]
"ADDR_HEX" = "Side comment text"
```

| Field | Type | Description |
| :--- | :--- | :--- |
| `system_name` | string | Display name shown in the Document Settings **System** selector. Must be unique across all configs. |
| `enabled` | bool | If `false`, the system is hidden from the selector. Useful for WIP configs. |
| `labels` | table | Groups of address → label mappings. Each key is a **group name** (e.g. `"KERNAL"`, `"I/O"`) that appears as a toggle in Document Settings → **System Labels**. |
| `labels.GROUP_NAME` | table | Map of hex address strings to label names. Addresses use uppercase hex **without** the `$` prefix or `0x` prefix (e.g. `"D020"` not `"$D020"`). |
| `comments` | table | Map of hex addresses to side-comment strings. When enabled via **Show system comments** in Document Settings, these appear as side comments in the disassembly. |
| `excluded` | array of strings | Address ranges the analyzer should skip (e.g. `"D000-D031"`). Ranges use `START-END` notation in uppercase hex. Enables the **Exclude well-known addresses** checkbox in Document Settings. |

> [!NOTE]
> `labels`, `comments`, and `excluded` are all optional — you can omit any section you don't need.
> An empty `labels` table means no **System Labels** checkboxes appear; an empty `excluded` array
> means the **Exclude well-known addresses** checkbox is hidden.

### Example: creating a custom system

Below is a minimal config for a hypothetical "Acme Computer" with a video chip at `$9000–$900F`:

```toml
system_name = "Acme Computer"
enabled = true
excluded = ["9000-900F"]

[labels.VIDEO]
"9000" = "VID_CTRL"
"9001" = "VID_STATUS"
"9002" = "VID_HSCROLL"
"9003" = "VID_VSCROLL"
"900E" = "VID_BORDER_COLOR"
"900F" = "VID_BG_COLOR"

[comments]
"9000" = "Video Control Register"
"9001" = "Video Status (read-only)"
"900E" = "Border Color"
"900F" = "Background Color"
```

Save this as `system-acme_computer.toml` in the config directory. Next time you launch
Regenerator 2000, **Acme Computer** will appear in the System selector.

### Tips

- **Back up** the original files before editing — you can always re-dump with `--dump-system-config-files`.
- **Disable** a config temporarily by setting `enabled = false` instead of deleting the file.
- **Validate** your TOML before launching — a syntax error will cause the file to be silently
  skipped. Check the log file (path varies by OS) for parsing errors.
- Each label **group** maps to a separate checkbox in Document Settings. Use meaningful group names
  like `"KERNAL"`, `"BASIC"`, `"I/O"`, or `"SOUND"` to let users toggle sets independently.
