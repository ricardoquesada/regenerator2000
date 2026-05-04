# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.13] - 2026-05-03

### Features

- **Configuration**: Migrate configuration format from JSON to TOML with automatic legacy file conversion.
- **Themes**: Move built-in themes from Rust code to embedded TOML assets; add custom theme support via TOML files.
- **Disk Images**: Add support for D71 80-track, and D64 42-track disk images.
- **Hexdump**: Implement byte-value color palettes and updated hex dump UI layout for improved readability.
- **Charset**: Show cursor address in charset view title and improve cursor visibility.
- **Labels**: Add centralized label validation and helper for setting user labels with duplicate name detection.
- **CLI**: Display config directory path in `--help` output.
- **UI**: Add Delete key to exclude external addresses from analysis.

### Fixes

- **Labels**: Exclude external addresses from being marked as unexplored in disassembly view.

### Changes

- **Labels**: Update label naming convention to use underscored prefixes (`s_`, `j_`, `zpf_`, `p_`, `f_`, `a_`) for improved searchability.
- **Platform**: Rename 'system' to 'platform' across the codebase.

### Refactor / Internal

- **Hexdump**: Remove entropy visualization from hexdump view.

### Documentation

- **Analysis**: Add analysis documentation and update label prefix references.
- **Settings**: Update settings documentation with platform-specific system label and analysis options.
- **CLI**: Add `--dump-system-config-files` CLI documentation and system config override workflow.
- **Themes**: Move custom theme documentation from install.md to themes.md.

### Tools

- **Verifier**: Add verifier tool for project binary reproducibility checks.

### Agent / Skills

- **Skills**: Add `r2000-analyze-program` orchestration skill for full-program analysis.
- **Skills**: Generalize `r2000-analyze-symbol` and `r2000-analyze-routine` instructions by removing platform-specific C64 references.
- **Skills**: Update subroutine and data symbol naming conventions in analysis skill documentation. Also, they should not ask user for confirmation.

### Examples

- **C64**: Updated Burnin' Rubber, Kikstart, and Moving Tubes with additional labels, comments, and project metadata.

## [0.9.12] - 2026-04-27

### Features

- **Debugger**: Added TED chip register dump view for the Commodore Plus/4 platform.
- **CLI**: Added `--export_html` command-line option; `--mcp-server` and `--headless` are now mutually exclusive.
- **UI**: Added `Ctrl+Enter` and `Alt+Enter` as alternative shortcut for newline in Comment dialog.
- **UI**: Added C64 boot-screen easter egg to About dialog with blinking cursor, typing animation, and auto-close.
- **UI**: Added `handle_tick` mechanism to UI widgets for auto-close functionality.

### Fixes

- **Labels**: Preserve `label_type` when renaming external labels; external labels now belong to correct group.
- **HTML Export**: Cross-references now rendered in external label definition lines.
- **HTML Export**: External labels exported with correct grouping.
- **Platform**: Improved system comment consistency across different platforms; better "excluded" handling for platforms.

### Changes

- **Labels**: Renamed `exclude_comments_from_well_known` to `exclude_well_known_labels` with updated semantic.

### Refactor / Internal

- **Rust**: Enforced `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc` clippy lints.
- **Rust**: Applied modern Rust best practices and documented them in `AGENTS.md`.
- **Dependencies**: Updated dependencies.
- **HTML Export**: Updated CSS layout for code cells.

### Examples

- **C64**: Annotated Burnin' Rubber with additional labels, comments, and documentation.
- **VIC-20**: Extensive analysis and annotations for Omega Race.
- **Plus/4**: Added Kikstart Plus/4 example project with labels and comments.
- **PET**: Updated Lode Runner example with annotations.

## [0.9.11] - 2026-04-22

### Features

- **UI**: Added mouse click-to-cursor and drag-to-select support in the line comment dialog.

## [0.9.10] - 2026-04-22

### Features

- **Navigation**: Added "Jump to next/prev unexplored block" to the Jump menu.
- **Disassembly**: Force a new line when a side comment is present on an grouped block.
- **HTML Export**: Added support for opening external documentation and examples links via browser.
- **Analysis**: Fixed automatically displayed x-refs for split tables.

### Fixes

- **Disassembly**: Fixed `.fill` not emitted when preceded by non-matching bytes in same block.
- **UI**: Adjusted mnemonic column width for improved side-comment alignment.
- **File Dialog**: Correctly show parent directly.
- **Dependencies**: Updated dependencies to fix security vulnerabilities in CI.
- **Platform**: VIC-20 system labels/comments now correctly identified and displayed in Document Settings Dialog.

### Refactor / Internal

- **UI**: Updated search dialog to use persisted UI state for query and filters.
- **MCP**: Added strict input validation for MCP handler parameters.
- **Rust**: Applied Clippy fixes for Rust 1.96.0.

