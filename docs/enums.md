# Enums

Regenerator 2000 supports target-specific custom **Enums**, allowing you to replace raw numeric values (immediate operands, data bytes, and data words) with semantic named constants across all your disassemblies.

Enums make disassembled code significantly more readable by mapping magic numbers (like color indices, hardware control registers, or state constants) to clear text identifiers.

---

## The Three-Tier Scope Model

Regenerator 2000 uses a flexible three-tiered model for managing enum definitions. This allows you to reuse common hardware definitions across multiple disassemblies while still allowing specialized, project-specific names where needed.

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
  │    - Stored in ~/.config/regenerator2000/enums/         │
  │    - Saved as TOML files                                │
  └───────────┬─────────────────────────────────────────────┘
              │ (precedes)
              ▼
  ┌─────────────────────────────────────────────────────────┐
  │ 3. Built-in System Enums                                │
  │    - Curated for target machine (e.g. VIC-II, TED)      │
  │    - Read-only, embedded in the application             │
  └─────────────────────────────────────────────────────────┘
```

### 1. Project-Specific Enums
* **Scope**: Local to the current project file (`.regen2000proj`).
* **Purpose**: Definitions unique to the binary you are currently reverse-engineering (e.g., game state constants, level indices).
* **Precedence**: Has the highest priority. If a project-specific enum shares a name with a global or system enum, the project-specific definition is used.
* **Properties**: Fully integrated into the command queue. Creating, updating, or deleting project enums supports full Undo and Redo (`u` / `Ctrl+R`).

### 2. Global Custom Enums
* **Scope**: User-wide, shared across all projects on your machine.
* **Purpose**: Reuse your own custom enum definitions (e.g., standard math constants, common loader codes) across multiple different projects.
* **Precedence**: Medium priority. Overrides built-in system enums, but is overridden by local project enums.
* **Storage**: Stored as individual `.toml` files inside the user's global config directory:
  * **Linux/macOS**: `~/.config/regenerator2000/enums/` (or your OS equivalent)
  * **Windows**: `%APPDATA%\regenerator2000\enums\`

### 3. Built-in System Enums
* **Scope**: Read-only system definitions.
* **Purpose**: Standard constants defined by the target hardware. For example, when the target system is a Commodore 64, the built-in `VIC_Colors` enum is automatically available.
* **Precedence**: Lowest priority. Used as a fallback when no local or global custom enum overrides it.

---

## Portability and Reference Behavior

To ensure that your `.regen2000proj` project files are fully portable and can be shared with other developers without losing symbolic details, Regenerator 2000 manages enum usage rules carefully:

* **Built-in & Global Enums**: Applying a built-in system or global custom enum to an operand does **not** copy that entire enum definition into your project file. The project file only records a reference to the enum's name.
* **Project-Local Enums**: When you create a custom local enum, the definition is fully serialized into the `.regen2000proj` file.
* **Importing/Sharing**: If you load a project on another machine that is missing a custom global enum referenced by the disassembly, you can easily copy the definition to your local project pool using the TUI **Manage Enums** dialog to restore self-contained portability.

---

## Using Enums in the TUI

### Applying an Enum
To replace a raw numeric value with an enum variant:
1. Place the cursor on a line containing an immediate operand (`lda #$00`), a data byte (`.byte $02`), or a data word (`.word $0001`).
2. Press **++e++** to open the **Apply Enum** dialog.
3. Select the desired enum from the list of available definitions.
4. Select the matching variant from that enum to apply it.

The disassembler will immediately update the rendering to display the enum variant name (e.g. `lda #Colors.BLACK`).

### Managing Enums
You can create, edit, clone, or delete custom enums:
1. Open the main menu (press **++f10++** or press **++alt+t++** to go to the Edit menu).
2. Navigate to **Manage Enums...**.
3. The dialog features tabs for **Project**, **Global**, and **System** enums:
   - **Add**: Create a brand new local or global enum.
   - **Edit**: Update variants, names, or descriptions.
   - **Delete**: Remove enums (with safety checks that prevent deleting enums currently in use).
   - **Clone to Project/Global**: Easily copy built-in system enums or global custom enums into your project pool (or vice versa) for local customization.

---

## Assembler Syntax Support

Different 6502 cross-assemblers have their own conventions for representing scoped namespaces and enums. Regenerator 2000 translates your enums automatically during export to match the syntax of your selected target assembler:

=== "64tass"
    Uses named dictionaries to group variants:
    ```asm
    Colors .struct
    BLACK = $00
    WHITE = $01
    RED   = $02
    .ends

    ; Usage
    lda #Colors.BLACK
    .byte Colors.RED
    ```

=== "ca65"
    Uses `.enum` blocks and double-colon scope namespaces:
    ```asm
    .enum Colors
      BLACK = $00
      WHITE = $01
      RED   = $02
    .endenum

    ; Usage
    lda #Colors::BLACK
    .byte Colors::RED
    ```

=== "KickAssembler"
    Uses standard `.enum` blocks and dot namespaces:
    ```asm
    .enum Colors { BLACK=$00, WHITE=$01, RED=$02 }

    // Usage
    lda #Colors.BLACK
    .byte Colors.RED
    ```

=== "ACME"
    Since ACME does not natively support namespaces or enum blocks, enums are flattened using an underscore separator prefix:
    ```asm
    Colors_BLACK = $00
    Colors_WHITE = $01
    Colors_RED   = $02

    ; Usage
    lda #Colors_BLACK
    .byte Colors_RED
    ```
