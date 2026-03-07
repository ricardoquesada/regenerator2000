# Contributing to Regenerator 2000

First off, thanks for taking the time to contribute!

The following is a set of guidelines for contributing to Regenerator 2000. These are just guidelines, not rules. Use your best judgment, and feel free to propose changes to this document in a pull request.

## Dev Setup

### Prerequisites

- **Rust** (stable toolchain) — install via [rustup](https://rustup.rs/)
- **64tass** (optional) — needed for assembler verification tests on Linux:
  ```bash
  sudo apt-get install 64tass          # Debian / Ubuntu
  brew install 64tass                  # macOS (Homebrew)
  ```
- **cargo-audit** (optional) — for dependency vulnerability scanning:
  ```bash
  cargo install cargo-audit
  ```

### Clone & Build

```bash
git clone https://github.com/ricardoquesada/regenerator2000.git
cd regenerator2000
cargo build
```

### Run

```bash
cargo run -- [path/to/file.prg]    # Open a binary file
cargo run                          # Launch without a file
```

## Testing

### Running Tests

```bash
cargo test                                     # Run all tests
cargo test --test disassembler_tests           # Run a single integration test file
cargo test --test parser_malformed_input_tests # Run parser edge-case tests
```

### Test Categories

| Test file | Coverage |
|-----------|----------|
| `tests/parser_malformed_input_tests.rs` | Malformed input for D64, CRT, T64, VSF parsers |
| `tests/formatter_64tass_tests.rs` | 64tass assembler output formatting |
| `tests/formatter_acme_tests.rs` | ACME assembler output formatting |
| `tests/formatter_ca65_tests.rs` | ca65 assembler output formatting |
| `tests/formatter_kickasm_tests.rs` | KickAssembler output formatting |
| `tests/vice_protocol_tests.rs` | VICE binary monitor protocol encode/decode |
| `tests/theme_loading_tests.rs` | Theme loading, defaults, and fallbacks |
| `tests/config_serialization_tests.rs` | Config JSON round-trip and backward compat |
| `tests/exporter_tests.rs` | Full assembly export pipeline |
| `tests/disassembler_tests.rs` | End-to-end disassembly tests |

### CI Checks

CI runs on every push and PR to `main`. All of these must pass:

```bash
cargo fmt -- --check              # Formatting (Linux only)
cargo clippy -- -D warnings       # Lint — warnings are errors
cargo build --verbose             # Build
cargo test --verbose              # Tests (macOS, Linux, Windows)
cargo audit                       # Dependency vulnerability scan (Linux only)
```

## Architecture

The project follows a unidirectional data flow (Redux/Elm style). See [`AGENTS.md`](AGENTS.md) for the full architecture documentation, including the data flow diagram, key modules, and instructions for adding new features.

## How Can I Contribute?

### Reporting Bugs

- **Check if the bug has already been reported**.
- **Use the Bug Report template**: When you open a new issue, please fill out the Bug Report template with as much detail as possible.

### Suggesting Enhancements

- **Check if the enhancement has already been suggested**.
- **Use the Feature Request template**: When you open a new issue, please fill out the Feature Request template.

### Pull Requests

- Fill in the required template
- Do not include issue numbers in the PR title
- Include screenshots and animated GIFs in your pull request whenever possible
- End all files with a newline
- Ensure `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` all pass locally before submitting

## Styleguides

### Git Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests liberally after the first line

### Rust Code

- No `.unwrap()` or `.expect()` in production code (enforced by `clippy::unwrap_used` deny)
- All mutations to `AppState` go through `Command::apply()` for undo/redo support
- Follow standard `cargo fmt` formatting
