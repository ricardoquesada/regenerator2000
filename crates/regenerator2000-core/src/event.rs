//! Frontend-agnostic events and dialog requests.

use crate::state::Addr;
use crate::state::actions::AppAction;
pub use crate::state::types::CommentKind;

/// Events emitted by [`Core::apply_action`](crate::Core::apply_action) for the active frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// Indicates that [`AppState`](crate::state::AppState) changed after a command or analysis pass.
    StateChanged,
    /// Indicates that [`CoreViewState`](crate::view_state::CoreViewState) changed (such as cursor or pane updates).
    ViewChanged,
    /// Status bar message to display to the user.
    StatusMessage(String),
    /// Requests the frontend to open a modal dialog.
    DialogRequested(DialogType),
    /// Requests the frontend to close the active dialog.
    DialogDismissalRequested,
    /// Requests the frontend to exit the application.
    QuitRequested,
    /// Requests the frontend to open a URL in the system browser.
    OpenUrl(String),
    /// Requests the frontend to run the binary unpacker on a background thread.
    UnpackStarted {
        /// Raw binary bytes to unpack.
        raw_data: Vec<u8>,
        /// Load address of the binary.
        load_addr: u16,
        /// Unpacker execution configuration.
        config: crate::unpacker::UnpackConfig,
    },
    /// Requests the frontend to start the MCP server.
    StartMcpServerRequested,
    /// Requests the frontend to stop the MCP server.
    StopMcpServerRequested,
}

/// Output format for full-program export dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Assembly source code (`.asm`).
    Asm,
    /// Self-contained HTML document (`.html`).
    Html,
}

/// Modal dialog variants that the core engine can request from a frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogType {
    /// Application about dialog.
    About,
    /// Project bookmarks list dialog.
    Bookmarks,
    /// Debugger breakpoint address input dialog with an optional initial address.
    BreakpointAddress(Option<u16>),
    /// Side or line comment editor dialog.
    Comment {
        /// Target memory address.
        address: Addr,
        /// Existing comment text at `address`, if any.
        current: Option<String>,
        /// Comment placement kind (side or line).
        kind: CommentKind,
    },
    /// Confirmation prompt before running a destructive action.
    Confirmation {
        /// Dialog title text.
        title: String,
        /// Prompt message text.
        message: String,
        /// Action to dispatch if the user confirms.
        action: AppAction,
    },
    /// Per-project document settings dialog.
    DocumentSettings,
    /// Binary metadata and packer information dialog.
    FileInfo,
    /// Export destination dialog for assembly or HTML output.
    ExportAs {
        /// Suggested filename stem or path, if available.
        initial_filename: Option<String>,
        /// Selected export file format.
        format: ExportFormat,
    },
    /// VICE label export destination dialog.
    ExportLabels {
        /// Suggested filename stem or path, if available.
        initial_filename: Option<String>,
    },
    /// Cross-reference list dialog for the given target address.
    FindReferences(Addr),
    /// Symbol search and jump dialog.
    GoToSymbol,
    /// VICE label file import dialog.
    ImportViceLabels,
    /// Hexadecimal address jump dialog.
    JumpToAddress,
    /// Disassembly line-number jump dialog.
    JumpToLine,
    /// Keyboard shortcut reference dialog.
    KeyboardShortcuts,
    /// User label editor dialog.
    Label {
        /// Target memory address.
        address: Addr,
        /// Pre-filled label name.
        initial_name: String,
        /// `true` when the existing label at `address` is a local (`LocalUserDefined`) label.
        is_local: bool,
        /// `true` when editing an external equate line outside the binary range.
        is_external: bool,
    },
    /// File browser dialog for opening binaries or projects.
    Open,
    /// Recent projects selector dialog.
    OpenRecent,
    /// Project save path dialog.
    SaveAs {
        /// Suggested project filename, if available.
        initial_filename: Option<String>,
    },
    /// Text and pattern search dialog.
    Search {
        /// Initial search query string.
        query: String,
        /// Active search domain and encoding filters.
        filters: crate::state::search::SearchFilters,
    },
    /// Global application settings dialog.
    Settings,
    /// Disassembly origin address dialog.
    Origin,
    /// VICE binary monitor connection dialog.
    ViceConnect,
    /// Debugger watchpoint address dialog with an optional initial address.
    WatchpointAddress(Option<u16>),
    /// Debugger memory dump address dialog with an optional initial address.
    MemoryDumpAddress(Option<u16>),
    /// Low/high immediate byte pairing dialog.
    CompleteAddress {
        /// Known low or high byte from the selected instruction.
        known_byte: u8,
        /// `true` if `known_byte` is the low byte, `false` if it is the high byte.
        lo_first: bool,
        /// Address of the immediate instruction being paired.
        address: Addr,
    },
    /// Initial import context dialog for newly loaded or unpacked binaries.
    ImportContext {
        /// Binary load origin address.
        origin: Addr,
        /// Suggested execution entry point address.
        entry_point: Addr,
    },
    /// Immediate operand enum selector dialog.
    ApplyEnum {
        /// Instruction address to annotate.
        address: Addr,
        /// Operand value to match against enum variants.
        value: u16,
    },
    /// Enum definition manager dialog.
    ManageEnums,
    /// Symbolic offset expression editor dialog (`base +/- offset`), opened via `o` in the
    /// Disassembly View.
    SymbolicOffset {
        /// Referencing instruction or table entry address when editing a site operand, or `None`
        /// for external equates, mid-instruction byte rows, and global struct/data addresses.
        site_address: Option<Addr>,
        /// Primary effective target address being annotated (for example, `$00FC` on `STA $FC` or
        /// `$C0FF` on `.byte <$C0FF`).
        target_address: Addr,
        /// Optional inclusive `(start, end)` address range when invoked across a multi-entry
        /// visual selection (such as an `RTS` dispatch table or contiguous struct fields).
        address_range: Option<(Addr, Addr)>,
        /// Pre-populated symbolic offset expression (for example, `"zp_ptr + 1"`, `"sub_C100 - 1"`,
        /// `"-1"`, or a struct base name).
        initial_input: String,
        /// Initial state of the global Tier 2 (`address_offset`) scope checkbox.
        is_global: bool,
        /// When `true`, forces and locks Tier 2 global scope (`is_global == true`) because no
        /// referencing operand site applies (such as external equates or raw struct/data bytes).
        lock_global: bool,
        /// `true` when validating a 2-byte zero-page instruction operand (`ZeroPage*`, `IndirectX`,
        /// `IndirectY`), enforcing `base <= $00FF` for single-pass cross-assembler compatibility.
        is_zero_page_mode: bool,
    },
    /// Binary unpacker configuration dialog.
    Unpack,
}