### Examples / Research

- **VIC-20**: Extensive reverse engineering and documentation for the Omega Race cartridge.
- **C64**: Annotated Burnin' Rubber collision system routines and tables.

## [0.9.9] - 2026-04-15

### Features

- **Disassembly**: Add `.fill` directive support for contiguous identical byte runs (configurable threshold, default 8); runs are suppressed when a cross-reference, line-comment, or side-comment is present at any interior address. Assembler-specific directives: `.fill` (64tass, KickAssembler), `!fill` (ACME), `.res` (ca65).
- **UI**: Add drag-to-select and Shift+Click mouse selection in the disassembly view.
- **HTML Export**: Show assembler-specific build instructions in the HTML export header.
- **HTML Export**: Add clickable hyperlinks to example disassembly files from assembler homepage links.
- **MCP**: Rename and extend `r2000_get_symbols` and `r2000_get_comments` tools with optional address-range and type filters.
- **Document Settings**: Add `Fill run threshold` option (0 = disabled) to the Document Settings dialog with left/right arrow and direct-edit support; max value 64.

### Fixes

- **HTML Export**: Fix x-ref relative address link alignment in disassembly output.
- **Document Settings**: Fix navigation bounds and number input dialog in Document Settings.

### Documentation

- **Settings**: Document the Fill run threshold option with per-assembler directive table and 64tass example.
- **Architecture**: Update architecture docs to reflect HTML/verification exporters and the Import Context dialog.
- **Examples**: Add detailed comments and labels to Moving Tubes and PET Lode Runner example projects.

### Agent / Skills

- **Skills**: Refine `r2000-analyze-routine` and `r2000-analyze-basic` skill workflows.

## [0.9.8] - 2026-04-11

### Features

- **UI**: Implement interactive cursor navigation and text editing capabilities in all dialog prompts.
- **HTML Export**: Added assembler homepage hyperlinks to generated HTML headers.
- **HTML Export**: Improved aesthetics to match GitHub themes and implemented clickable instruction anchors.

### Fixes

- **Core**: Split transient export paths for ASM and HTML configurations.
- **Core**: Prevent internal side comment propagation in nested Address blocks.

### Documentation

- **Examples**: Updated descriptions and links for C64 and PET projects.

## [0.9.7] - 2026-04-10

### Features

- **HTML Export**: Added standalone HTML external file modules, inline hyperlink navigation, anchors, theme toggling, anchors, variable links, and dynamic layout logic.
- **Import**: Automatically suggest entry points by parsing SYS addresses in multi-line BASIC PRGs.
- **Navigation**: Added support for directory navigation history and parent folder rules in open dialogs.
- **Agent**: Added skill to parse BASIC tokens.

### Fixes

- **UI**: Added hex prefix (`$`) to addresses across the status bar.
- **UI**: Centered position contexts automatically for input prompts in full-range address modal windows.
- **Core**: Fixed context-level splitter handling in raw-analysis blocks.
- **Blocks**: Synchronized UI navigation contexts efficiently for blocks sharing duplicative base addresses.

### Refactor / Internal

- **Exporter**: Modularized formatting rules into independent files (asm / html).
- **Exporter**: Delegated formatting of native inclusions to formatter definitions instead of maintaining hardcoded match constraints.
- **Refactor**: Replaced raw string platform formats with constants natively.
- **Refactor**: Restrict recent project contexts to `.regen2000proj` files.

## [0.9.6] - 2026-04-04

### Features

- **Import**: Automatically detect platform from VSF header and suggest entry point by parsing SYS address in PRG files.
- **Import**: New Import Context dialog that asks for Platform, Origin, Entry Point when importing a new file.
- **UI**: Added local/global scope selection to the label dialog.
- **UI**: Implemented mouse interaction for the minimap to navigate the disassembly view.

### Fixes

- **UI**: Fixed `LocalOrGlobalAddr` label heuristic and visual equates for .dis65.

### UI / UX

- **UI**: Made minimap cursor color theme-dependent.

### Documentation

- **User Guide**: Added documentation for local vs. global label scoping and cleaned up formatting in tutorial and FAQ.
- **User Guide**: Updated screenshots in documentation.

### Refactor / Internal

- **Refactor**: Centralized PRG parsing and refactored container formats.
- **Refactor**: Used strongly-typed `Platform` for suggested platforms.
- **Refactor**: Removed redundant manual session restoration logic in favor of `ui_state.restore_session`.

## [0.9.5] - 2026-03-31

### Features

- **Project**: Added support for 6502bench (`.dis65`) project files.
- **Navigation**: Added support for "Disassemble address" (keyboard shortcut 'd') and flow analysis for code block detection.
- **UI**: Added a high-precision horizontal minimap bar with sub-character precision ticks.
- **UI**: Added configurable default block type and updated TUI settings menu.
- **UI**: Dim unexplored code blocks in disassembly view.

