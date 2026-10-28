# Analysis

Regenerator 2000 includes an **auto-analyzer** that scans the binary and creates labels and cross-references automatically. This page explains the analyzer workflow and the generated label prefixes.

---

## Triggering the Analyzer

The analyzer runs in two situations:

1. **On load**: When you open a file and auto-analysis is active in [Settings](settings.md).
2. **Manually**: When you press ++ctrl+a++ to run the analysis again.

The analyzer never overwrites user-defined labels or comments. If you rename a label at an address, the analyzer preserves your name and skips auto-generation for that address.

---

## How It Works

The analyzer scans all bytes in the loaded binary linearly. It inspects each instruction and data block to identify target addresses and reference types.

### Step 1: Instruction Scanning

For every byte in a **Code** block, the analyzer decodes the 6502 instruction and classifies the operand address by its **addressing mode**:

| Addressing Mode | Example Instruction | Label Type Generated             |
|:----------------|:--------------------|:---------------------------------|
| Absolute        | `JSR $C000`         | **Subroutine** (`s_`)            |
| Absolute        | `JMP $C000`         | **Jump** (`j_`)                  |
| Absolute        | `LDA $C000`         | **Absolute Address** (`a_`)      |
| Absolute,X / Y  | `LDA $C000,X`       | **Field** (`f_`)                 |
| Indirect        | `JMP ($0300)`       | **Pointer** (`p_`)               |
| Zero Page       | `LDA $A0`           | **ZP Absolute Address** (`zpa_`) |
| Zero Page,X / Y | `LDA $A0,X`         | **ZP Field** (`zpf_`)            |
| (Indirect,X)    | `LDA ($FB,X)`       | **ZP Pointer** (`zpp_`)          |
| (Indirect),Y    | `LDA ($FB),Y`       | **ZP Pointer** (`zpp_`)          |
| Relative        | `BNE $C010`         | **Branch** (`b_`)                |

!!! note

    Instructions without a memory operand (Implied, Accumulator, Immediate) do not generate labels.

!!! note

    If a `JSR`, `JMP`, or branch instruction targets an internal address that starts with `RTS` ($60) or `RTI` ($40), the analyzer assigns a **Return** (`r_`) label. This behavior matches the `locret_` convention in IDA Pro.

### Step 2: Data Block Scanning

The analyzer also scans **data blocks** that encode addresses:

- **Address blocks**: The analyzer reads each byte pair as a little-endian 16-bit address and generates an **Absolute Address** (`a_`) label at the target.
- **Lo/Hi Address tables**: The analyzer splits the block into two halves. The first half holds low bytes, and the second half holds high bytes. Each pair produces an address label.
- **Hi/Lo Address tables**: The analyzer splits the block into high bytes first and low bytes second.
- **Lo/Hi Word** and **Hi/Lo Word** tables: The analyzer skips these blocks because they encode raw values instead of addresses.

