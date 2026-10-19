# Enums

Regenerator 2000 supports custom **Enums** for each target system. You can replace raw numeric values (immediate operands, data bytes, and data words) with named constants across your disassemblies.

Enums improve code readability by mapping numeric literals (such as color indices or state constants) to text identifiers.

---

## The Three-Tier Scope Model

Regenerator 2000 uses a three-tier model to manage enum definitions. You can reuse common hardware definitions across multiple disassemblies and define project-specific names where necessary.

```
  ┌─────────────────────────────────────────────────────────┐
  │ 1. Project-Specific Enums                               │
  │    - Stored in .regen2000proj                           │
  │    - Supports Undo/Redo                                 │
  │    - Local overrides                                    │
  └───────────┬─────────────────────────────────────────────┘
              │ (precedes)
              ▼
  ┌─────────────────────────────────────────────────────────┐
  │ 2. Global Custom Enums                                  │
  │    - Persistent across projects                         │
  │    - Stored in config folder                            │
  │    - Saved as TOML files                                │
  └───────────┬─────────────────────────────────────────────┘
              │ (precedes)
              ▼
  ┌─────────────────────────────────────────────────────────┐
  │ 3. Built-in System Enums                                │
  │    - Curated for target machine (e.g. VIC-II, PETSCII)  │
  │    - Read-only, embedded in the application             │
  └─────────────────────────────────────────────────────────┘
```

### 1. Project-Specific Enums

- **Scope**: Local to the current project file (`.regen2000proj`).
- **Purpose**: Stores definitions unique to the current binary (such as game state constants or level indices).
- **Precedence**: Highest priority. If a project enum shares a name with a global or system enum, Regenerator 2000 uses the project enum.
- **Properties**: Uses the command queue so you can undo and redo (`u` / `Ctrl+R`) enum creation, updates, and deletions.

### 2. Global Custom Enums

- **Scope**: User-wide across all projects on your computer.
- **Purpose**: Reuses custom enum definitions (such as math constants or loader codes) across multiple projects.
- **Precedence**: Medium priority. Global enums override built-in system enums, and project enums override global enums.
- **Storage**: Saved as individual `.toml` files in your global configuration directory.

### 3. Built-in System Enums

- **Scope**: Read-only system definitions.
- **Purpose**: Provides standard hardware constants. For example, if you select the Commodore 64 system, Regenerator 2000 provides the `VicIIColors` enum automatically.
- **Precedence**: Lowest priority. Regenerator 2000 uses system enums when no project or global enum overrides them.

---

## Portability and Reference Behavior

Regenerator 2000 manages enum references to keep `.regen2000proj` project files portable across machines:

- **Built-in & Global Enums**: When you apply a system or global enum to an operand, the project file stores only the enum name reference. It does not copy the full enum definition into the project file.
- **Project-Local Enums**: When you create a local project enum, Regenerator 2000 serializes the full definition into the `.regen2000proj` file.
- **Importing and Sharing**: If a shared project references a missing global enum, copy that enum into the local project pool from the **Manage Enums** dialog.

---

## Using Enums in the TUI

### Applying an Enum

Replace a raw numeric value with an enum variant:

1. Move the cursor to a line with an immediate operand (`lda #$00`), a data byte (`.byte $02`), or a data word (`.word $0001`).
2. Press **++e++** to open the **Apply Enum** dialog.
3. Select an enum from the list of definitions.
4. Select the matching variant from that enum to apply it.

The disassembler updates the line immediately to show the enum variant name (for example, `lda #VicIIColors.BLACK`).

### Managing Enums

Create, edit, clone, or delete custom enums:

1. Press **++f10++** to open the main menu (or press **++alt+t++** to open the Edit menu).
2. Select **Manage Enums...**.
3. Use the **Project**, **Global**, and **System** tabs in the dialog:
    - **Add**: Create a new local or global enum.
    - **Edit**: Update variant values, names, or descriptions.
    - **Delete**: Remove an unused enum. Regenerator 2000 blocks deletion if the disassembly uses the enum.
    - **Clone to Project/Global**: Copy a system or global enum into your project pool (or vice versa) to customize it.

---

## Custom Global Enums

Place an `enum-*.toml` file in the application configuration directory to create a global enum or override a built-in system enum.

### Config Directory Location

| Platform    | Config Directory                                          |
|:------------|:----------------------------------------------------------|
| **macOS**   | `~/Library/Application Support/regenerator2000/`          |
| **Linux**   | `~/.config/regenerator2000/`                              |
| **Windows** | `C:\Users\<User>\AppData\Roaming\regenerator2000\config\` |

### Dumping Built-in Enums

Dump the built-in system enums to use them as templates for your own global enums:

1. **Dump** all built-in enums to the configuration directory:

    ```bash
    # macOS
    regenerator2000 --dump-enum-files ~/Library/Application\ Support/regenerator2000/

    # Linux
    regenerator2000 --dump-enum-files ~/.config/regenerator2000/

    # Windows (PowerShell)
    regenerator2000.exe --dump-enum-files $env:APPDATA\regenerator2000\config\
    ```

    This command writes every `enum-*.toml` file to the destination folder and exits.

2. **Edit** an existing file or create a new enum file (for example, `enum-CustomColors.toml`):

    ```toml
    name = "CustomColors"
    description = "My custom system color definitions"

    [variants]
    BLACK = 0
    WHITE = 1
    RED = 2
    ```

3. **Start** Regenerator 2000. The application loads your custom enum automatically under the **Global** tab in the **Manage Enums** and **Apply Enum** dialogs.

---

## Assembler Syntax Support

During export, Regenerator 2000 translates enums into the syntax required by your target assembler:

=== "64tass"
Uses named `.struct` blocks to group variants:
```asm
Colors .struct
BLACK = $00
WHITE = $01
RED = $02
.ends

    ; Usage
    lda #Colors.BLACK
    .byte Colors.RED
    ```

=== "ca65"
Uses `.enum` blocks and double-colon (`::`) scope namespaces:
```asm
.enum Colors
BLACK = $00
WHITE = $01
RED = $02
.endenum

    ; Usage
    lda #Colors::BLACK
    .byte Colors::RED
    ```

=== "KickAssembler"
Uses `.enum` blocks and dot (`.`) namespaces:
```asm
.enum Colors { BLACK=$00, WHITE=$01, RED=$02 }

    // Usage
    lda #Colors.BLACK
    .byte Colors.RED
    ```

=== "ACME"
Because ACME does not support namespaces or enum blocks, Regenerator 2000 flattens enum names with an underscore (`_`) separator:
```asm
Colors_BLACK = $00
Colors_WHITE = $01
Colors_RED = $02

    ; Usage
    lda #Colors_BLACK
    .byte Colors_RED
    ```