### Fixes

- **UI**: Fix block type color consistency in minimap bar.
- **UI**: Update cursor position after analysis.

### Documentation

- **User Guide**: Updated tutorial, settings, and roadmap to reflect new features and keyboard shortcuts.
- **Roadmap**: Expanded phase 5 scope and added platform-specific v1.x releases.

## [0.9.4] - 2026-03-26

### Features

- **Debugger**: Highlight changed register, memory, and vector values in the debugger view by comparing with previous state.
- **Exporter**: Place long labels on their own row in exported `.asm` files for better readability.

### Fixes

- **UI**: Show the offending address and valid range in the status bar when jump-to-address or jump-to-line targets are out of range (e.g. `Address $FE00 out of range ($0801-$2800)`).
- **Exporter**: Fix screencode and PETSCII roundtrip for ca65 and KickAssembler by emitting problematic characters as raw hex bytes.

### Documentation

- **User Guide**: Updated keyboard shortcuts, documented memory dump and scope features, and fixed settings shortcut mapping.
- **User Guide**: Added ca65 warning about one-pass assembler limitations.

### Refactor / Internal

- **Assembler**: Moved scope resolution separator to assembler-specific formatters instead of hardcoding.

## [0.9.3] - 2026-03-23

### Features

- **Debugger**: Added memory dump viewer to the debugger UI.
- **Scopes**: Added comprehensive support across 64tass, ca65, and KickAssembler. Scope is like a namespace, think of ".proc" / ".endproc" in ca65.
- **Scopes**: Added TUI support for adding, renaming, and deleting scopes with proper indentation, splitters, gutters, boundaries, and default `scope_{ADDRESS}` auto-generated labels.
- **MCP Server**: Added `r2000_add_scope` MCP tool.
- **UI**: Improved text input widgets with full cursor movement and editing capabilities for document settings and labels.
- **UI**: Simplified splitter rendering in disassembly view.

### Fixes

- **UI**: Fix disassembly view arrow display bugs.
- **Navigation**: Store correct external label addresses in navigation history instead of default addresses.
- **Stability**: Fix dependency vulnerabilities in `rustls-webpki`.
- **Undo/Redo**: Fix redundant undo actions by grouping state and analysis commands.

### Documentation

- **API**: Added comprehensive module-level documentation and enforced clippy lints for core and TUI crates.
- **User Guide**: Documented the new Scope feature and updated keyboard shortcuts.
- **Examples**: Updated example projects to demonstrate scopes, local labels, and new features.

### Refactor / Internal

- **Testing**: Added unit tests for overlapping and nested scopes checking.

## [0.9.2] - 2026-03-16

### Features

- **UI**: Added a check to avoid drawing bookmark tags on wrapped label lines to reduce visual noise.

### Refactor / Internal

- **Crate Renaming**: Renamed internal crates `regenerator-core` and `regenerator-tui` to `regenerator2000-core` and `regenerator2000-tui` for consistency with the project name.
- **Documentation**: Updated all documentation and agent instructions to reflect the new crate names and structure.

## [0.9.1] - 2026-03-15

### Features

- **UI**: Added a new TUI logo.

### Refactor / Internal

- **System Assets**: Relocated system definition assets to `regenerator2000-core` for better separation of concerns and added new system configurations.

## [0.9.0] - 2026-03-15

### Major Architectural Refactor

- **Core/TUI Separation**: The project has been restructured into a multi-crate workspace:
  - `regenerator2000-core`: Contains all UI-agnostic logic, including state management, disassembler, analyzer, commands, and the MCP server.
  - `regenerator2000-tui`: Contains the terminal user interface logic, widgets, and event loop.
  - `regenerator2000`: The main binary crate that ties everything together.
- **Improved Data Flow**: Transitions to a cleaner unidirectional data flow where the UI dispatches semantic `AppAction`s, which are then applied to the `AppState` via a command system with full undo/redo support.
- **Type Safety**: Introduced `Addr` and `Platform` newtypes to improve type safety and ensure consistent memory address handling throughout the codebase.
- **MCP Migration**: Relocated the MCP server to the core crate, making it available for programmatic access without requiring the TUI.

### Features

- **UI Enhancements**:
  - Implemented "smart jump" logic in the disassembly view to minimize unnecessary scrolling when the target address is already visible.
- **Analysis**: Refactored disassembler arrow computation for improved efficiency and accuracy.
- **Theme Previews**: Added screenshots of available themes to the documentation.

### Fixes

- **UX**: Fixed dialog confirmation and closing logic to ensure consistent behavior across all modal windows.
- **MCP**: Fixed bounds checking for the `set_data_type` MCP tool.
- **Stability**: Refined Clippy rules and fixed several minor stability issues discovered during the architectural refactoring.

