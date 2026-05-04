# Analysis

Regenerator 2000 includes an **auto-analyzer** that scans the binary and automatically creates labels and
cross-references. This page explains how the analyzer works and what the auto-generated label prefixes mean.

---

## Triggering the Analyzer

The analyzer runs in two situations:

1. **On load**: When a file is first opened (if enabled in [Settings](settings.md)).
2. **Manually**: Press ++ctrl+a++ to re-run the analysis at any time.

The analyzer is **non-destructive** — it never overwrites user-defined labels or comments. If you have already
renamed a label at an address, the analyzer will preserve your name and skip auto-generation for that address.

---

## How It Works

The analyzer performs a linear scan over all bytes in the loaded binary, examining each instruction and data block
to determine which addresses are referenced and _how_ they are referenced.

### Step 1: Instruction Scanning

For every byte in a **Code** block, the analyzer decodes the 6502 instruction and classifies the operand address
based on the **addressing mode**:

| Addressing Mode | Example Instruction | Label Type Generated |
| :------------------ | :------------------------- | :---------------------------------------- |
| Absolute            | `JSR $C000`                | **Subroutine** (`s_`)                     |
| Absolute            | `JMP $C000`                | **Jump** (`j_`)                           |
| Absolute            | `LDA $C000`                | **Absolute Address** (`a_`)               |
| Absolute,X / Y      | `LDA $C000,X`              | **Field** (`f_`)                          |
| Indirect            | `JMP ($0300)`              | **Pointer** (`p_`)                        |
| Zero Page           | `LDA $A0`                  | **ZP Absolute Address** (`zpa_`)          |
| Zero Page,X / Y     | `LDA $A0,X`                | **ZP Field** (`zpf_`)                     |
| (Indirect,X)        | `LDA ($FB,X)`              | **ZP Pointer** (`zpp_`)                   |
| (Indirect),Y        | `LDA ($FB),Y`              | **ZP Pointer** (`zpp_`)                   |
| Relative            | `BNE $C010`                | **Branch** (`b_`)                         |

!!! note

    Instructions with no memory operand (Implied, Accumulator, Immediate) do not generate labels.

### Step 2: Data Block Scanning

The analyzer also scans **data blocks** that encode addresses:

- **Address blocks**: Each pair of bytes is read as a little-endian 16-bit address and generates an
  **Absolute Address** (`a_`) label at the target.
- **Lo/Hi Address tables**: The block is split in half — the first half contains the low bytes and the second
  half contains the high bytes. Each pair produces an address label.
- **Hi/Lo Address tables**: Same as Lo/Hi but reversed — high bytes first, then low bytes.
- **Lo/Hi Word** and **Hi/Lo Word** tables are skipped (they encode raw values, not addresses).