!!! tip

    Press ++pipe++ to insert [splitters](tutorial.md#splitters) between adjacent address tables. Without a splitter, the analyzer treats contiguous blocks of the same type as one table.

#### Fill Run Detection

If a **Byte** block contains a contiguous run of identical bytes that exceeds the **Fill run threshold** in [Document Settings](settings.md), the disassembler groups those bytes. During export, the exporter emits assembler-specific fill directives (such as `.fill` or `.res`).

### Step 3: Label Generation

After the scan completes, the analyzer maps every referenced address to its usage type and generates labels:

1. **Preserve user labels**: If an address has a user label (`LabelKind::User`), the analyzer keeps it and skips auto-generation.
2. **Assign internal labels**: Internal addresses (inside the binary) receive one label from the first usage type that the analyzer encounters.
3. **Assign external labels**: External addresses (outside the binary) also receive one label from the first usage type. The analyzer converts control-flow targets (`JSR`, `JMP`, branches) outside the binary into **External Jump** (`e_`) labels.

### Step 4: Cross-Reference Building

When an instruction references an address, the analyzer records a **cross-reference** (x-ref) from the instruction address to the target. The disassembly view displays these x-refs as side comments. Press ++ctrl+x++ (Find References) to navigate to them.

### Step 5: Indirect Jump Resolution

In the final pass, the analyzer inspects `JMP ($xxxx)` instructions where `$xxxx` points to an internal **Address** block. If the analyzer finds a match, it reads the 16-bit pointer and registers the destination as a jump target with a label and cross-reference.

---

## Label Prefixes

Every auto-generated label starts with a prefix that shows how the code references that address. This prefix identifies whether an address is a subroutine entry point, a data table, or a pointer.

### Code-Flow Labels

Control-flow instructions (jumps, calls, and branches) generate these labels:

| Prefix | Full Name     | Generated When                                                           | Example  |
|:-------|:--------------|:-------------------------------------------------------------------------|:---------|
| `s_`   | Subroutine    | Target of a `JSR` instruction                                            | `s_C000` |
| `j_`   | Jump          | Target of a `JMP` instruction                                            | `j_C100` |
| `b_`   | Branch        | Target of a branch instruction (`BNE`, `BEQ`, …)                         | `b_C010` |
| `r_`   | Return        | Target of a branch/`JMP`/`JSR` whose first instruction is `RTS` or `RTI` | `r_AACC` |
| `e_`   | External Jump | `JSR`/`JMP`/branch target outside the binary                             | `e_FFD2` |

### Data-Access Labels

Data-access instructions (loads, stores, and indexed operations) generate these labels:

| Prefix | Full Name           | Generated When                                       | Example  |
|:-------|:--------------------|:-----------------------------------------------------|:---------|
| `a_`   | Absolute Address    | `LDA $XXXX`, `STA $XXXX`, etc. (absolute mode)       | `a_D020` |
| `f_`   | Field               | `LDA $XXXX,X`, `STA $XXXX,Y` (absolute indexed mode) | `f_0400` |
| `p_`   | Pointer             | `JMP ($XXXX)` (indirect mode)                        | `p_0300` |
| `zpa_` | ZP Absolute Address | `LDA $XX`, `STA $XX` (zero page mode)                | `zpa_A0` |
| `zpf_` | ZP Field            | `LDA $XX,X`, `STA $XX,Y` (zero page indexed mode)    | `zpf_30` |
| `zpp_` | ZP Pointer          | `LDA ($XX),Y`, `LDA ($XX,X)` (indirect ZP modes)     | `zpp_FB` |

### User-Defined Labels

| Prefix   | Full Name        | Description                                            |
|:---------|:-----------------|:-------------------------------------------------------|
| `L_`     | User-Defined     | Default prefix when you create a label manually        |
| `scope_` | LocalUserDefined | Prefix for labels inside a scope (`.proc` or `.block`) |

!!! tip

    Press ++l++ to rename any auto-generated label. The analyzer preserves your custom name during subsequent analysis runs.

---

## Address Formatting

Label names include the hexadecimal target address. The address range determines the number of hexadecimal digits:

| Address Range                             | Digits | Example                      |
|:------------------------------------------|:-------|:-----------------------------|
| Zero page ($00–$FF) with a ZP type        | 2      | `zpa_A0`, `zpf_30`, `zpp_FB` |
| Zero page ($00–$FF) with an absolute type | 4      | `a_00A0`, `f_0030`, `p_00FB` |
| Above zero page ($0100+)                  | 4      | `s_C000`, `j_1005`, `a_D020` |

---

## Excluded Addresses

You can exclude specific addresses from label generation at two levels:

### System-Level Exclusion

Select **"Exclude well-known addresses from symbolic analysis"** in [Document Settings](settings.md). This setting suppresses labels for noisy system addresses (such as indirectly accessed hardware registers). It uses the exclusion list from the active system definition.

### Per-Project User Exclusion

Place the cursor on an **external label** in the disassembly. Press ++delete++ to exclude that address for the current project. Regenerator 2000 adds the address to the **user-excluded addresses** set and saves it in the project file.

Use this feature to suppress false references, such as dynamically computed addresses.

!!! tip

    Press ++ctrl+z++ to undo an address exclusion. When you exclude an address, Regenerator 2000 removes the label and its cross-references from the disassembly immediately.

### Effect

If an address belongs to the system-level or per-project exclusion set, the analyzer skips it and generates no label or cross-reference.

---

## System-Specific Labels

When you select a [system](systems.md) (such as the Commodore 64), the analyzer loads predefined **system labels** for hardware registers, KERNAL entry points, and OS variables. These labels use `LabelKind::System`, and the analyzer preserves them across analysis runs.

For example, on the C64:

| Address | System Label | Description              |
|:--------|:-------------|:-------------------------|
| `$D020` | `BORDER`     | Border color register    |
| `$D021` | `BGCOL0`     | Background color 0       |
| `$FFD2` | `CHROUT`     | KERNAL: Output character |
| `$FFE4` | `GETIN`      | KERNAL: Get input        |

System labels take precedence over auto-generated labels. If the analyzer detects `JSR $FFD2`, it displays `JSR CHROUT` instead of `JSR s_FFD2`.

---

## Re-Running the Analyzer

Press ++ctrl+a++ to run the analyzer again after you make structural changes:

- After you convert blocks between Code and Data types.
- After you define new Address, Lo/Hi, or Hi/Lo tables.
- After you load additional system definition files.

The analyzer regenerates all auto-generated labels and cross-references. It preserves your user-defined labels and comments.