### Documentation

- **Architecture**: Comprehensive update of the architecture documentation, including new Mermaid diagrams and descriptions of the crate structure.
- **Project Documentation**: Added `AGENTS.md` and updated `CONTRIBUTING.md` with new project instructions and best practices.

## [0.8.8] - 2026-03-09

### Features

- Feature: **Settings**: Control automatic analysis via `auto_analyze` setting; removed file-type-based auto-analysis on file open.
- Feature: **VICE Debugger**: Display watchpoint stop reason in debugger view.
- Feature: **Events**: Introduce `EventOutcome` enum to control event loop flow with dedicated `KeyEvent` and `MouseEvent` handling.

### Fixes

- Fix: **Testing**: Use default `SystemConfig` in `AppState::new()` to fix test flakiness from environment bleed.
- Fix: **Security**: Update `quinn-proto` to 0.11.14 to fix RUSTSEC-2026-0037.

### Refactor / Internal

- Refactor: **Events**: Centralize dialog closing logic by adding `closes_dialog` to `MenuAction`.
- Refactor: **VICE**: Extract VICE message parsing logic into dedicated typed functions and structs in `vice/protocol.rs`.
- Refactor: **Disassembler**: Simplify handler function signatures with new `HandleArgs` struct.
- Refactor: **Main**: Add utility functions for file classification, CLI validation, image loading, batch operations, and roundtrip verification.
- Refactor: Apply widespread code quality improvements and address Clippy warnings.
- Chore: Exclude docs/tests/agent files not needed for crates.io packaging.

## [0.8.7] - 2026-03-07

### Features

- Feature: **UI**: Long labels are now rendered on their own line above the instruction to preserve indentation.
- Feature: **VICE Debugger**: Visual indicators for breakpoints and watchpoints (flashing status line, terminal bell).
- Feature: **CLI**: Added `--vice` flag to auto-connect to VICE binary monitor on startup.

### Fixes

- Fix: **UI**: Centered text in the unsaved changes confirmation dialog.

### Documentation

- Docs: Documented `--vice` CLI flag in README and `docs/debugger.md`.
- Docs: Expanded `CONTRIBUTING.md` and added `cargo audit` to CI.

### Refactor / Internal

- Test: Added comprehensive test coverage for parsers, formatters, VICE protocol, themes, and config.

## [0.8.6] - 2026-03-06

### Features

- Feature: **Themes**: Added Nord, Catppuccin Mocha, and Catppuccin Latte themes.
- Feature: **Themes**: Default theme changed to Dracula; added left/right arrow cycling in theme selector.
- Feature: **UI**: Added blinking cursor to all input dialogs.
- Feature: **Project**: Added version field and migration logic to `.regen2000proj` file format.

### Fixes

- Fix: **UI**: Improved dialog UX consistency (theming, centering, dead code cleanup).
- Fix: **UI**: Added `$` prefix to Jump To Address dialog for hex clarity.

### Changes

- Chore: Migrated to ratatui 0.30.0.

### Documentation

- Docs: Updated themes and settings documentation for 9 themes, Dracula default, and new UX features.
- Docs: Updated CLI section in README.

## [0.8.5] - 2026-03-05

### Features

- Feature: **Export**: Support for undocumented (illegal) opcodes in export and verify.
- Feature: **CLI**: Added `--assembler` flag to override assembler format in headless mode.
- Feature: **CLI**: Integrated `clap` crate for robust command-line argument parsing with `--help` and shell completions.
- Feature: **MCP**: `get_binary_info` tool now hints whether the binary uses undocumented opcodes.
- Feature: **UI**: Added Gruvbox Light theme.
- Feature: **Release**: Added Linux ARM64 (aarch64) binary builds.

### Fixes

- Fix: **ACME Exporter**: Fixed zero-page address handling that caused byte mismatches during roundtrip verification.
- Fix: **ACME Exporter**: Fixed illegal opcode mnemonics for ACME assembler compatibility.
- Fix: **Stability**: Replaced `println!`/`eprintln!` with logging macros to prevent TUI corruption.

### Documentation

- Docs: Added themes documentation page with screenshots.
- Docs: Documented `--verify` CLI option and assembler setup requirements.
- Docs: Added KickAssembler `KICKASS_JAR` environment variable instructions.

### Changes

- Chore: Track `Cargo.lock` for reproducible binary builds.

## [0.8.4] - 2026-03-03

### Features

- Feature: **Search**: Added filter checkboxes to the search dialog for granular control.
- Feature: **Analyzer**: Added analysis hints with cross-instruction pattern detection.
- Feature: **Export**: Added roundtrip export verification (export → assemble → diff).
- Feature: **UI**: Check for new version and display it in the top-right corner.

