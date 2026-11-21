# Regenerator 2000 - Agent Instructions

Regenerator 2000 is an interactive 6502 disassembler TUI written in Rust, targeting Commodore 8-bit machines. It uses [ratatui](https://github.com/ratatui-org/ratatui) + crossterm for the terminal UI and exposes an MCP (Model Context Protocol) server for programmatic access.

## Architecture & Crate Structure

The project is split into two main crates to separate logic from presentation:

- **`regenerator2000-core`**: Contains all UI-agnostic logic.
  - `action_handlers/`: Modular domain action handlers (`context.rs` [`ActionContext`, `DisassemblyViewAnchor`, `DomainActionHandler`], `file_handler.rs`, `disassembly_handler.rs`, `debug_handler.rs`, `navigation_handler.rs`).
  - `analyzer.rs`: Auto-analysis (walking code, generating labels, building typed cross-references).
  - `assets.rs`: System asset loading (labels, comments, exclude lists).
  - `commands.rs`: The `Command` enum and `UndoStack`. Every undoable action is a command.
  - `config.rs`: System-wide configuration management.
  - `core.rs`: Central orchestration logic (`Core` combining `AppState` and `CoreViewState`, and `apply_action` dispatch).
  - `cpu.rs`: 6502 opcode table, addressing mode logic, and `Opcode::xref_kind()`.
  - `disassembler/`: 6502 decoding, shared `DisassemblyLine` geometry/lowering (`line.rs`, `context.rs`), `Disassembler` engine (`engine.rs`, `pipeline.rs`, `symbols.rs`, `data_blocks.rs`, `handlers.rs`), and assembler formatters (`formatter_*.rs`: 64tass, ACME, KickAssembler, ca65).
  - `error.rs`: Typed `thiserror` error hierarchy (`CoreError`).
  - `event.rs`: Application event types (`CoreEvent`, `DialogType`).
  - `exporter/`: 3-stage Exporter IR pipeline (`ir.rs`), pure ASM/HTML backends (`asm.rs`, `html.rs`), and roundtrip verification (`verify.rs`).
  - `mcp/`: MCP server implementation (stdio and HTTP transports). Contains `handler.rs` dispatcher and `tools/` submodules (`analysis_tools`, `navigation_tools`, `modification_tools`, `session_tools`, `common`).
  - `navigation.rs`: Address navigation logic.
  - `packers/`: Declarative 6502 binary packer specifications (`PackerSpec`, `Signature`, `Hints`, `MemoryPatch`, `PackerHooks` in `spec.rs`, `PackerInfo` in `info.rs`, and the ordered `REGISTRY` in `registry.rs`).
  - `parser/`: File format parsers (.prg, .d64/.d71/.d81, .t64, .crt, .vsf snapshots, .dis65, VICE labels).
  - `state/`: Persistent application state (`AppState`), sparse `AnnotationManager`, domain types, and project serialization.
  - `unpacker/`: 2-phase 6502 emulation unpacker sandbox (`bus.rs`, `cia.rs`, `machine.rs`, `engine.rs`, `detector.rs`, `runner.rs` with `UnpackConfig`, `UnpackResult`, `unpack`, and `find_sys_address`, plus `UnpackReport` and `BoundSource`).
  - `utils.rs`: Shared utility functions.
  - `vice/`: VICE binary monitor protocol client for live debugging.
  - `view_state.rs`: View state management (`CoreViewState`).

- **`regenerator2000-tui`**: Contains all terminal UI-related code.
  - `ui/`: TUI widgets (`widget.rs`, `navigable.rs`), top menu (`menu/`), views (`view_disassembly/` submodule breakdown [`layout.rs`, `navigation.rs`, `mouse.rs`, `render.rs`, `mod.rs`], HexDump, Sprites, Charset, Bitmap, Blocks, Debugger), and modal dialogs (`dialog_*.rs`, including unified `FileBrowserDialog`, `PromptDialog`, `ContainerPickerDialog`, and table-driven `DocumentSettingsDialog`).
  - `ui_state.rs`: Transient UI state (cursor, scroll, active dialog, wrapping `CoreViewState` via `Deref`).
  - `events.rs` & `events/`: Event loop, background task coordination, and input handling.
  - `theme.rs`: Color scheme and styling logic.
  - `theme_file.rs`: Theme file parsing and custom theme support.
  - `utils.rs`: TUI utility functions.

## Core Domain Model

### Key Types (`crates/regenerator2000-core/src/state/types.rs`)

- **`Addr`**: A wrapper around `u16` representing a 6502 address.
- **`TargetSystem`** (`System` alias): Target machine (`C64`, `C128`, `Vic20`, `Pet20`, `Pet40`, `Plus4`, `C16`, `C1541`, `C1571`, `C1581`, `Custom`).
- **`Assembler`**: Target assembler syntax (`Tass64`, `Acme`, `Ca65`, `Kick`).
- **`BlockType`**: How a range of bytes is interpreted (`Code`, `DataByte`, `DataWord`, `Address`, `PetsciiText`, `ScreencodeText`, `LoHiAddress`, `HiLoAddress`, `LoHiWord`, `HiLoWord`, `ExternalFile`, `Undefined`).
- **`LabelType`**: Semantic meaning of a label (`Subroutine`, `Jump`, `Branch`, `Return`, `Pointer`, `ZeroPagePointer`, `Field`, `ZeroPageField`, `AbsoluteAddress`, `ZeroPageAbsoluteAddress`, `ExternalJump`, `Predefined`, `UserDefined`, `LocalUserDefined`).
- **`XRef` / `XRefKind`**: Typed cross-reference edge (`from: Addr`, `kind: XRefKind` — `Read`, `Write`, `ReadModifyWrite`, `Call`, `Jump`, `Offset`).

### State Management

- **`AppState`** (`crates/regenerator2000-core/src/state/app_state.rs`): The single source of truth for persistent data. Includes `raw_data`, `block_types`, `labels`, `annotations` (`AnnotationManager` consolidating comments, immediate value formats, bookmarks, scopes, and enum usages), `undo_stack`, `settings`, `cross_refs` (`BTreeMap<Addr, Vec<XRef>>`), and `splitters`. Serialized to `.regen2000proj`.
- **`UIState`** (`crates/regenerator2000-tui/src/ui_state.rs`): Transient state like cursor positions, scroll offsets, and active dialogs (wrapping `CoreViewState`). Never serialized.

## Application Logic Flow

Regenerator 2000 follows a unidirectional data flow (Redux/Elm style):

1. **Input**: `crates/regenerator2000-tui/src/events/input.rs` and `Widget::handle_input` map key events to `AppAction` (`MenuAction`).
2. **Dispatch**: `handle_menu_action` (`crates/regenerator2000-tui/src/ui/menu/menu_action.rs`) forwards `AppAction` to `Core::apply_action`, which delegates to `action_handlers/` and converts actions into `Command` variants and `CoreEvent` notifications.
3. **Execution**: `Command::apply(&mut AppState)` modifies the state and pushes to `UndoStack`.
4. **Analysis**: `AppState::perform_analysis()` (via `analyzer.rs`) updates labels and typed cross-references (`XRef`).
5. **Render**: `ui()` in `crates/regenerator2000-tui/src/ui.rs` re-renders the TUI from `AppState` + `UIState`.

### No Logic Duplication — State Logic Lives in Core

All validation, business rules, and state-mutation logic **must** live in `regenerator2000-core` (typically as methods on `AppState` in `state/`). Both the TUI (`regenerator2000-tui`) and the MCP server (`mcp/` handler + `mcp/tools/` submodules) are **consumers** of core logic — they must call shared `AppState` methods, not re-implement the same checks independently.

For example, if label creation needs to reject duplicate names, that validation belongs in an `AppState` method (e.g., `create_set_user_label_command`). The MCP tool submodules (`mcp/tools/`) and the TUI dialogs both call that single method. **Never** add domain logic directly into `mcp/tools/` or `events/input.rs` if it can be expressed as a core method. TUI disassembly rendering belongs in `view_disassembly/render.rs`, view navigation in `view_disassembly/navigation.rs`, visual layout math in `view_disassembly/layout.rs`, and mouse interaction in `view_disassembly/mouse.rs`.

When adding or modifying a feature, ask: *"Would this logic need to be duplicated if a third client (e.g., a GUI, a CLI) were added?"* If yes, it belongs in `regenerator2000-core`.

### System-Agnostic Design & Unpacker Rules

The core disassembler and binary unpacker (`crates/regenerator2000-core/src/unpacker/` directory) target Commodore 8-bit machines (C64, C128, VIC-20, PET, Plus/4).

- **Never hardcode platform constants**: Do not hardcode machine-specific memory addresses (such as `$0800` RAM start, `$0801` BASIC start, `$0400` screen RAM, or `$0314` IRQ vector) directly in `src/unpacker/` or documentation. Always query system properties via `System` platform methods (`system.ram_start()`, `system.default_basic_start()`, `system.screen_ram()`, `system.default_irq()`, `system.is_c64()`).
- **Guard platform-specific features**: System-specific behavior (such as C64 PLA processor port `$01` banking or zero-page template copying) must be explicitly guarded (e.g. `if system.is_c64()`).
- **Keep Documentation System-Agnostic**: When updating `docs/unpacker.md` or adding unpacker documentation, describe memory boundaries and zero-page setups in a system-agnostic way (referencing system RAM start `$0800` on C64, `$1C00` on C128, `$1000` on VIC-20, `$0400` on PET) rather than assuming C64 defaults globally.

## AI Agent Skills & Subagents (`.agent/`)

Specialized agents (`.agent/agents/`) and skills (`.agent/skills/`) are available for common tasks:

### Custom Agents (`.agent/agents/`)

- `r2000-cbm-principal-swe`: Commodore 8-Bit (CBM) and MOS 6502 Principal Systems, Demo-Coding, and Reverse-Engineering agent (`r2000_cbm_principal_swe.md`). Used by the `r2000-analyze-*` skills and for 6502 assembly tasks.

### Skills (`.agent/skills/`)

- `r2000-analyze-basic`: Analyze BASIC programs and mark pointers (uses `r2000-cbm-principal-swe`).
- `r2000-analyze-blocks`: Auto-detect and set block types (text, data) (uses `r2000-cbm-principal-swe`).
- `r2000-analyze-program`: Orchestrate 2-tier MapReduce full-program analysis using `r2000-cbm-principal-swe` subagents across BASIC, blocks, routines, and symbols.
- `r2000-analyze-routine`: Trace a subroutine and mark code paths (uses `r2000-cbm-principal-swe`).
- `r2000-analyze-symbol`: Find and label all references to a specific address (uses `r2000-cbm-principal-swe`).
- `compare-unpacker`: Benchmark Regenerator 2000 unpacker against unp64 and generate comparison reports.
- `add-mcp-tool`: Templates for adding new programmatic tools to the MCP server.
- `bump-version`: Automate version bumping and changelog updates.
- `update-keyboard-shortcuts`: Sync keyboard shortcuts across docs and source files.
- `update-mcp-docs`: Sync `docs/mcp.md` with the actual MCP tools.
- `verify-mcp`: Run MCP integration test suite to verify server functionality.

## Rust Best Practices

All AI-generated code MUST follow the rules below. The workspace enforces baseline lints through `cargo clippy -- -D warnings`; violations will block CI.

### Module Layout and Imports

- **Thin `mod.rs` files**: Keep non-leaf `mod.rs` files limited to module declarations, module doc comments (`//!`), and `pub use` re-exports. Place structs, traits, implementations, and unit tests in named leaf submodules (e.g., `action_handlers/context.rs`, `disassembler/engine.rs`, `disassembler/line.rs`, `packers/info.rs`, `unpacker/runner.rs`, and `bin/unpacker_compare_all/{corpus/constants.rs,unp64/runner.rs}`).
- **Flat `use` statements**: Import items from one module per `use` statement — avoid nested `use` trees (write separate `use std::collections::BTreeMap;` and `use std::fs;` imports rather than `use std::{collections::BTreeMap, fs};`). Grouping sibling items from the same module in `{...}` is fine (e.g., `use crate::state::{Addr, AppState};`).
- **Import types; avoid inline multi-segment paths**: Import types, traits, and enums at module scope (or inside `#[cfg(test)] mod tests`) and refer to them unqualified (`Range<usize>`, `HashSet<Addr>`, `Path`). Qualify free functions with their parent module when it improves clarity or avoids name collisions (`fs::read`, `analyzer::analyze`, `navigation::resolve_jump_content_line_index`, `basic::find_sys_address`). Exceptions: disambiguating shadowed prelude names (`std::result::Result` where `crate::Result` is in scope) and `$crate::...` paths inside `macro_rules!`.
- **No glob imports in production code**: Avoid `use foo::*;`. Glob imports are permitted only for facade re-exports (`pub use error::*;` in `lib.rs`, `pub use ...::*;` in `state/mod.rs`) and `use super::*;` inside `#[cfg(test)] mod tests`.

### Panic Safety

Production code must never panic silently.

- **Never use `.unwrap()` or `.expect()`** outside of `#[cfg(test)]` blocks. Both are banned by `clippy::unwrap_used` / `clippy::expect_used`.
- **Never use `std::panic::catch_unwind`** for control flow or error recovery.
- **No check-then-unwrap or check-then-expect**: Prefer pattern matching (`if let Some(val) = ...`, `let Some(val) = ... else { ... };`) over checking `.is_some()` / `.is_ok()` followed by unwrapping or `.expect()`.
- **Return `Result` or `Option`** and propagate errors with `?`.
- Use `anyhow::Result` for application-level errors where context strings suffice, and typed `thiserror` enums for library-facing error surfaces.
- `unreachable!()` inside exhaustive `match` arms (e.g., an enum variant that logically cannot appear) is acceptable. All other panic macros require a comment explaining why the invariant holds.
- Both `lib.rs` files gate these lints via:
  ```rust
  #![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
  #![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
  ```
  Do **not** remove these attributes.

### Error Handling

- Prefer `?` over manual `match`/`if let` for error propagation.
- Add context with `.context("what was attempted")` (from `anyhow`) when propagating errors across module boundaries.
- Do **not** silently swallow errors with `let _ = some_fallible_call();`. Log or propagate them.
- MCP tool handler functions must return explicit `McpError` variants for invalid/missing parameters — see `mcp/tools/` submodules for the established pattern.

### API Hygiene and Naming

- Add `#[must_use]` to every **pure** function (one with no side effects whose return value carries meaning). This includes constructors (`fn new`), getters, and any function returning `bool`, a numeric type, or a wrapped newtype.
- Add `#[must_use]` to error, outcome, and small value types (such as `CorpusError`) when ignoring a returned instance is likely a bug.
- Functions that mutate state (`&mut self`) and return `()` do **not** need `#[must_use]`.
- `#[must_use]` on an `impl` block annotates every method — use sparingly; prefer per-method annotations.
- **No `get_` prefix on public getters**: Name simple getters after the concept (`opcodes()`, `pointer()`, `formatter()`, `line_index_for_address()`, `hardware_name()`, `basic_start()`). Use descriptive verbs (`parse_*`, `find_*`, `resolve_*`) for fallible parsing or non-trivial lookups (e.g., `parse_address_arg`).
- **Descriptive lifetimes on public types**: Prefer named domain lifetimes (`'app`, `'ctx`, `'mem`, `'emu`, `'cfg`) over generic `'a` on public structs holding borrowed references (`ActionContext<'app>`, `DisassemblyContext<'app>`, `HandleArgs<'ctx>`, `LoadedImage<'mem>`, `StepContext<'emu>`, `UnpackEngine<'cfg>`).
- Run `cargo clippy -- -W clippy::must_use_candidate` to find unannotated candidates in existing code.

### Documentation

- All `pub` items in `regenerator2000-core` — including modules (`//!`), structs, struct fields, enums, enum variants, functions, and constants — must have doc comments (`///`).
- Functions returning `Result` must include a `# Errors` section explaining when they fail.
- Functions that can panic (outside tests) must include a `# Panics` section.
- Wrap code identifiers, CamelCase names (e.g., `` `AppState` ``, `` `KickAssembler` ``), hex addresses (`$0801`), flags, and file paths in backticks (`clippy::doc_markdown`).
- Use shorthand intra-doc links (`` [`TypeName`] ``) to cross-reference types rather than redundant explicit targets such as `` [`EnumDefinition`](EnumDefinition) `` (`rustdoc::redundant_explicit_links`).
- Run `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` and `cargo clippy -p regenerator2000-core --all-targets --all-features -- -D warnings -D missing_docs -D clippy::doc_markdown` to verify doc links, doc-tests, and coverage.

### Type Design

- Prefer **newtypes** over raw primitives when a value has a distinct semantic meaning (e.g., `Addr(u16)` instead of raw `u16`). See `state/types.rs` for the established pattern.
- **Prefer named structs over multi-element tuples in public APIs**: Define small documented structs with named fields instead of returning or accepting unlabeled tuples (`AddrSpan`, `OffsetSpan`, `IrqVector`, `EffectiveTarget`, `DecodedPacket`, `ParsedCrt`, `ExpandedBlocks`, `AnalysisOutcome`, `DetectedPacker`, `ResolvedEnumValue`, `ViceLabel`).
- Derive `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` for all small value types. Add `Hash` when the type is used as a map key, `Ord`/`PartialOrd` when ordering is needed.
- Implement standard library traits (`Display`, `From`/`TryFrom`, `Error`) where appropriate instead of custom conversion or formatting methods.
- Implement `Default` for options/config structs (`UnpackConfig`, `DocumentSettings`); use `#[derive(Default)]` and `#[default]` on enum variants when zero-values apply, or an explicit `Default` impl for non-zero defaults.
- **Avoid `bool` parameters in public APIs**: Use descriptive two-variant enums instead (`AssetReload`, `Reanalysis`, `ExternalXRefs`, `BlockItemTarget`, `LabelLocality`, `PetsciiCharset`, `TrimMode`) so call sites are self-documenting and cannot transpose arguments.

### Ownership and Borrowing

- Prefer `&str` over `&String`, `&[T]` over `&Vec<T>`, and `&Path` over `&PathBuf` in function signatures.
- Use `impl Into<String>` (or `impl AsRef<str>`) for constructor arguments that store a `String`, as `System::new` does.
- Avoid unnecessary `.clone()` (`clippy::redundant_clone`) and redundant clone assignments (`buf = src.clone()`, `clippy::assigning_clones`) — pass references or borrow slices where ownership is not needed.
- Pass borrowed views or references instead of unused owned values (`clippy::needless_pass_by_value`).
- When passing closures that only forward to a free function (`|x| f(x)`), write `f` directly (`clippy::redundant_closure`).
- Provide explicit type annotations when type inference would obscure a non-obvious type (such as `.collect()` destination types).

### Collections

- Use `BTreeMap`/`BTreeSet` for address-keyed maps that must be iterated in order (deterministic output for project files). Use `HashMap`/`HashSet` only when iteration order does not matter and performance is critical.
- Prefer `.entry().or_default()` over `if !map.contains_key() { map.insert() }`.

### Formatting and Style

- Run `cargo fmt --all` before committing (enforced by pre-commit hook and CI).
- Prefer `.is_some_and(...)` / `.is_ok_and(...)` over `.map_or(false, ...)` (`clippy::manual_is_variant_and`).
- Prefer `write!(buf, "{x}")` over `buf.push_str(&format!("{x}"))` (`clippy::format_push_string`) and avoid `.map(...).collect::<String>()` formatting loops (`clippy::format_collect`).
- Use inline format capture syntax (`format!("{x}")`, `clippy::uninlined_format_args`) rather than positional `format!("{}", x)`.
- `match` arms that do nothing should use the `_ => {}` form, not `_ => unreachable!()` unless you are genuinely asserting the arm is unreachable.

### Tests

- Every non-trivial function in `regenerator2000-core` should have at least one unit test in the same file (`#[cfg(test)]`).
- Integration tests live in `tests/` and cover cross-module behavior (disassembler output, project serialization, MCP protocol, unpacker corpus).
- **Prefer `.expect("reason")` / `.expect_err("reason")` over `.unwrap()` / `.unwrap_err()`** in all unit tests, integration tests, and doc-tests (`#![cfg_attr(test, allow(...))]` exempts tests from clippy errors, so write clear failure context rather than bare unwraps).
- **Avoid check-then-expect in tests**: Do not write `assert!(x.is_some()); let v = x.expect(...);` — call `.expect("...")` directly or pattern match.
- **Add context strings to non-obvious assertions** (`assert!`, `assert_eq!`), especially inside loops or table-driven test cases.
- Use `AppState::new()` (not a manually built struct) as the test baseline; it sets a safe throwaway config path that never touches the real user config.

### Clippy Configuration

Lint policy is defined in `[workspace.lints.clippy]` in the root `Cargo.toml`. All crates inherit it via `[lints] workspace = true` in their own `Cargo.toml` — do not add per-crate `[lints.clippy]` blocks.

Currently enforced (error in CI via `-D warnings`):
| Lint | Category |
|---|---|
| `unwrap_used` | Panic safety |
| `expect_used` | Panic safety |
| `panic` | Panic safety |
| `must_use_candidate` | API hygiene |
| `missing_errors_doc` | Documentation |
| `missing_panics_doc` | Documentation |

Aspirational / Readability Checks (apply to new code, enforced via `-D` on `regenerator2000-core` readability passes):
| Lint | Category / Apply when |
|---|---|
| `missing_docs` | Public API docs in `regenerator2000-core` |
| `clippy::doc_markdown` | Backtick code/identifiers in doc comments |
| `clippy::redundant_clone` | Unnecessary `.clone()` calls |
| `clippy::assigning_clones` | Reassigning via `.clone()` instead of `clone_from` / slice copy |
| `clippy::needless_pass_by_value` | Writing new functions taking unused owned params |
| `clippy::manual_is_variant_and` | Boolean checks on `Option` / `Result` |
| `clippy::uninlined_format_args` | Format string readability |
| `clippy::format_collect` | Building strings in loops (`write!` instead) |
| `clippy::redundant_closure` | Passing single-arg closures (`f` instead of `\|x\| f(x)`) |


---

## Development Workflow

### Commands

```bash
cargo build                                                              # Debug build
cargo run -- [file.prg]                                                  # Run with optional file
cargo test --workspace --all-features                                    # Run all workspace unit, integration, and doc tests
cargo fmt --all -- --check                                               # Check formatting
cargo clippy --workspace --all-targets --all-features -- -D warnings     # Workspace lints (warnings are errors)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features # Verify rustdoc builds and intra-doc links
cargo clippy -p regenerator2000-core --all-targets --all-features -- \
  -D warnings -D missing_docs -D clippy::doc_markdown \
  -D clippy::redundant_clone -D clippy::assigning_clones \
  -D clippy::needless_pass_by_value -D clippy::manual_is_variant_and \
  -D clippy::uninlined_format_args -D clippy::format_collect              # Strict core readability lint pass
```

### Adding Features

- **New Block Type**: Add to `BlockType` enum in `types.rs`, update `analyzer.rs`, and add a shortcut in `input.rs`.
- **New Assembler**: Add to `Assembler` enum, implement `Formatter` trait in `disassembler/formatter_*.rs`.
- **New Command**: Add variant to `Command` in `commands.rs`, implement `apply` and `undo`.
- **New Packer**: Create a new module file in `crates/regenerator2000-core/src/packers/<name>.rs` defining `pub static SPEC: PackerSpec` with declarative `Signature` patterns (`Anchor`, `sig!`), `Hints` (`AddrSource`), optional `patches` (`MemoryPatch`), optional `resolve` escape hatch (`fn(&LoadedImage<'_>, &SignatureMatch) -> Option<ResolvedSpec>`), optional `hooks` factory (`PackerHooks` with `StepContext` and `RefineContext`), and a synthetic `fixture` (`fixture_load_addr`). Export `pub mod <name>;` in `src/packers/mod.rs`, register `&<name>::SPEC` in ascending `priority` order in `REGISTRY` in `src/packers/registry.rs`, add a unit test, and (when a real packed `.prg` fixture is available) add a `[[case]]` entry to `tests/6502/unpack_corpus.toml`, record its `unp64` reference with `cargo run -p regenerator2000-core --bin unpacker_compare_all -- --update`, and update `tests/6502/unpack_baseline.toml` (`UPDATE_UNPACK_BASELINE=1`).

### Git Commit Style

Use present tense, imperative mood (e.g., `Add support for D71 disk images`). Limit first line to 72 chars.

### No Automatic Commits

**Never** create a git commit automatically after completing a task. Only commit when the user explicitly asks for it (e.g., "commit this", "create a commit"). Staged changes (`git add`) are acceptable as part of normal workflow, but the final `git commit` requires an explicit user instruction.

### Testing

- **Unit Tests**: Located in `src` files (e.g., `cpu.rs`).
- **Integration Tests**: Located in `tests/` directory. Use these for testing disassembler output, project serialization, and MCP functionality.
- **Verification**: Always run `cargo test --workspace --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` before finalizing changes.
