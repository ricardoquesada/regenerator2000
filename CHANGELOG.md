# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