### Fixes

- Fix: **Stability**: Eliminated all `unwrap`/`panic` from production code, added parser fuzz tests.

### Refactor / Internal

- Refactor: **Search**: Centralized search logic into `state/search.rs`.
- MCP: Simplified search encoding to `text` and `hex`.

### Documentation

- Docs: FAQ and tutorial heavily improved.

## [0.8.3] - 2026-03-01

### Features

- Feature: **VICE Debugger**: Shift+F2 shortcut opens dialog to toggle breakpoints.
- Feature: **VICE Debugger**: Show 6502 hw vectors info for all platforms.
- Feature: **VICE Debugger**: Show 6510 registers when platform is C64 / C128.
- Feature: **VICE Debugger**: Show VIC/CIA registers when platform is C64 / C128.
- Feature: **UI**: Hexdump view supports selecting columns, not just rows.

### Fixes

- Fix: **Hexdump**: Fixed hexdump and disassembly synchronization issues.
- Fix: **Disassembler**: Export to ASM and view disassembler fixes.

### Documentation

- Docs: Added VICE note that both VICE and Regenerator must run the same binary.
- Docs: View documentation fixes, reordered content, and added more keyboard shortcuts.
- Examples: Updated example projects.

### Refactor / Internal

- Agent: Added `CLAUDE.md` and merged `AGENTS.md` into it.

## [0.8.2] - 2026-02-28

### Documentation

- Docs: Updated `index.md` and `tutorial.md` with comprehensive VICE debugger workflow and documentation.
- Examples: Added more detailed comments and descriptions to example projects.

### Refactor / Internal

- MCP: Simplified MCP server tools by consolidating and renaming them to reduce context and improve agent reliability.
- Agent: Updated `r2000-analyze-blocks`, `r2000-analyze-symbol`, and `r2000-analyze-routine` skills to follow best practices and provide better analysis context.

## [0.8.1] - 2026-02-25

### Features

- Feature: **Line Comment**: `Ctrl+Enter` / `Ctrl+J` creates a new line within the comment dialog (multi-line support).
- Feature: **Line Comment**: Keyboard shortcuts for inserting separator comments (dashes, equals, mixed) from within the dialog.
- Feature: **Dialogs**: Save As, Export As, and Export VICE Labels dialogs now pre-fill the filename with the current project name.
- Feature: **UI**: Application handles terminal resize events and repaints correctly.

### Fixes

- Fix: **LoHi/HiLo Words**: Words no longer resolve to addresses even when a word value matches an existing label address.
- Fix: **Cross-references**: Cross-references now work correctly on relative address sub-indices.
- Fix: **T64 Picker**: File picker handles "noisy" (non-printable) characters in T64 filenames gracefully.

### Documentation

- Docs: Updated settings and views documentation (indentation, Blocks View ordering, Debug View added).
- Examples: Added more detailed comments and descriptions to example projects.

## [0.8.0] - 2026-02-23

### Features

- Feature: **VICE Debugger**: Comprehensive support for VICE remote debugging.
  - Debugger view with CPU status, stack, and registers information.
  - Breakpoints and Watchpoints support.
  - Execution controls: Continue, Step Into, Step Over, Step Out, Run to Cursor.
  - Live Disassembly View
- Feature: **Open Recents**: Added "Open Recent" support for quickly accessing recent files.
- Feature: **UI**: Mouse click on blocks in Blocks View now updates the Disassembly View cursor.
- Feature: **Settings**: Added description field to Document Settings.
- Feature: **MCP**: `get_binary_info` tool now includes the filename, description and platform to aid agent tasks.

### Changes

- UI: Removed "switch view" from MCP as it is not currently implemented.
- UI: Improved navigation in the Document Settings dialog.
- UI: Minor cosmetic fixes in keyboard shortcut dialogs.

### Fixes

- Fix: Keyboard shortcuts handling simplified and corrected for the Debugger integrations (F6, F7, etc).
- Fix: Addressed duplicate issues with initial breakpoint on connection.

### Documentation

- Docs: Added comprehensive VICE debugger documentation and screenshots.
- Docs: Updated MCP documentation to reference Antigravity.
- Examples: Updated descriptions for example projects.

## [0.7.2] - 2026-02-18

### Features

- Feature: **Disassembly View**: Improved arrow visualization for jump instructions (restored full arrows).

### Documentation

- Docs: Improved documentation structure and readability (MkDocs, Homepage, and Tutorials).

### Refactor / Internal

- Agent: Added `r2000-analyze-symbol` and `r2000-analyze-blocks` skills.
- Agent: Renamed skills to use `r2000` prefix to avoid conflicts.
- Agent: `r2000-analyze-routine` skill now uses platform expertise.
- MCP Server: `get_binary_info` tool now returns platform information.
- MCP Server: Comments tool fix (removed automatic `;` prefix).
- Agent: Added `update-keyboard-shortcuts` skill for maintaining consistency.

