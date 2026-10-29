# Binary Unpacker

Regenerator 2000 includes a CPU-emulated **Binary Unpacker** for compressed ("packed") Commodore 8-bit binaries (`.prg`).

Many Commodore 8-bit programs use packers such as Dali, Exomizer, PUCrunch, TSCrunch, or ByteBoozer. A packed binary starts with a small decompression stub followed by compressed data. Static analysis cannot read the program until the stub decompresses the code and data into RAM.

Regenerator 2000 runs the unpacking routine inside a background 6502 emulator sandbox, tracks modified memory and bound provenance, and loads the decompressed binary directly into the disassembler via `AppState::apply_unpack_result`.

---

## Subsystem Architecture & 6-Module Hierarchy

The binary unpacker is organized into six single-responsibility modules under [`crates/regenerator2000-core/src/unpacker/`](https://github.com/ricardoquesada/regenerator2000/tree/main/crates/regenerator2000-core/src/unpacker):

```
crates/regenerator2000-core/src/unpacker/
├── mod.rs        // Public API facade (unpack, UnpackConfig, UnpackResult, re-exports)
├── cia.rs        // MOS 6526 CIA 1 & CIA 2 timer state emulation & cycle-stepping (CiaState)
├── bus.rs        // C64Bus (UnpackerMemory) bus, C64 $00/$01 processor port, ROM banking, I/O dispatch
├── machine.rs    // Target machine profiles (RomTrapTable, KernalTrap, Phase, get_basic_start)
├── engine.rs     // Decompression state machine, 2-phase 6502 loop, patches, UnpackReport & BoundSource
└── detector.rs   // Memory range detection, trailing cluster trimming, snapshot diffing heuristics
```

1. **`mod.rs`**: Exposes `unpack()`, `UnpackConfig`, `UnpackResult`, `UnpackerMemory`, `UnpackError`, and re-exports `PackerId`, `BoundSource`, `ExitReason`, `TRACE_TAIL_CAPACITY`, `UnpackEngine`, and `UnpackReport`.
2. **`cia.rs`**: Implements `CiaState` for MOS 6526 CIA 1 and CIA 2 timer latches, counters, and cycle stepping (`step_cycles(cycles)`).
3. **`bus.rs`**: Implements `mos6502::memory::Bus` for `C64Bus` (`UnpackerMemory`). Handles checked ROM slice lookups (`checked_sub` and `.get()`), C64 processor port `$00`/`$01` banking, I/O register dispatch, and per-phase write-tracking bitmaps (`written` and `written_phase2`).
4. **`machine.rs`**: Defines system-keyed machine profiles (`RomTrapTable::for_system(system)`, `KernalTrap`, `Phase`, and `get_basic_start(load_addr)`), isolating target-specific KERNAL/BASIC ROM traps, exit vectors, and screen RAM boundaries from the execution engine.
5. **`engine.rs`**: Runs the unified two-phase 6502 state machine (`Phase::FindDepacker` and `Phase::Decompress`), applies pre-emulation `MemoryPatch` rules, handles undocumented opcodes (`SHX`, `SHY`), and records structured diagnostics in `UnpackReport` (`ExitReason`, phase instruction counts, `BoundSource` provenance, and optional `trace_tail`).
6. **`detector.rs`**: Implements named-threshold memory range scanning (`detect_output_range`), trailing depacker workspace trimming, and pre/post-emulation snapshot diffing.

---

## How It Works: Two-Phase Emulation & Provenance

The unpacker runs a cycle-accurate MOS 6502 emulation sandbox with target system memory mappings. It uses a two-phase execution algorithm inspired by [**unp64**](http://iancoog.altervista.org/) combined with write-tracking bitmaps and RAM snapshot diffing:

```
  ┌──────────────────────────────────────────────────────────────┐
  │ Phase 1: FindDepacker                                        │
  │ - Resolves entry (forced_entry > PackerSpec > BASIC SYS)     │
  │ - Applies conditional/unconditional pre-emulation patches    │
  │ - Captures pre-emulation 64 KB RAM snapshot                  │
  │ - Emulates until PC reaches depacker or drops below RAM      │
  │   start ($0800 on C64, $1C00 on C128, $1000 on VIC-20/Plus4, │
  │   $0400 on PET)                                              │
  └──────────────────────────────┬───────────────────────────────┘
                                 │ (Phase::Decompress entered)
                                 ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ Phase 2: Decompress                                          │
  │ - Continues emulation from the located depacker stub         │
  │ - Tracks every byte written to RAM                           │
  │ - Stops when PC returns to unpacked code, hits known entry,  │
  │   leaves loaded region, or hits a ROM exit / BASIC RUN trap  │
  └──────────────────────────────┬───────────────────────────────┘
                                 │ (ExitReason recorded)
                                 ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ Extraction, Bound Refinement & UnpackReport Provenance       │
  │ - Scans write-tracking bitmap + snapshot diff for bounds     │
  │ - Applies PackerSpec hints, post-run pointers & refine hooks │
  │ - Records BoundSource for start_addr, end_addr & entry_point │
  └──────────────────────────────────────────────────────────────┘
```

### Phase 1: `Phase::FindDepacker`

Most packed programs begin with a BASIC bootstrap stub (such as `10 SYS 2061` on C64) or a machine-code header that relocates a small decruncher routine into low memory (such as zero page or stack/tape buffer RAM) or high RAM:

- After initializing system zero page (when `system.is_c64()`), the engine runs `packers::registry::detect` against the `LoadedImage`.
- The initial program counter is chosen by precedence: user `UnpackConfig::forced_entry` > packer `Hints::forced_entry` > parsed BASIC `SYS` target at `get_basic_start`.
- Any pre-emulation `MemoryPatch` entries declared by the matched `PackerSpec` are applied to RAM before taking the 64 KB pre-emulation snapshot (without marking those bytes in the write tracker).
- Phase 1 transitions to `Phase::Decompress` when `PC` matches the resolved `dep_addr`, when `PC` drops below the return boundary (`system.ram_start()`: `$0800` on C64, `$1C00` on C128, `$1000` on VIC-20 and Plus/4, `$0400` on PET) and a write occurs at or above `ram_start`, or when code executing in high RAM writes to the program region.

### Phase 2: `Phase::Decompress`

During decompression:

- Every RAM write sets the corresponding address in the `written` and `written_phase2` bitmaps.
- If the matched `PackerSpec` declares monitored pointers (`PointerAtDepacker`, `MonitoredMax`, `MonitoredMin`), `PointerMonitor` samples the pointer whenever `PC` hits `dep_addr`.
- Emulation stops with a typed `ExitReason`:
  - `ExitReason::WrittenCodeReached { pc }`: `PC` returned to the program region (`pc >= ret_addr`, or `pc == ret_addr` when `Hints::exact_return` is enabled) at an address written during emulation.
  - `ExitReason::KnownEntryReached { pc }`: `PC` reached the packer's known `entry_point` after the initial 10-instruction guard.
  - `ExitReason::LeftLoadedRegion { pc }`: `PC` jumped outside the original loaded range (at or above `ram_start`) to a written address.
  - `ExitReason::RomExitVector { pc }`: `PC` hit a trapped KERNAL/BASIC exit vector from `RomTrapTable`.
  - `ExitReason::BasicRun`: `PC` hit a trapped BASIC `RUN` vector from `RomTrapTable`.

### Bound Resolution & `UnpackReport` Provenance

When emulation finishes, the engine computes `(start_addr, end_addr, entry_point)` and records a `BoundSource` for each bound in `UnpackResult.report` (`UnpackReport`) following a fixed override order:

1. **Generic write/snapshot diff (`BoundSource::WriteDiff`)**: `detect_output_range` scans the `written` bitmap and diffs final RAM against the pre-emulation snapshot to trim depacker workspace clusters. The initial entry point is recorded as `BoundSource::ExitPc`, `BoundSource::SysLineLookup`, `BoundSource::SnapshotEntry`, or `BoundSource::Forced` (when `UnpackConfig::forced_entry` or `Hints::forced_entry` was used).
2. **Static `PackerSpec` overrides (`BoundSource::PackerSpec { id }`)**: Static `Hints` (`Const`, `BasicStart`, `OperandLe`, `AbsoluteLe`) override `start_addr`, `end_addr`, or `entry_point`.
3. **Post-run pointer overrides (`BoundSource::PointerAfterRun { addr }`)**: Runtime pointer sources (`PointerAfterRun` with `PointerGuard::NearDetectedEnd` or `PointerGuard::McCrackenEndOfProgram`, `PointerPlusRegister`, `PointerAtDepacker`, `MonitoredMax`, `MonitoredMin`) refine `end_addr` or `start_addr`.
4. **Hook refinement (`BoundSource::PackerRefine { id }`)**: If the packer provides `PackerHooks::refine`, any bound whose value changes is attributed to `PackerRefine { id }` (bounds left unchanged preserve their prior `BoundSource`).
5. **BASIC `SYS` clamp (`BoundSource::SysLineClamp` / `BoundSource::SysLineLookup`)**: If a valid BASIC `SYS` line exists at `basic_start`, `entry_point` replaces a ROM/`basic_start` exit with `SysLineLookup` and `start_addr` is clamped downward to `basic_start` with `SysLineClamp`.
6. **Entry containment clamp (`BoundSource::EntryClamp`)**: Ensures `entry_point` (when `>= $0200`) lies within `start_addr..=end_addr`.

`UnpackReport` also records `find_depacker_instructions`, `decompress_instructions`, and (when `UnpackConfig::trace` is `true`) `trace_tail`, a ring buffer of up to `TRACE_TAIL_CAPACITY` (`32`) program counters executed immediately before exit.

---

## Machine Architecture Memory Boundaries

`UnpackConfig.target_system` configures system-specific memory boundaries and `RomTrapTable` entries via `System` methods:

- **Commodore 64 (C64)**: Default RAM start `$0800`, default BASIC start `$0801` (with `$07C0` recognized as the Card Cruncher load convention in `get_basic_start`), screen RAM `$0400`–`$07E7`, I/O `$D000`–`$DFFF`, PLA processor port `$00`/`$01`.
- **Commodore 128 (C128)**: Default RAM start `$1C00`, default BASIC start `$1C01`, MMU banking.
- **VIC-20**: Default RAM start `$1000`, default BASIC start `$1001`, VIC-I character maps.
- **PET (2001/4008/8032)**: Default RAM start `$0400`, default BASIC start `$0401`, monochrome PETSCII maps.
- **Plus/4 & C16**: Default RAM start `$1000`, default BASIC start `$1001`, TED registers.

---

## Declarative Packer Specifications & Registry

Packer detection and lifecycle hooks live under [`crates/regenerator2000-core/src/packers/`](https://github.com/ricardoquesada/regenerator2000/tree/main/crates/regenerator2000-core/src/packers). Instead of ad-hoc imperative detection chains, every packer is declared as a static `PackerSpec` in `packers/spec.rs` and registered in ascending `priority` order in `packers/registry.rs` (`REGISTRY` / `PACKERS`):

- **Declarative Signatures (`Signature`, `Anchor`, `Pat`, `sig!`)**: Match exact bytes (`Pat::Byte`), wildcards (`_` / `__` → `Pat::Any`), bitmasks (`(mask, eq)` → `Pat::Mask`), or byte sets (`[a, b]` → `Pat::OneOf`) at anchors relative to `System::default_basic_start()` (`Anchor::BasicStart { offset }`), fixed addresses (`Anchor::Absolute`), address windows (`Anchor::Window`), forward/backward load scans (`Anchor::ScanFromLoad`, `Anchor::ScanBackFromEnd`), or candidate lists (`Anchor::Candidates`).
- **Declarative Address Sources (`Hints`, `AddrSource`, `PointerGuard`)**: Express `dep_addr`, `start_addr`, `end_addr`, `entry_point`, `forced_entry`, and `exact_return` via `AddrSource::Const`, `AddrSource::BasicStart(i16)` (signed offset from `system.default_basic_start()`, e.g., `BasicStart(-1)` resolves to `$0800` on C64 without hardcoding platform constants), `AddrSource::OperandLe`, `AddrSource::AbsoluteLe`, `AddrSource::PointerAfterRun`, `AddrSource::PointerPlusRegister`, `AddrSource::PointerAtDepacker`, `AddrSource::MonitoredMax`, and `AddrSource::MonitoredMin`.
- **Pre-Emulation Memory Patches (`MemoryPatch`, `PatchAddr`)**: Applied to RAM before the snapshot is taken (unconditionally or guarded by `expected: Some(byte)`), supporting packers such as Antiram and ALZ64/Kabuto.
- **Per-Position Detection Escape Hatch (`PackerSpec::resolve`)**: Optional `fn(&LoadedImage<'_>, &SignatureMatch) -> Option<ResolvedSpec>` evaluated at each candidate anchor match when a packer requires secondary scans, computed operands, or position-dependent validation (returning `None` advances to the next candidate position in the anchor window).
- **Encapsulated Emulation Hooks (`PackerHooks`, `StepContext`, `RefineContext`)**: The 4 packers that require emulation-time logic (`exomizer`, `tscrunch`, `dali`, `byteboozer`) implement `PackerHooks` (`on_step` and `refine`). `StepContext` exposes a value copy of `Registers`, side-effect-free raw RAM reads (`peek`, `peek_u16_le`), and Phase 1 write-tracking resets (`forget_writes`) without leaking internal `CPU` or `C64Bus` state.

### Registered Packers (21 Specs in `REGISTRY`)

| Priority | ID | Display Name(s) | Kind |
| ---: | :--- | :--- | :--- |
| 10 | `eca` | `ECA Compactor` | `Window` + `resolve` (per-position window scan) |
| 20 | `action_replay` | `Action Replay` | Static `PackerSpec` |
| 30 | `cruel_cruncher` | `Cruel Cruncher` | Static `PackerSpec` |
| 40 | `pucrunch` | `PUCrunch` | `BasicStart` + `resolve` + `PointerAfterRun` |
| 50 | `time_cruncher` | `Time Cruncher` | Static `PackerSpec` |
| 60 | `card_cruncher` | `1001 CardCruncher ACM` / `1001 CardCruncher v4` | `Candidates` + `resolve` |
| 70 | `mc_cracken` | `McCracken Compressor` | `Window` + `resolve` + `PointerAfterRun` (`McCrackenEndOfProgram`) |
| 80 | `tbc` | `TBC Multicompactor` | `Absolute` (`Pat::Mask`) + `resolve` |
| 90 | `byteboozer` | `ByteBoozer` | `Absolute` + `resolve` + `PointerAfterRun` + `PackerHooks::refine` |
| 100 | `alz64` | `ALZ64/Quiss` / `ALZ64/Kabuto` | `Absolute` + `resolve` + `MemoryPatch` + `forced_entry` / `exact_return` |
| 110 | `tscrunch` | `TSCrunch v1.3+` / `TSCrunch v1.3+-X2` | `Absolute` + `resolve` + `PackerHooks` (`on_step`, `refine`) |
| 120 | `dali` | `Dali` | `Absolute` + `resolve` + `PointerAfterRun` + `PackerHooks::refine` |
| 130 | `tiny_crunch` | `TinyCrunch` | Static `PackerSpec` (`OperandLe`) |
| 140 | `super_cruncher` | `Super Cruncher` | Static `PackerSpec` |
| 150 | `final_cartridge` | `Final Cartridge III` | Static `PackerSpec` |
| 160 | `exomizer` | `Exomizer 3.0` / `Exomizer v3.02+` / `Exomizer 2.x` | `ScanBackFromEnd` + `resolve` + `PackerHooks` (`on_step`, `refine`) |
| 170 | `ccs` | `Commodore Cruncher System` | `resolve` (bug-compatible multi-variant scan) |
| 180 | `triad` | `Triad Cruncher` | Static `PackerSpec` |
| 190 | `eagle` | `Eagle Cruncher` | Static `PackerSpec` |
| 200 | `antiram` | `Antiram Packer v2.0` / `Antiram Packer v1.0` | `Absolute` + `resolve` + conditional `MemoryPatch` + `forced_entry` / `exact_return` |
| 210 | `turbo_cruncher` | `Turbo Cruncher` | Static `PackerSpec` |

> [!NOTE]
> **Deferred Q3 TODO — Sourcing Real `.prg` Corpus Fixtures:**
> Every spec in `REGISTRY` embeds a synthetic `fixture` verified by the registry self-tests (`every_spec_matches_its_own_fixture` and `no_fixture_is_claimed_by_a_higher_priority_spec`). However, eight registered packers currently have **no** real packed `.prg` binary in `tests/6502/` (**Action Replay**, **CCS**, **Cruel Cruncher**, **Eagle**, **Final Cartridge III**, **Super Cruncher**, **Triad**, and **Turbo Cruncher**), and **Card Cruncher** covers only its ACM variant (`c64_323_ice_psm.1001_card_cruncher.prg`, not v4), alongside uncovered sub-variants of Exomizer (`v3.02+`, `1.x`) and ALZ64 (`Kabuto`). Sourcing real packed `.prg` fixtures for these packers and recording their `unp64` reference entries with `unpacker_compare_all --update` is tracked as the deferred **Q3 TODO** (Design Doc Addendum A, §Q3).

---

## How to Add a Packer

Adding a new packer (or porting a scanner from `unp64`) follows a declarative workflow:

### 1. Define `pub static SPEC: PackerSpec` in `src/packers/<name>.rs`

Create `crates/regenerator2000-core/src/packers/<name>.rs`. Express signatures with `Signature`, `Anchor`, and `sig!`, and express address hints with `Hints` and `AddrSource`. Always use `Anchor::BasicStart { offset }` and `AddrSource::BasicStart(offset)` instead of hardcoding platform addresses, and gate the spec with `systems`:

```rust
use crate::state::types::TargetSystem;
use super::spec::{AddrSource, Anchor, Hints, PackerSpec, Signature, sig};

const SIGNATURES: [Signature; 1] = [Signature {
    anchor: Anchor::BasicStart { offset: 0x0C }, // BasicStart + $0C ($080D on C64)
    pattern: sig![
        0x78, 0xA2, __, 0xBD, __, __, 0x9D, 0x00, 0x01, 0xD0, 0xFC, 0x4C, 0x00, 0x01
    ],
}];

pub static SPEC: PackerSpec = PackerSpec {
    id: "eagle",
    name: "Eagle Cruncher",
    priority: 190,
    systems: &[TargetSystem::C64],
    load_addr_max: Some(0x0801),
    signatures: &SIGNATURES,
    hints: Hints {
        dep_addr: Some(AddrSource::Const(0x0100)),
        start_addr: Some(AddrSource::BasicStart(0)),
        ..Hints::NONE
    },
    patches: &[],
    resolve: None,
    hooks: None,
    fixture: &[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x78, 0xA2, 0x00, 0xBD, 0x00, 0x00, 0x9D, 0x00, 0x01, 0xD0, 0xFC, 0x4C, 0x00, 0x01,
    ],
    fixture_load_addr: 0x0801,
};
```

- **Pre-emulation patches**: Populate `patches: &[MemoryPatch { addr: PatchAddr::Absolute(...) /* or FromMatch(...) */, expected: Some(...), bytes: &[...] }]`.
- **Computed detection**: Set `resolve: Some(resolve_fn)` where `fn resolve_fn(image: &LoadedImage<'_>, sig_match: &SignatureMatch) -> Option<ResolvedSpec>` inspects `image` and returns `Some(ResolvedSpec::from_hints(...))` (or `None` to continue scanning the anchor window).
- **Emulation hooks**: Set `hooks: Some(hooks_fn)` returning `Box<dyn PackerHooks>` if the packer needs `on_step(&mut self, ctx: &mut StepContext<'_>)` (using `ctx.peek(addr)` or `ctx.forget_writes(range)`) or post-emulation `refine(&self, ctx: &RefineContext<'_>, bounds: &mut Bounds)`.

### 2. Register in `src/packers/mod.rs` and `src/packers/registry.rs`

1. Add `pub mod <name>;` to `crates/regenerator2000-core/src/packers/mod.rs`.
2. Insert `&<name>::SPEC` into `REGISTRY` in `crates/regenerator2000-core/src/packers/registry.rs` in strictly ascending `priority` order.
3. Add a signature and hint resolution unit test in `<name>.rs` (or `registry.rs`). The registry invariant tests (`ids_are_unique_and_priorities_strictly_increase`, `every_spec_matches_its_own_fixture`, `no_fixture_is_claimed_by_a_higher_priority_spec`, `c64_spec_does_not_match_on_other_systems`) will automatically validate your `SPEC` and its `fixture`.

### 3. Add a Corpus Fixture to `tests/6502/unpack_corpus.toml`

When a real packed `.prg` fixture is available:

1. Place the `.prg` file in `tests/6502/` and add a `[[case]]` entry (`file = "<name>.prg"`) to `tests/6502/unpack_corpus.toml`.
2. Record the `unp64` reference result into the manifest:
   ```bash
   cargo run -p regenerator2000-core --bin unpacker_compare_all -- --update
   ```
3. If `unpacker_compare_all --update` proposes an `accepted_divergence`, inspect the difference, verify that the overlapping payload matches, and sign `reviewed_by`.
4. Update the frozen R2000 baseline and run the corpus integration test:
   ```bash
   UPDATE_UNPACK_BASELINE=1 cargo test -p regenerator2000-core --test unpack_corpus
   cargo test -p regenerator2000-core --test unpack_corpus
   ```

---

## Advanced Sandboxing Features

The unpacker emulates hardware features that complex packers require:

- **PLA Bank Switching (C64)**: When `system.is_c64()`, `C64Bus` simulates Processor Port (`$00`/`$01`) banking. Packers can decompress data into RAM under the `$D000`–`$DFFF` I/O area or under the `$A000`–`$BFFF` and `$E000`–`$FFFF` ROM ranges.
- **ROM Interception (`RomTrapTable`)**: Traps target-system KERNAL and BASIC vectors defined in `RomTrapTable::for_system(system)` (such as `GETIN` at `$FFE4`, `CHROUT` at `$FFD2`, `CINT` at `$FF5B`, and C64 screen clear at `$E544` filling `system.screen_ram()` with `$20`). It supplies simulated keystrokes and returns `RTS` so depackers do not hang waiting for user input.
- **PLA-Visible RAM Writes**: Ignores writes to I/O registers (`$D000`–`$DFFF` on C64 when I/O is mapped) while tracking writes when RAM is mapped into that range.

---

## Using the Unpacker

### 1. Automatic Entropy Detection

When you load a binary file, Regenerator 2000 calculates its **Shannon Entropy**. Compressed or encrypted binaries have high entropy (`>= 7.4`).

If a file has high entropy or matches a registered `PackerSpec` in `registry::detect`:

1. **Import Context Setup Dialog**: Displays a warning at the bottom of the dialog (for example, `[!] High Entropy (7.82). Packed with Exomizer 3.0.`). It also focuses the **`< Unpack >`** button. Press **++enter++** to unpack immediately.
2. **High Entropy Warning Dialog**: Alerts you when you open a packed binary.

### 2. Unpacking in the TUI

Unpack a loaded binary from the terminal UI:

1. Press **++alt+f++** (or click **File**) to open the **File** menu.
2. Select **Unpack Binary...** to open the Unpack Options dialog:
    - Press **++enter++** to run with automatic detection.
    - Or set manual overrides before you press **++enter++**:
        - **Entry Point (Hex)** (such as `0810`, equivalent to `unp64 -e`): Overrides the Phase 1 start address.
        - **Return Address (Hex)** (such as `0800` on C64, equivalent to `unp64 -r`): Overrides the Phase 1 return boundary.
        - **Depacker Address (Hex)** (such as `033C`, equivalent to `unp64 -d`): Overrides the Phase 2 decruncher loop address.
        - **Max Instructions** (such as `50000000`, equivalent to `unp64 -m`): Sets the instruction timeout limit.
3. Wait while the background thread runs the 6502 emulator. The status bar shows live progress (for example, `Unpacking... $0002F8A0`).
4. When unpacking finishes, both the TUI and MCP invoke `AppState::apply_unpack_result`, which loads the unpacked image at `start_addr`, performs a single flow-analysis pass from `entry_point`, creates the `start` label, moves the cursor to `entry_point`, and reports the detected packer, address range, entry point, and `ExitReason`.

### 3. Programmatic Unpacking via MCP

Call the `r2000_unpack_binary` tool from an MCP client or AI agent to unpack the loaded binary:

```json
{
  "name": "r2000_unpack_binary",
  "arguments": {
    "entry_point": "0810",
    "return_address": "0800",
    "depacker_address": "033C",
    "max_instructions": 100000000
  }
}
```

---

## Troubleshooting & Configuration Limits

- **Timeout limits**: The unpacker enforces a default limit of **50 million instructions** (`max_instructions`) shared across `Phase::FindDepacker` and `Phase::Decompress`. If a binary exceeds this limit, the unpacker returns `UnpackError::Phase1Timeout` or `UnpackError::Phase2Timeout`.
- **Execution trace (`UnpackConfig::trace`)**: Setting `trace: true` records the last `TRACE_TAIL_CAPACITY` (`32`) executed program counters in `UnpackReport::trace_tail` to diagnose unexpected exits.
- **Custom ROMs**: `UnpackConfig` supports custom `basic_rom` (`$A000`–`$BFFF` on C64), `kernal_rom` (`$E000`–`$FFFF` on C64), and `char_rom` (`$D000`–`$DFFF` on C64) images.

---

## unp64 Oracle, Corpus Verification & Reviewed Divergences

Regenerator 2000 validates its unpacker against a 43-case corpus manifest in [`tests/6502/unpack_corpus.toml`](../tests/6502/unpack_corpus.toml) and a frozen output snapshot in [`tests/6502/unpack_baseline.toml`](../tests/6502/unpack_baseline.toml) via `cargo test -p regenerator2000-core --test unpack_corpus`.

Each corpus case is evaluated by one of three explicit oracle rules:

1. **Ground-Truth Originals (`[[group]]`)**: For the 11 packed variants in the `moving_tubes` (`c64_moving_tubes_lxt_2e00.prg`) and `mule` (`c64_mule_1100.prg`) groups, the unpacked range, entry point, and payload bytes must match the original uncompressed binary byte-for-byte, and all packers in the group must agree with each other.
2. **Recorded `unp64` Reference (`[case.unp64]`)**: By default, cases without an original binary must match the range, entry point, length, and FNV-1a-64 payload hash recorded from `unp64` v2.42 (`cargo run -p regenerator2000-core --bin unpacker_compare_all -- --update`).
3. **Reviewed Exceptions (`[case.accepted_divergence]` & `expected_fail`)**: Because `unp64` does not track memory writes or diff RAM snapshots (for example, defaulting its end address to `$FFFF` when no end scanner fires, or differing by 1 byte at the BASIC link/load boundary), any case where Regenerator 2000 differs from `unp64` must carry an explicit, reviewed `accepted_divergence` entry (or `expected_fail` marker). For every `accepted_divergence`, only the listed `fields` (`start`, `end`, `entry`) may differ, and the unpacked payload over the overlapping range (`[case.unp64.overlap]`) must still match `unp64` byte-for-byte.

### Accepted Divergences & Expected Failures (`tests/6502/unpack_corpus.toml`)

The table below lists every case in `tests/6502/unpack_corpus.toml` that carries an `accepted_divergence` or `expected_fail` marker (kept in sync with the manifest by test `docs_divergence_table_in_sync_with_manifest` in `crates/regenerator2000-core/tests/unpack_corpus.rs`):

<!-- BEGIN CORPUS DIVERGENCE TABLE -->
| File | unp64 (range → entry) | R2000 (range → entry) | Kind | Fields | Expected Fail |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `c64_323_ice_psm.1001_card_cruncher.prg` | `$0801-$319C → $3197` | `$07C1-$319C → $3197` | `unexplained` | `start` | `-` |
| `c64_8_bit_ball.meanteam_cruncher.prg` | `$0801-$BFFE → $8100` | `$0801-$FF9E → $8100` | `unexplained` | `end` | `-` |
| `c64_HBFS.exo3.prg` | `$0400-$FEFF → $080D` | `$0801-$FEFF → $EFB0` | `unexplained` | `start, entry` | `payload_mismatch_under_investigation` |
| `c64_Layers.exo3.prg` | `$080D-$FBF1 → $0834` | `$0801-$FBF1 → $0834` | `unexplained` | `start` | `-` |
| `c64_bluemarble4k_unk.prg` | `$0800-$FFFF → $0911` | `$0800-$FFEF → $0911` | `unp64_default_end` | `end` | `-` |
| `c64_boilerplate.exo3.prg` | `$0801-$FEA4 → $1000` | `$0800-$FEA4 → $1000` | `padding_byte` | `start` | `-` |
| `c64_boo_alz64.prg` | `$2A78-$4D3B → $2A78` | `$0801-$4D3C → $2A78` | `unexplained` | `start, end` | `-` |
| `c64_connection-8580.pucrunch.prg` | `$0810-$FF3F → $0810` | `$0801-$F87D → $0810` | `unexplained` | `start, end` | `-` |
| `c64_cubicdream.exo3.prg` | `$0801-$EF2A → $080D` | `$0800-$EF2A → $080D` | `padding_byte` | `start` | `-` |
| `c64_daah_those_acid_pills.exo2.prg` | `$0810-$FFF9 → $0810` | `$0801-$FF91 → $0810` | `unexplained` | `start, end` | `-` |
| `c64_druid_too.exo3.prg` | `$0801-$CE1F → $4800` | `$0800-$CE1F → $4800` | `padding_byte` | `start` | `-` |
| `c64_hw20131031.exo.prg` | `$0800-$FF3F → $3000` | `$0801-$F481 → $3000` | `unexplained` | `start, end` | `-` |
| `c64_leftovers-pl.exo3.prg` | `$0801-$F3F0 → $080E` | `$0800-$F3F0 → $080E` | `padding_byte` | `start` | `-` |
| `c64_lft-nine.exo3.prg` | `$0801-$7CBC → $080D` | `$0800-$7CBC → $080D` | `padding_byte` | `start` | `-` |
| `c64_lft-rodents-in-the-attic.exo3.prg` | `$0801-$C56B → $080D` | `$0800-$C56B → $080D` | `padding_byte` | `start` | `-` |
| `c64_robot - not human.exo3.prg` | `$0801-$CBE6 → $0810` | `$0800-$CBE6 → $0810` | `padding_byte` | `start` | `-` |
| `c64_roma.exe.exo3.prg` | `$0801-$C8C5 → $0820` | `$0800-$C8C5 → $0820` | `padding_byte` | `start` | `-` |
| `c64_soul_on_fire_unk.prg` | `$0800-$FFFF → $E000` | `$0801-$E000 → $E000` | `unexplained` | `start, end` | `-` |
| `c64_spectro.exo3.prg` | `$080D-$E7FF → $08A1` | `$0801-$E7FF → $08A1` | `unexplained` | `start` | `-` |
| `c64_sprite_runners.exo3.prg` | `$0801-$95BF → $6900` | `$0800-$95BF → $6900` | `padding_byte` | `start` | `-` |
| `c64_traveller.tiny_crunch.prg` | `$0801-$FFFD → $0911` | `$0801-$7949 → $0911` | `unp64_default_end` | `end` | `-` |
<!-- END CORPUS DIVERGENCE TABLE -->

All other 22 cases in `tests/6502/unpack_corpus.toml` (including all 11 ground-truth cases in the `moving_tubes` and `mule` groups, plus `c64_Bit_by_Bits-BZ!.exo3.prg`, `c64_CopperBooze.byte_boozer2.prg`, `c64_FppScroller.byte_boozer2.prg`, `c64_chiller.antiram_packer.prg`, `c64_endoskull.exo3.prg`, `c64_f600.exo.prg`, `c64_fantasy_intro.eca_compactor.prg`, `c64_gianna_sister_remix_badboy.tbc_multicompactor.prg`, `c64_little_things.exo3.prg`, `c64_radiant-every_time_i_go_on_pouet.byte_boozer2prg.prg`, and `c64_thats_the_way_scoop.time_cruncher.prg`) match `unp64` (and their original uncompressed binaries, where present) in start address, end address, entry point, and full payload hash.