!!! tip

    Use [splitters](tutorial.md#splitters) (++pipe++) to divide adjacent address tables. Without a splitter,
    the analyzer treats a continuous run of the same block type as a single table and pairs bytes accordingly.

### Step 3: Label Generation

After scanning, the analyzer has a map of every referenced address and _how_ it was used. It then generates labels:

1. **User labels are preserved**: If you have already defined a label at an address (`LabelKind::User`), the
   analyzer keeps it and does not add an auto-generated label.
2. **Internal addresses** (within the loaded binary) get a single label based on the first usage type encountered.
3. **External addresses** (outside the loaded binary) may get **up to two labels** — one for zero-page access
   and one for absolute access — since the same address can be referenced both ways (see
   [Zero Page vs. Absolute](#zero-page-vs-absolute-dual-labels) below).

### Step 4: Cross-Reference Building

Every time an instruction references an address, the analyzer records a **cross-reference** (x-ref) from the
instruction's address to the target. These x-refs are displayed as side comments in the disassembly and can be
navigated with ++ctrl+x++ (Find References).

### Step 5: Indirect Jump Resolution

As a final pass, the analyzer looks for `JMP ($xxxx)` instructions where `$xxxx` points to an **Address** block
inside the binary. If it finds one, it reads the 16-bit pointer stored there and registers the destination as a
jump target with its own label and cross-reference.

---

## Label Prefixes

Every auto-generated label uses a prefix that indicates _how_ the address is referenced in the code. The prefix
tells you at a glance whether an address is a subroutine entry point, a data table, a pointer, etc.

### Code-Flow Labels

These labels are generated from control-flow instructions (jumps, calls, branches):

| Prefix | Full Name     | Generated When                                   | Example              |
| :----- | :------------ | :----------------------------------------------- | :------------------- |
| `s_`   | Subroutine    | Target of a `JSR` instruction                    | `s_C000`             |
| `j_`   | Jump          | Target of a `JMP` instruction                    | `j_C100`             |
| `b_`   | Branch        | Target of a branch instruction (`BNE`, `BEQ`, …) | `b_C010`             |
| `e_`   | External Jump | `JSR`/`JMP`/branch target outside the binary     | `e_FFD2`             |

### Data-Access Labels

These labels are generated from data-access instructions (loads, stores, indexed access):

| Prefix | Full Name             | Generated When                                        | Example    |
| :----- | :-------------------- | :---------------------------------------------------- | :--------- |
| `a_`   | Absolute Address      | `LDA $XXXX`, `STA $XXXX`, etc. (absolute mode)       | `a_D020`   |
| `f_`   | Field                 | `LDA $XXXX,X`, `STA $XXXX,Y` (absolute indexed mode) | `f_0400`   |
| `p_`   | Pointer               | `JMP ($XXXX)` (indirect mode)                         | `p_0300`   |
| `zpa_` | ZP Absolute Address   | `LDA $XX`, `STA $XX` (zero page mode)                 | `zpa_A0`   |
| `zpf_` | ZP Field              | `LDA $XX,X`, `STA $XX,Y` (zero page indexed mode)     | `zpf_30`   |
| `zpp_` | ZP Pointer            | `LDA ($XX),Y`, `LDA ($XX,X)` (indirect ZP modes)      | `zpp_FB`   |

### User-Defined Labels

| Prefix | Full Name    | Description                                        |
| :----- | :----------- | :------------------------------------------------- |
| `L_`   | User-Defined | Default prefix when you create a label manually    |

!!! tip

    You can rename any auto-generated label by pressing ++l++. Once renamed, the analyzer will preserve your
    custom name across future analysis runs.

---

## Address Formatting

Label names include the target address in hexadecimal. The number of hex digits depends on the address range:

| Address Range     | Digits | Example                           |
| :---------------- | :----- | :-------------------------------- |
| Zero page ($00–$FF) with a ZP type | 2      | `zpa_A0`, `zpf_30`, `zpp_FB`    |
| Zero page ($00–$FF) with an absolute type | 4      | `a_00A0`, `f_0030`, `p_00FB` |
| Above zero page ($0100+)   | 4      | `s_C000`, `j_1005`, `a_D020`    |

This distinction matters because the same zero-page address can be accessed with _both_ a 2-byte zero-page
instruction and a 3-byte absolute instruction (see next section).

---

## Zero Page vs. Absolute (Dual Labels)

A common pattern in 6502 code is accessing the same zero-page address with two different addressing modes:

```asm
LDA $A0       ; Zero Page mode  → 2 bytes (A5 A0)
LDA $00A0     ; Absolute mode   → 3 bytes (AD A0 00)
```

When this happens for **external addresses** (outside the loaded binary), the analyzer generates **two labels**
at the same address:

- `zpa_A0` — the zero-page variant (2-digit address)
- `a_00A0` — the absolute variant (4-digit address)

This ensures that each instruction uses the correct label name matching its addressing mode. The assembler
needs both forms to generate the correct opcode size during reassembly.

!!! note

    For **internal addresses** (within the loaded binary), only one label is generated using the first
    addressing mode encountered during the scan.

---

## Excluded Addresses

Some addresses are excluded from label generation. The **excluded addresses** set (managed per-project) lets
you suppress labels for addresses that would create noise, such as addresses that are computed dynamically
and don't represent real targets.

When an address is in the excluded set, the analyzer skips it entirely — no label or cross-reference is generated.

---

## Platform-Specific Labels

When a [platform](platforms.md) is selected (e.g., Commodore 64), the analyzer benefits from pre-defined
**system labels** — well-known addresses for hardware registers, KERNAL entry points, and OS variables.
These labels have `LabelKind::System` and are preserved across analysis runs just like user labels.

For example, on the C64:

| Address   | System Label | Description              |
| :-------- | :----------- | :----------------------- |
| `$D020`   | `BORDER`     | Border color register    |
| `$D021`   | `BGCOL0`     | Background color 0       |
| `$FFD2`   | `CHROUT`     | KERNAL: Output character |
| `$FFE4`   | `GETIN`      | KERNAL: Get input        |

System labels take precedence over auto-generated labels. If the analyzer detects a `JSR $FFD2`, it will
display `JSR CHROUT` rather than `JSR s_FFD2`.

---

## Re-Running the Analyzer

You should re-run the analyzer (++ctrl+a++) after making significant changes:

- After converting blocks between Code and Data types.
- After defining new Address, Lo/Hi, or Hi/Lo tables.
- After loading additional platform definition files.

The analyzer rebuilds all auto-generated labels and cross-references from scratch, while always preserving
your user-defined labels and comments.