## [0.7.1] - 2026-02-17

### Features

- Feature: **Bookmarks**: Added bookmark support.
  - `Ctrl+B`: Toggle bookmark at current address
  - `Shift+Ctrl+B` / `Alt+B`: Open Bookmark dialog to navigate between bookmarks
- Feature: **MCP Server – Address Details**: Added `r2000_get_address_details` tool to retrieve detailed information about a specific memory address.
- Feature: **MCP Server – Batch Execute**: Added batch execute support, allowing multiple MCP tool calls to be dispatched in a single request.
- Feature: **MCP Server – Cursor Navigation**: MCP clients can now drive the UI by jumping to an address (`r2000_set_disassembly_cursor`), with history preserved for undo/redo.

### Fixes

- Fix: **Menu**: Added missing "Pack Hi/Lo / Lo/Hi address" entries (`[` / `]`) to the Edit menu, enabled when an LDA/LDX/LDY immediate-mode opcode is selected.

### Documentation

- Docs: MCP documentation fixes and embedded YouTube video. Updated supported tools.

### Refactor / Internal

- Agent Skills: Added `verify-mcp`, `add-mcp-tool`, `update-mcp-docs` and `analyze-routine` agent skills.

## [0.7.0] - 2026-02-16

### Features

- Feature: **MCP Server Support**: Added comprehensive Model Context Protocol (MCP) server support for programmatic access.
  - HTTP and stdio transport modes
  - Tools for disassembly manipulation, memory search, block operations, and project management
  - Resources for accessing binary data, disassembly, and hexdump views
  - Support for undo/redo operations via MCP
  - Hexadecimal address support
- Feature: **CRT Support**: Enhanced CRT (Cartridge) file handling.
  - Bank picker dialog to choose which bank to analyze
  - CRT type display in dialog picker
- Feature: **T64 Support**: Added T64 file picker dialog.
- Feature: **D64 Enhancements**: Improved D64 file picker dialog.
  - Added entropy column
  - Display start and end addresses for files
- Feature: **Navigation**: Allow navigating on top of comments, relative addresses, and other "sub_index" addresses.

### Changes

- TAP support was temporarily added and then removed. Proper TAP support requires handling different loaders. Users should convert TAP files to .prg format first.

### Fixes

- Fix: **Pack Hi/Lo**: Corrected reversed hi/lo and lo/hi packing in single line.
- Fix: **Navigation**: Multiple disassembly navigation fixes.
  - Mouse click now sets cursor correctly
  - Landing on an address with subindex updates highlight correctly
  - Page Up/Down have consistent increment/decrement behavior
- Fix: **File Loading**: Only load supported file extensions, fail with error otherwise. Also prevents crash when cursor is >= new loaded file size.
- Fix: **UI**: Honor "right pane none" setting with minor fixes to address representation.

### Examples

- Examples: Enhanced example projects with more detailed comments from Claude.

### Documentation

- Docs: Added MCP Server documentation.

### Refactor / Internal

- Testing: Added unit tests for MCP server.

## [0.6.6] - 2026-02-11

### Security

- Feature: macOS binary is code-signed and notarized.

## [0.6.5] - 2026-02-10

### Features

- Feature: **Line Comments**: Line comments now also function as splitters, breaking code blocks.
- Feature: **Navigation**: Added Page Up/Down support in Open File dialog.
- Feature: **Charset View**: Page Up/Down now advances 10 lines instead of 10 characters for faster navigation.

### Changes

- UI: **Disassembly View**: Adjusted label and opcode spacing to provide more room for labels.
- UI: **Dialogs**: Improved sizing for single-row input dialogs.
- Shortcuts: **LoHi/HiLo**: Changed keyboard shortcuts for LoHi/HiLo word tables to `,` and `.` (was `t` and `T`).

### Fixes

- Fix: **State**: Clearing all state when opening a new file from an existing session.
- Fix: **Disassembly**: Correct cursor behavior when converting blocks involving addresses.

### Examples

- Examples: More detailed comments in example projects (joystick reading).

## [0.6.4] - 2026-02-08

### Features

- Feature: **D71 Support**: Added support for D71 disk images (70 tracks, double-sided).
- Feature: **D81 Support**: Added support for D81 disk images (80 tracks, 40 sectors per track).
- Feature: **40-Track D64**: Added support for 40-track D64 disk images.

### Changes

- UI: **Document Settings Dialog**: Reorganized settings dialog with dynamic options at the bottom.

### Fixes

- Fix: Settings: **Default Platform**: Default platform is "Commodore 64", was broken in previous version.

### Performance

