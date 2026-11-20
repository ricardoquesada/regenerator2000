---
icon: material/chip
description: Built-in memory maps, ROM labels, side comments, and custom TOML system definitions for 13 classic 6502-based machines.
---

# Systems

Regenerator 2000 ships with built-in definitions for **13** 6502-based systems—supplying ROM entry points, hardware register labels, inline side comments, and excluded address ranges.

Switch the active target system in **File → Document Settings** (++alt+d++ or ++ctrl+shift+d++).

---

## Supported Systems

### Enabled by Default

| System | Label Groups | Comments | Excludes |
| :--- | :--- | :---: | :---: |
| **Commodore 64** | BASIC, KERNAL, Lower Page | :material-check-circle: | :material-check-circle: |
| **Commodore 128** | KERNAL | :material-check-circle: | :material-check-circle: |
| **Commodore VIC-20** | BASIC, KERNAL, Lower Page | :material-check-circle: | :material-check-circle: |
| **Commodore Plus4** | KERNAL, Lower Page | :material-check-circle: | :material-check-circle: |
| **Commodore PET 2.0** | BASIC, KERNAL, Lower Page | :material-check-circle: | :material-check-circle: |
| **Commodore PET 4.0** | BASIC, KERNAL, Lower Page | :material-check-circle: | :material-check-circle: |
| **Commodore 1541** | — | — | — |

### Experimental (Disabled by Default)

These definitions ship with `enabled = false`—dump the built-in config files and flip `enabled = true` (see [Custom Systems](#custom-systems)) to turn them on:

| System | Label Groups | Comments | Excludes |
| :--- | :--- | :---: | :---: |
| **NES** | SYSTEM | :material-check-circle: | — |
| **Apple II** | — | :material-check-circle: | :material-check-circle: |
| **Atari 8bit** | SYSTEM | :material-check-circle: | :material-check-circle: |
| **BBC Micro** | SYSTEM | :material-check-circle: | :material-check-circle: |
| **Oric 1.0** | SYSTEM | :material-check-circle: | :material-check-circle: |
| **Oric 1.1** | SYSTEM | :material-check-circle: | :material-check-circle: |

---

## Label Groups

Each `[labels.<GROUP>]` table becomes an independent toggle checkbox under **System Labels** in Document Settings:

**KERNAL**
:   ROM entry points and operating system jump tables (enabled by default).

**BASIC**
:   BASIC interpreter routines, tokens, and internal tables.

**Lower Page**
:   Zero-page and page-one OS variables.

**SYSTEM**
:   Hardware registers and memory-mapped I/O addresses.

---

## Custom Systems

Drop a `system-*.toml` file into your configuration directory to extend an existing machine or define a new 6502 target:

--8<-- "config_dir.md"

### Creating from Built-in Templates

1. Dump all built-in system files to inspect or override them:

    === "macOS"

        ```bash
        regenerator2000 --dump-system-config-files ~/Library/Application\ Support/regenerator2000/
        ```

    === "Linux"

        ```bash
        regenerator2000 --dump-system-config-files ~/.config/regenerator2000/
        ```

    === "Windows (PowerShell)"

        ```powershell
        regenerator2000.exe --dump-system-config-files $env:APPDATA\regenerator2000\config\
        ```

2. Create or edit a `system-<name>.toml` file—click the `+` pins below to inspect each field:

    ```toml title="system-acme_computer.toml"
    system_name = "Acme Computer" # (1)!
    enabled = true                # (2)!
    excluded = ["9000-900F"]      # (3)!

    [labels.VIDEO]                # (4)!
    "9000" = "VID_CTRL"
    "9001" = "VID_STATUS"
    "900E" = "VID_BORDER_COLOR"
    "900F" = "VID_BG_COLOR"

    [comments]                    # (5)!
    "9000" = "Video Control Register"
    "9001" = "Video Status (read-only)"
    "900E" = "Border Color"
    "900F" = "Background Color"
    ```

    1. Unique display name in the **Target System** selector. Reuse a built-in name (such as `"Commodore 64"`) to **replace** the built-in definition.
    2. Set `false` to hide the system from the selector without deleting the file.
    3. Optional uppercase `START-END` hex ranges skipped during symbolic analysis (hides the **Exclude well-known addresses** checkbox if omitted).
    4. Optional group table—keys are uppercase hex addresses without `$` or `0x`. Each group gets a checkbox in Document Settings.
    5. Optional side-comment table displayed when **Show system comments** is checked.

3. Restart Regenerator 2000—**Acme Computer** appears in Document Settings automatically.

/// details | Complete TOML Schema Reference & Naming Rules
    type: info

| Field | Type | Description |
| :--- | :--- | :--- |
| `system_name` | `string` | **Required.** Unique display name in Document Settings → **Target System**. |
| `enabled` | `bool` | Hides the system from the selector when `false`. |
| `labels` | `table` | Nested `[labels.GROUP_NAME]` tables for toggleable symbol groups. |
| `labels.GROUP_NAME` | `table` | Map of uppercase hex strings (`"D020"`) to symbol identifiers. |
| `comments` | `table` | Map of uppercase hex strings (`"D020"`) to inline side-comment text. |
| `excluded` | `array[string]` | Address ranges (`"START-END"`) ignored during symbolic code analysis. |

- File names follow `system-<name>.toml` (spaces replaced by underscores); `system_name` matching is case-sensitive.
- Legacy `platform-*.toml` and `platform-*.json` files remain supported for backward compatibility.
- Regenerator 2000 logs TOML syntax errors and skips invalid files rather than crashing.
- Back up customized files before re-running `--dump-system-config-files`.
///
