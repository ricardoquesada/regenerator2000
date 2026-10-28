# Systems

Regenerator 2000 includes built-in definitions for 13 6502-based systems. Each system definition provides **system labels** (ROM entry points, hardware registers, and zero-page variables), **side comments**, and **excluded address ranges**.

Select the target system in **File → Document Settings** (++ctrl+d++) under the **System** selector.

## Supported Systems

### Enabled by Default

These systems appear in the System selector by default:

| System                | Label Groups              | Comments | Excludes |
|:----------------------|:--------------------------|:--------:|:--------:|
| **Commodore 64**      | BASIC, KERNAL, Lower Page |    ✅     |    ✅     |
| **Commodore 128**     | KERNAL                    |    ✅     |    ✅     |
| **Commodore VIC-20**  | BASIC, KERNAL, Lower Page |    ✅     |    ✅     |
| **Commodore Plus4**   | KERNAL, Lower Page        |    ✅     |    ✅     |
| **Commodore PET 2.0** | BASIC, KERNAL, Lower Page |    ✅     |    ✅     |
| **Commodore PET 4.0** | BASIC, KERNAL, Lower Page |    ✅     |    ✅     |
| **Commodore 1541**    | —                         |    ❌     |    ❌     |

### Experimental (Disabled by Default)

Regenerator 2000 includes these systems with `enabled = false`. To enable them, dump the built-in configuration files. Then set `enabled = true` (see [Custom Systems](#custom-systems)):

| System         | Label Groups | Comments | Excludes |
|:---------------|:-------------|:--------:|:--------:|
| **NES**        | SYSTEM       |    ✅     |    ❌     |
| **Apple II**   | —            |    ✅     |    ✅     |
| **Atari 8bit** | SYSTEM       |    ✅     |    ✅     |
| **BBC Micro**  | SYSTEM       |    ✅     |    ✅     |
| **Oric 1.0**   | SYSTEM       |    ✅     |    ✅     |
| **Oric 1.1**   | SYSTEM       |    ✅     |    ✅     |

## Label Groups

Each system defines one or more **label groups** that map addresses to label names. In Document Settings, each group appears as a checkbox under **System Labels** so you can toggle it independently.

Common groups include:

- **KERNAL** — ROM entry points and kernel routines (enabled by default).
- **BASIC** — BASIC interpreter routines and tables.
- **Lower Page** — Zero-page and page-one variables.
- **SYSTEM** — Hardware registers and I/O addresses.

## Custom Systems

Place a `system-*.toml` file in the application configuration directory to override a built-in system or add a new system.

### Config Directory Location

| OS          | Config directory                                          |
|:------------|:----------------------------------------------------------|
| **macOS**   | `~/Library/Application Support/regenerator2000/`          |
| **Linux**   | `~/.config/regenerator2000/`                              |
| **Windows** | `C:\Users\<User>\AppData\Roaming\regenerator2000\config\` |

> [!TIP]
> Run `regenerator2000 --help` to verify the configuration directory path on your computer. The output prints the directory path near the bottom.

### Getting Started

Start from the built-in files to create a custom system configuration:

1. **Dump** all built-in system configuration files to your configuration directory:

    ```bash
    # macOS
    regenerator2000 --dump-system-config-files ~/Library/Application\ Support/regenerator2000/

    # Linux
    regenerator2000 --dump-system-config-files ~/.config/regenerator2000/

    # Windows (PowerShell)
    regenerator2000.exe --dump-system-config-files $env:APPDATA\regenerator2000\config\
    ```

    This command writes every `system-*.toml` file to the destination folder and exits.

2. **Edit** the target file in a text editor.

3. **Start** Regenerator 2000. The application loads your custom system definitions automatically.

### How It Works

- Name each file `system-<name>.toml` (for example, `system-commodore_64.toml`).
- If a file uses the **same** `system_name` as a built-in definition, it replaces the built-in definition.
- If a file uses a **new** `system_name`, Regenerator 2000 adds it as a new system.
- Name matching is case-sensitive. Replace spaces in the filename with underscores.
- Regenerator 2000 also supports legacy `platform-*.toml` and `platform-*.json` files for backward compatibility.

### TOML Schema Reference

Every `system-*.toml` file uses the following structure:

```toml
system_name = "My Custom System"
enabled = true
excluded = ["9000-900F"]

[labels.GROUP_NAME]
"ADDR_HEX" = "LABEL_NAME"

[comments]
"ADDR_HEX" = "Side comment text"
```

| Field               | Type             | Description                                                                                                                                      |
|:--------------------|:-----------------|:-------------------------------------------------------------------------------------------------------------------------------------------------|
| `system_name`       | string           | Display name in the Document Settings **System** selector. Must be unique across all configuration files.                                        |
| `enabled`           | bool             | If `false`, Regenerator 2000 hides the system from the selector.                                                                                 |
| `labels`            | table            | Groups of address-to-label mappings. Each key is a **group name** (such as `"KERNAL"` or `"I/O"`) in Document Settings → **System Labels**.      |
| `labels.GROUP_NAME` | table            | Map of hexadecimal address strings to label names. Format addresses in uppercase hex without `$` or `0x` (for example, `"D020"`).                |
| `comments`          | table            | Map of hexadecimal addresses to side-comment strings. Enable **Show system comments** in Document Settings to display them.                      |
| `excluded`          | array of strings | Address ranges that the analyzer skips (for example, `"D000-D031"`). Format ranges as `START-END` in uppercase hex.                              |

> [!NOTE]
> The `labels`, `comments`, and `excluded` sections are optional. If you omit `labels`, Document Settings hides the **System Labels** checkboxes. If you omit `excluded`, Document Settings hides the **Exclude well-known addresses** checkbox.

### Example: Creating a Custom System

The following configuration defines an "Acme Computer" with a video chip at `$9000–$900F`:

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

Save this file as `system-acme_computer.toml` in the configuration directory. When you restart Regenerator 2000, **Acme Computer** appears in the System selector.

### Tips

- **Copy** your custom files to a backup location before you run `--dump-system-config-files` again.
- **Disable** a configuration temporarily with `enabled = false`. You do not need to delete the file.
- **Validate** your TOML syntax before you start the application. Regenerator 2000 skips files with syntax errors and logs the error to the log file.
- Group related labels under descriptive names such as `"KERNAL"`, `"BASIC"`, `"I/O"`, or `"SOUND"` so users can toggle them independently.