- Performance: **Event Handling**: Further optimizations to event handling and rendering pipeline.

### Refactor / Internal

- Refactor: **Embedded Config Files**: System configuration files are now embedded in the binary for easier distribution.
- Refactor: **Disk Parser**: Unified D64/D71/D81 parsing logic with `DiskType` enum.

### Examples

- Examples: Renamed `burnin_rubber` example to `c64_burnin_rubber` for clarity.

## [0.6.3] - 2026-02-07

### Features

- Feature: **Complete Address Dialog**: Added dialog to complete missing Hi/Lo or Lo/Hi byte when only one line is selected (Edit menu).

### Performance

- Performance: **Rendering Loop**: Optimized screen rendering to occur only once per event, eliminating unnecessary redraws.

### Fixes

- Fix: **Settings Dialog**: Improved navigation in Document Settings dialog.

### Documentation

- Docs: Added source code links in architecture documentation.
- Docs: Added missing dialogs to architecture documentation.
- Docs: Improved keyboard shortcuts documentation formatting.

### Refactor / Internal

- Refactor: Consolidated system configuration files - merged separate `.txt` files (comments, excludes, labels) into unified `.json` files for each platform.
- Refactor: Updated asset loading to support consolidated `.json` configuration files.

## [0.6.2] - 2026-02-05

### Features

- Feature: **Entropy View**: Added entropy visualization column in HexDump view to help identify compressed/encrypted data.
- Feature: **Labels**: Added standard C64 labels for KERNAL, BASIC, and Zero Page (including NMI, RESET, IRQ vectors).
- Feature: **Config**: Automatic configuration backup if loading fails.
- Feature: **UI**: Circular navigation in D64 file picker.
- Feature: **Analysis**: If the file has high entropy, displays a warning that it might be compressed.

### Fixes

- Fix: **Labels**: Resolved issues with label duplication and priority.
- Fix: **External Labels**: Fixed display issues for external labels.
- Fix: **Stability**: Added terminal restoration handler in case of crash.
- Fix: **Navigation**: Improved "Jump to Address" (Enter key) behavior.

### Documentation

- Docs: Updated documentation for Blocks ("e" type), Settings, and Views.
- Docs: Added information about Immediate mode representations (lo/hi byte).

## [0.6.1] - 2026-02-01

### Features

- Feature: Support for .d64 disk images. Supports picking a .prg file from within the disk image.
- Feature: CLI: Added `--export_asm` and `--export_lbl` command line options.
- Feature: Bitmap view: Screen RAM address is now configurable.

### Changes

- Settings: "BRK single byte" is now enabled by default.
- UI: Hexdump default view mode is now "Screencode shifted".

### Documentation

- Docs: Added comprehensive "Mini Tutorial" walkthrough.
- Docs: Added graphics and updated tutorials section.
- Docs: Blocks documentation updated with tabs for each supported assembler.
- Docs: Updated keyboard shortcuts documentation.

### Fixes

- Fix: Splitter and collapsed blocks improvements.

### Refactor / Internal

- Refactor: `state.rs` split into multiple submodules (types, settings, project, app_state).
- Refactor: Large methods in `disassembler.rs` moved to separate modules.
- Refactor: Graphics code moved to its own file.
- Testing: Added comprehensive CPU module tests.

## [0.6.0] - 2026-01-30

### Features

- Feature: Support for LoHi and HiLo word block type. Keyboard shortcuts 'T' and 'Shift+T'.
- Feature: Export VICE labels to `.lbl` format (File -> Export -> Export VICE Labels)
- Feature: Import VICE labels from `.lbl` files (--import_lbl command line option)
- Feature: Go to Symbol dialog (Ctrl+P) - navigate to labels by name
- Feature: File dialogs remember last used paths
  - Export As remembers last used filename
  - Save/Export/Import remember last used folders
- Feature: Status bar shows filename for Save, Export, Import, and Open operations
- Feature: File dialogs include file extensions in the dialog
- Feature: Alt+F keyboard shortcut to open File menu
- Feature: Alt+H keyboard shortcut to open Help menu
- Feature: Keyboard shortcuts for Edit Menu and Search Menu

### Changes

- The keyboard shortcut for PETSCII Text changed from 'T' to 'P'.

### Fixes

- Fix: Splitter and collapsed blocks work correctly together
- Fix: Arrow visualization improvements - fixed passthrough arrows, eliminated "ghost" arrows
- Fix: Keyboard shortcuts requiring Shift key now work correctly on Windows
- Fix: Find References dialog only enabled when focus is on Disassembly view

### Documentation

- Docs: Initial MkDocs integration with ReadTheDocs hosting
- Docs: Architecture documentation updated with all current components
- Docs: Architecture diagram converted to Mermaid format
- Docs: Improved ca65 assembler documentation
- Docs: Added Settings dialog documentation
- Docs: Keyboard shortcuts now use pymdownx.keys extensions for better formatting
- Docs: Added FAQ and Views documentation
- Docs: Added logo and favicon for documentation site

