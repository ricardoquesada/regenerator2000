# Regenerator 2000 - Agent Instructions

Regenerator 2000 is an interactive 6502 disassembler TUI written in Rust, targeting Commodore 8-bit machines. It uses [ratatui](https://github.com/ratatui-org/ratatui) + crossterm for the terminal UI and exposes an MCP (Model Context Protocol) server for programmatic access.

## Architecture & Crate Structure

The project is split into two main crates to separate logic from presentation:

- **`regenerator2000-core`**: Contains all UI-agnostic logic.
  - `analyzer.rs`: Auto-analysis (walking code, generating labels).
  - `assets.rs`: System asset loading (labels, comments, exclude lists).
  - `commands.rs`: The `Command` enum and `UndoStack`. Every undoable action is a command.
  - `config.rs`: System-wide configuration management.
  - `core.rs`: Central orchestration logic (file loading, command dispatch, integration).
  - `cpu.rs`: 6502 opcode table and addressing mode logic.
  - `disassembler/`: 6502 decoding and assembler formatters (64tass, ACME, KickAssembler, ca65).
  - `event.rs`: Application event types.
  - `exporter/`: Code for exporting disassembly to ASM or HTML, with roundtrip verification.
  - `mcp/`: MCP server implementation (stdio and HTTP transports).
  - `navigation.rs`: Address navigation logic.
  - `parser/`: File format parsers (.prg, .d64/.d71/.d81, .t64, .crt, .vsf snapshots, .dis65, VICE labels).
  - `state/`: Persistent application state and project serialization.
  - `utils.rs`: Shared utility functions.
  - `vice/`: VICE binary monitor protocol client for live debugging.
  - `view_state.rs`: View state management.

- **`regenerator2000-tui`**: Contains all terminal UI-related code.
  - `ui/`: TUI widgets, views (Disassembly, HexDump, Sprites, Charset, Bitmap, Blocks, Debugger), and dialogs.
  - `ui_state.rs`: Transient UI state (cursor, scroll, active dialog).
  - `events/`: Input handling and event loop.
  - `theme.rs`: Color scheme and styling logic.
  - `theme_file.rs`: Theme file parsing and custom theme support.
  - `utils.rs`: TUI utility functions.

## Core Domain Model

### Key Types (`crates/regenerator2000-core/src/state/types.rs`)

- **`Addr`**: A wrapper around `u16` representing a 6502 address.
- **`System`**: Target machine (C64, C128, VIC20, PET, etc.).
- **`Assembler`**: Target assembler syntax (Tass64, Acme, Ca65, Kick).
- **`BlockType`**: How a range of bytes is interpreted (Code, DataByte, DataWord, Address, PetsciiText, ScreencodeText, LoHiAddress, HiLoAddress, LoHiWord, HiLoWord, ExternalFile, Undefined).
- **`LabelType`**: Semantic meaning of a label (Subroutine, Jump, Branch, Pointer, ZeroPagePointer, Field, ZeroPageField, AbsoluteAddress, ZeroPageAbsoluteAddress, ExternalJump, Predefined, UserDefined, LocalUserDefined).

### State Management

- **`AppState`** (`crates/regenerator2000-core/src/state/app_state.rs`): The single source of truth for persistent data. Includes `raw_data`, `block_types`, `labels`, `user_side_comments`, `user_line_comments`, `undo_stack`, `settings`, `cross_refs`, `bookmarks`, and `scopes`. Serialized to `.regen2000proj`.
- **`UIState`** (`crates/regenerator2000-tui/src/ui_state.rs`): Transient state like cursor positions, scroll offsets, and active dialogs. Never serialized.

## Application Logic Flow

Regenerator 2000 follows a unidirectional data flow (Redux/Elm style):

1. **Input**: `crates/regenerator2000-tui/src/events/input.rs` maps key events to `MenuAction`.
2. **Dispatch**: `MenuAction` is converted into one or more `Command` variants.
3. **Execution**: `Command::apply(&mut AppState)` modifies the state and pushes to `UndoStack`.
4. **Analysis**: `AppState::analyze()` (via `analyzer.rs`) updates labels and cross-references.
5. **Render**: `ui()` in `crates/regenerator2000-tui/src/ui.rs` re-renders the TUI from `AppState` + `UIState`.

### No Logic Duplication — State Logic Lives in Core

All validation, business rules, and state-mutation logic **must** live in `regenerator2000-core` (typically as methods on `AppState` in `state/`). Both the TUI (`regenerator2000-tui`) and the MCP server (`mcp/handler.rs`) are **consumers** of core logic — they must call shared `AppState` methods, not re-implement the same checks independently.

For example, if label creation needs to reject duplicate names, that validation belongs in an `AppState` method (e.g., `create_set_user_label_command`). The MCP handler and the TUI dialog both call that single method. **Never** add domain logic directly into `mcp/handler.rs` or `events/input.rs` if it can be expressed as a core method.

When adding or modifying a feature, ask: *"Would this logic need to be duplicated if a third client (e.g., a GUI, a CLI) were added?"* If yes, it belongs in `regenerator2000-core`.

## AI Agent Skills (`.agent/skills/`)

Specialized tools are available for common tasks. Invoke them via `activate_skill`:

- `r2000-analyze-basic`: Analyze BASIC programs and mark pointers.
- `r2000-analyze-blocks`: Auto-detect and set block types (text, data).
- `r2000-analyze-program`: Orchestrate full-program analysis using subagents for blocks, routines, and symbols.
- `r2000-analyze-routine`: Trace a subroutine and mark code paths.
- `r2000-analyze-symbol`: Find and label all references to a specific address.
- `add-mcp-tool`: Templates for adding new programmatic tools to the MCP server.
- `bump-version`: Automate version bumping and changelog updates.
- `coding`: General Rust coding assistance with project context.
- `code-review`: Review changes for idiomatic Rust and project conventions.
- `update-keyboard-shortcuts`: Sync keyboard shortcuts across docs and source files.
- `update-mcp-docs`: Sync `docs/mcp.md` with the actual MCP handler tools.
- `verify-mcp`: Run MCP integration test suite to verify server functionality.

## Rust Best Practices

All AI-generated code MUST follow the rules below. The project enforces them through `cargo clippy -- -D warnings`; violations will block CI.

### Panic Safety

Production code must never panic silently.

- **Never use `.unwrap()` or `.expect()`** outside of `#[cfg(test)]` blocks. Both are banned by `clippy::unwrap_used` / `clippy::expect_used`.
- **Return `Result` or `Option`** and propagate errors with `?`.
- Use `anyhow::Result` for application-level errors where context strings suffice, and typed `thiserror` enums for library-facing error surfaces.
- `unreachable!()` inside exhaustive `match` arms (e.g. an enum variant that logically cannot appear) is acceptable. All other panic macros require a comment explaining why the invariant holds.
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
- MCP handler functions must return explicit `McpError` variants for invalid/missing parameters — see `mcp/handler.rs` for the established pattern.

### API Hygiene (`#[must_use]`)

- Add `#[must_use]` to every **pure** function (one with no side effects whose return value carries meaning). This includes constructors (`fn new`), getters, and any function returning `bool`, a numeric type, or a wrapped newtype.
- Functions that mutate state (`&mut self`) and return `()` do **not** need `#[must_use]`.
- `#[must_use]` on an `impl` block annotates every method — use sparingly; prefer per-method annotations.
- Run `cargo clippy -- -W clippy::must_use_candidate` to find unannotated candidates in existing code.

### Documentation

- All `pub` items in `regenerator2000-core` must have a doc comment (`///`).
- Functions returning `Result` must include a `# Errors` section explaining when they fail.
- Functions that can panic (outside tests) must include a `# Panics` section.
- Use intra-doc links (`` [`TypeName`] ``) to cross-reference types. Run `cargo doc --no-deps` to verify they resolve.
- Run `cargo clippy -- -W clippy::missing_errors_doc -W clippy::missing_panics_doc` to audit doc coverage.

### Type Design

- Prefer **newtypes** over raw primitives when a value has a distinct semantic meaning (e.g. `Addr(u16)` instead of raw `u16`). See `state/types.rs` for the established pattern.
- Derive `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` for all small value types. Add `Hash` when the type is used as a map key, `Ord`/`PartialOrd` when ordering is needed.
- Implement `Display` for types shown to users; implement `From`/`Into` conversions where they remove boilerplate.
- Use `Default` for structs with sensible zero-values; use `#[default]` on enum variants rather than a manual `Default` impl.
- Avoid `bool` parameters in public APIs; a two-variant enum is self-documenting and prevents argument-order bugs.

### Ownership and Borrowing

- Prefer `&str` over `&String`, `&[T]` over `&Vec<T>`, and `&Path` over `&PathBuf` in function signatures.
- Use `impl Into<String>` (or `impl AsRef<str>`) for constructor arguments that store a `String`, as `System::new` does.
- Avoid unnecessary `.clone()` — pass references where ownership is not needed.
- When passing closures that only forward to a free function (`|x| f(x)`), write `f` directly (`clippy::redundant_closure`).

### Collections

- Use `BTreeMap`/`BTreeSet` for address-keyed maps that must be iterated in order (deterministic output for project files). Use `HashMap`/`HashSet` only when iteration order does not matter and performance is critical.
- Prefer `.entry().or_default()` over `if !map.contains_key() { map.insert() }`.

### Formatting and Style

- Run `cargo fmt` before committing (enforced by pre-commit hook and CI).
- Prefer `write!(buf, "{x}")` over `buf.push_str(&format!("{x}"))` (`clippy::format_push_string`).
- Use `format!` capture syntax: `format!("{x}")` not `format!("{}", x)` (Rust 2021+).
- `match` arms that do nothing should use the `_ => {}` form, not `_ => unreachable!()` unless you are genuinely asserting the arm is unreachable.

### Tests

- Every non-trivial function in `regenerator2000-core` should have at least one unit test in the same file (`#[cfg(test)]`).
- Integration tests live in `tests/` and cover cross-module behavior (disassembler output, project serialization, MCP protocol).
- Tests may freely use `.unwrap()` / `.expect()` — the per-crate `lib.rs` allows this under `#[cfg(test)]`.
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

Aspirational (run manually, apply to new code):
| Lint | Apply when |
|---|---|
| `needless_pass_by_value` | Writing new functions |
| `redundant_closure` | Writing closures |


---

## Development Workflow

### Commands

```bash
cargo build                      # Debug build
cargo run -- [file.prg]          # Run with optional file
cargo test                       # Run all tests
cargo fmt -- --check             # Check formatting
cargo clippy -- -D warnings      # Lint (warnings are errors)
```

### Adding Features

- **New Block Type**: Add to `BlockType` enum in `types.rs`, update `analyzer.rs`, and add a shortcut in `input.rs`.
- **New Assembler**: Add to `Assembler` enum, implement `Formatter` trait in `disassembler/formatter_*.rs`.
- **New Command**: Add variant to `Command` in `commands.rs`, implement `apply` and `undo`.

### Git Commit Style

Use present tense, imperative mood (e.g., `Add support for D71 disk images`). Limit first line to 72 chars.

### Testing

- **Unit Tests**: Located in `src` files (e.g., `cpu.rs`).
- **Integration Tests**: Located in `tests/` directory. Use these for testing disassembler output, project serialization, and MCP functionality.
- **Verification**: Always run `cargo test` and `cargo clippy` before finalizing changes.
