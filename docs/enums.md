# Requirements: Enum Support in Regenerator 2000

This document defines the requirements for adding Enum support to Regenerator 2000. Enums allow users to represent numeric constants (immediate operands in instructions or bytes/words in data blocks) with semantic names, making disassembled code significantly easier to read, maintain, and re-assemble.

## Core Functional Requirements

### 1. Target-Specific Assembler Synthesis
Each supported assembler must define enums and format enum references in the best possible target-specific way:

- **64tass (Named Dicts)**:
  - **Definition**: Defined as a named dictionary block:
    ```assembly
    Colors = {
        BLACK: $00,
        WHITE: $01
    }
    ```
  - **Reference**: Accessed as dot-notated scoped names (e.g., `Colors.BLACK` or `#Colors.BLACK` for immediate mode).

- **KickAssembler (`.enum`)**:
  - **Definition**: Defined inside a `.enum` block:
    ```assembly
    .enum Colors {
        BLACK = $00,
        WHITE = $01
    }
    ```
  - **Reference**: Accessed as dot-notated scoped names (e.g., `Colors.BLACK` or `#Colors.BLACK` for immediate mode).

- **ca65 (`.enum`)**:
  - **Definition**: Defined inside a `.enum` block:
    ```assembly
    .enum Colors
        BLACK = $00
        WHITE = $01
    .endenum
    ```
  - **Reference**: Accessed as namespace-scoped names (e.g., `Colors::BLACK` or `#Colors::BLACK` for immediate mode).

- **ACME (Flat Equates)**:
  - **Definition**: Flat equates using underscore namespace separator:
    ```assembly
    Colors_BLACK = $00
    Colors_WHITE = $01
    ```
  - **Reference**: Flat equated names (e.g., `Colors_BLACK` or `#Colors_BLACK` for immediate mode).

- **Equates Placement & Origin Safety**:
  - All equates and enums must be placed at the very top of the assembly output file, preceding any origin directive (e.g., `* = $XXXX` or `.org $XXXX`).
  - The origin directive must be synthesized exactly before the first byte of the disassembled loaded binary, protecting multi-line dictionary or block definitions from being broken by inline program counter settings.

### 4. TUI Interaction (Smart Context-Aware Dialog)
- To assign an enum, the user presses `e` on a line with an immediate instruction or a data byte/word.
- This opens a pop-up dialog titled **"Apply Enum"** displaying a list of all available enums.
- **Smart Categorization / Sorting**: To make selection highly efficient, the dialog will inspect the numeric value at the current cursor address and split the enums into two sorted sections:
  1. **Matching Enums** (at the top): Enums that contain a variant mapping for the current value. The matching variant name is displayed next to the enum name in parentheses (e.g., `VIC_Colors (WHITE)`). The cursor defaults to the first item in this list.
  2. **Other Enums** (below a visual divider): Enums that do not currently contain a mapping for this value. This allows the user to still apply any enum if they plan to define the variant value later.
- A **`<None>`** option is available to clear any currently applied enum.
- A **dynamic search filter** text input is provided at the top of the dialog, allowing users to type to instantly filter the enums by name.
- The old shortcut `e` for "Convert to External File" is moved to `Shift+E` (represented as `E` key).

### 5. Dumping Built-in Enums
- There must be a command-line flag (`--dump-enum-files <PATH>`) that writes all embedded built-in enums as `.toml` files to the specified directory, allowing users to easily inspect, copy, and customize them globally.

### 2. Custom and Pre-defined Enums
- **User-defined Custom Enums**: Users can define their own custom enums in the project.
- **Pre-defined Enums**:
  - **Built-in Enums** (Asset-based): Regenerator 2000 ships with built-in pre-defined enums compiled directly into the binary (such as `VIC_Colors` for Commodore machines), similar to the built-in themes.
  - **User Global Enums**: Users can create their own global pre-defined enums by storing `.toml` files in their preferences directory.
- **Project Embedding**: Once a `.regen2000proj` project uses a global or built-in enum at least once, the full enum definition must be copied and **embedded permanently** into the project file itself.
  - This ensures that the project remains completely portable and compiles/disassembles correctly even on machines without those global or built-in enums.
- **Precedence**: Precedence of enums with the identical name must be:
  1. **Project Enums** (embedded in `.regen2000proj`) take the highest precedence.
  2. **User Global Enums** (stored in the preferences directory) take secondary precedence.
  3. **Built-in Enums** (compiled into the binary) have the lowest precedence.
  This allows users to override built-in definitions globally or per-project!

### 3. Storage
- **Global Storage**: Global enums are stored in `.toml` files. They can be located in standard user preference folders, like `config.toml` or separate TOML files (similar to how themes are managed).
- **Project Storage**: Project-specific enums and address-to-enum associations are saved inside the `.regen2000proj` JSON file.

---

## Proposed Storage Specification (Technical Draft)

### 1. Global Enum File (`enum-*.toml`)
Global enums are stored in separate TOML files under the user preferences folder (e.g., `~/.config/regenerator2000/` on Linux/macOS) with an `enum-` prefix:

```toml
name = "VIC_Colors"

[variants]
"0" = "BLACK"
"1" = "WHITE"
"2" = "RED"
"3" = "CYAN"
"4" = "PURPLE"
"5" = "GREEN"
"6" = "BLUE"
"7" = "YELLOW"
"8" = "ORANGE"
"9" = "BROWN"
"10" = "LIGHT_RED"
"11" = "DARK_GREY"
"12" = "GREY"
"13" = "LIGHT_GREEN"
"14" = "LIGHT_BLUE"
"15" = "LIGHT_GREY"
```

*Note: Keys are parsed as string representations of numbers, which allows using decimal (`"10"`), hex (`"0x0a"`, `"$0a"`), or binary (`"0b0101"`, `"%0101"`) values.*

### 2. Project File (`.regen2000proj`) Serialization
The project file JSON structure is extended to support:
- `enums`: A dictionary of enum definitions embedded in the project.
- `enum_usages`: A map of addresses to enum names.

```json
{
  "version": 2,
  "origin": 2049,
  "raw_data_base64": "...",
  "enums": {
    "VIC_Colors": {
      "name": "VIC_Colors",
      "variants": {
        "0": "BLACK",
        "1": "WHITE",
        "2": "RED"
      }
    }
  },
  "enum_usages": {
    "4096": "VIC_Colors",
    "4120": "VIC_Colors"
  }
}
```