### Settings

- Settings: Changed default for sync views (disabled by default)

### Examples

- Examples: Updated example projects with improved annotations

## [0.5.2] - 2026-01-26

### Fixes

- Fix: cargo fmt fixes

## [0.5.1] - 2026-01-26

### Features

- Feature: Mouse support
  - Click on menu expands submenu
  - Close dialogs with mouse
  - File -> Exit works with mouse
  - Navigation with scroll wheel / touchpad two-fingers
- Feature: External labels support
  - Rename labels from external addresses
  - Navigate in external label references
- Feature: Menu -> Edit -> Set Label added
- Feature: Better navigation in Keyboard Shortcut dialog (Page Up, Page Down, Ctrl+D, Ctrl+U)

### Fixes

- Fix: Jump to address works correctly when address has associated lines and subindex
- Fix: Search lands in correct subline
- Fix: Crash when setting block type on empty range
- Fix: Don't allow setting labels in external labels that start with comment
- Fix: Close About dialog faster with mouse

### Refactor

- Refactor: Renamed `comment_address` to `external_label_address`
- Refactor: Reduced duplicate code between disassembler.rs and view_disassembly.rs
- Refactor: Move logic whether opcode should have arrows to cpu.rs
- Refactor: Cross-reference function factored out
- UI: X-ref with more than supported displayed as "..."

## [0.5.0] - 2026-01-24

- Feature: Find Reference dialog (Ctrl+x)
- Feature: Bitmap Viewer with High-Res and Multicolor support.
- Feature: Settings: Auto-analyze on file open (enabled by default)
- Feature: Settings: Sync between Disassembly and Hexdump, Sprites, Charset, Bitmap and Blocks views (enabled by default)
- Feature: Partial bitmap and sprites rendering support
- Feature: Visual selection mode in HexDump, Sprites, and Charset views
- Feature: Search:
  - Hex byte pattern search with wildcards
  - Omni search now includes PETSCII and screencodes
- Feature: Press 'b' in side panels to convert to bytes
- Feature: pressing Enter in right-side pane, updates cursor in Disassembly view.
- UI: Updated Keyboard Shortcut dialog
- UI: Reordered options in Settings dialog
- UI: Don't show addresses if there are no bytes
- Performance: Image caching for faster rendering (Bitmap view)
- Navigation: Move up/down skip lines without "real bytes"
- Settings: Config: "Patch BRK" enabled by default
- Docs: Updated User Guide with illegal opcodes, patch brk, and command line options explanations
- Docs: README fixes and Discord channel correction
- Fix: Jump to collapsed line works correctly

## [0.4.1] - 2026-01-20

- Fix: Works in Windows
- Fix: Add alternative keyboard shortcuts for Windows
- Fix: cursor at end of line, even for default comment
- Refactor: dialog code simplified

## [0.4.0] - 2026-01-19

- Pane view: Added "Blocks" view, that shows the different blocks.
- Formatters: Added support for KickAssembler and ca65
- Refactor: created Widget concept where views and dialog inherits from it.
- Refactor: each view and each dialog has its own file
- Fix: Improve selection and cursor movement in disassembly view
- Fix: Improve input handling code: each view handles their own keyboard shortcuts.

## [0.3.0] - 2026-01-09

- Added user_guide.md
- Added block splitters
- Added support for collapsed blocks
- Added support for searching within comments
- Added 'm' keyboard shortcut to toggle shifted/unshifted charset modes
- Improved arrow visualization in disassembly (removed "ghost" arrows, better styling)
- Improved keyboard shortcuts reliability
- Fixed valid tests altering global configuration (`last_project_path`)
- Fixed search cursor positioning logic
- Fixed 64tass export regression
- Fixed ACME exporter for acum opcodes (ror, rol, lsr, asl)

## [0.2.2] - 2026-01-06

- Update keyboard shortcuts (again, sorry)
- Fix "Patch BRK": was not generating labels
- Fix indentation for relative addresses
- Fix dangling "addresses" generated by illegal opcodes
- Fix "Enter" key in Hex/Charset/Sprite views updating Disassembly cursor

## [0.2.1] - 2026-01-05

- Added support for Charset view: single color and multicolor
- Added support for Sprites view: single color and multicolor
- Added support for undocumented opcodes
- Improved keyboard shortcuts
- Added support for relative addresses in disassembly
- Added support for renaming relative address labels
- `Enter` key in Hex/Charset/Sprite views updates Disassembly cursor
- Improved BRK instruction disassembly
- Fixed Cross-reference comment placement

## [0.1.2] - 2026-01-04

Initial public version
