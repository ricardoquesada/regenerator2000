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

### CI Checks

CI runs on every push and PR to `main` across macOS, Linux, and Windows. All of these must pass:

```bash
cargo fmt -- --check              # Formatting (Linux only)
cargo clippy -- -D warnings       # Lint — warnings are errors (all OSes)
cargo build --verbose             # Build (all OSes)
cargo test --verbose              # Tests (all OSes)
cargo audit                       # Dependency vulnerability scan (Linux only)
```

## Architecture

See the [Architecture](docs/architecture.md) documentation for the full data flow diagram, key modules, and instructions for adding new features.

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

- No `.unwrap()` or `.expect()` in production code (enforced by `clippy::unwrap_used` and `clippy::expect_used` deny)
- Add `#[must_use]` to every pure function (enforced by `clippy::must_use_candidate`)
- All `pub` items in `regenerator2000-core` must have doc comments (`///`), including `# Errors` sections for fallible functions
- All mutations to `AppState` go through `Command::apply()` for undo/redo support
- Prefer `Result`/`Option` with `?` propagation over manual `match`/`if let` error handling
- Use `anyhow::Result` for application-level errors; typed `thiserror` enums for library-facing error surfaces
- Prefer newtypes over raw primitives (e.g., `Addr(u16)` instead of raw `u16`)
- Use `BTreeMap`/`BTreeSet` for address-keyed maps that must iterate in order
- Follow standard `cargo fmt` formatting
