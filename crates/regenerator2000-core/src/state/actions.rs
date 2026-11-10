//! Semantic actions that any frontend (TUI, GUI, Web, MCP) can produce, along with document,
//! destructive-check, and modal dialog-closure contracts.

/// Semantic application action produced by the UI or programmatic clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    /// Exit the application.
    Exit,
    /// Open a binary or project file dialog.
    Open,
    /// Open the recent projects list dialog.
    OpenRecent,
    /// Save the active project to its current path.
    Save,
    /// Save the active project to a new path.
    SaveAs,
    /// Export the disassembly listing to the current `.asm` path.
    ExportAsm,
    /// Export the disassembly listing to a new `.asm` path.
    ExportAsmAs,
    /// Export the disassembly listing to the current `.html` path.
    ExportHtml,
    /// Export the disassembly listing to a new `.html` path.
    ExportHtmlAs,
    /// Undo the last applied command.
    Undo,
    /// Redo the last undone command.
    Redo,
    /// Mark the selected bytes as [`BlockType::Code`](super::types::BlockType::Code).
    Code,
    /// Run flow analysis from the current cursor address.
    DisassembleAddress,
    /// Create a lexical symbol scope over the selected range.
    Scope,
    /// Mark the selected bytes as [`BlockType::DataByte`](super::types::BlockType::DataByte).
    Byte,
    /// Mark the selected bytes as [`BlockType::DataWord`](super::types::BlockType::DataWord).
    Word,
    /// Mark the selected bytes as [`BlockType::Address`](super::types::BlockType::Address).
    Address,
    /// Mark the selected bytes as [`BlockType::PetsciiText`](super::types::BlockType::PetsciiText).
    PetsciiText,
    /// Mark the selected bytes as [`BlockType::ScreencodeText`](super::types::BlockType::ScreencodeText).
    ScreencodeText,
    /// Run full auto-analysis over the loaded binary.
    Analyze,
    /// Open the document settings dialog.
    DocumentSettings,
    /// Open the jump-to-address dialog.
    JumpToAddress,
    /// Open the jump-to-line dialog.
    JumpToLine,
    /// Jump to the operand target address of the current line.
    JumpToOperand,
    /// Jump to the next [`BlockType::Undefined`](super::types::BlockType::Undefined) block.
    JumpNextUnexplored,
    /// Jump to the previous [`BlockType::Undefined`](super::types::BlockType::Undefined) block.
    JumpPrevUnexplored,

    /// Pack the selected bytes as a paired low/high address table.
    PackLoHiAddress,
    /// Pack the selected bytes as a paired high/low address table.
    PackHiLoAddress,

    /// Mark the selected bytes as [`BlockType::LoHiAddress`](super::types::BlockType::LoHiAddress).
    SetLoHiAddress,
    /// Mark the selected bytes as [`BlockType::HiLoAddress`](super::types::BlockType::HiLoAddress).
    SetHiLoAddress,
    /// Mark the selected bytes as [`BlockType::LoHiWord`](super::types::BlockType::LoHiWord).
    SetLoHiWord,
    /// Mark the selected bytes as [`BlockType::HiLoWord`](super::types::BlockType::HiLoWord).
    SetHiLoWord,
    /// Mark the selected bytes as [`BlockType::ExternalFile`](super::types::BlockType::ExternalFile).
    SetExternalFile,
    /// Open the side comment editor for the current address.
    SideComment,
    /// Open the full-line comment editor for the current address.
    LineComment,
    /// Toggle the Hex Dump companion pane.
    ToggleHexDump,
    /// Toggle the Sprites companion pane.
    ToggleSpritesView,
    /// Open the About dialog.
    About,
    /// Open the bundled example files dialog.
    OpenExamples,
    /// Open the documentation viewer.
    OpenDocumentation,
    /// Open the change-origin dialog.
    ChangeOrigin,
    /// Open the keyboard shortcuts reference dialog.
    KeyboardShortcuts,
    /// Mark the selected bytes as [`BlockType::Undefined`](super::types::BlockType::Undefined).
    Undefined,
    /// Unpack the loaded binary with default unpacker settings.
    UnpackBinary,
    /// Unpack the loaded binary with a custom [`crate::unpacker::UnpackConfig`].
    UnpackBinaryWithConfig(crate::unpacker::UnpackConfig),
    /// Open the unpacker configuration dialog.
    UnpackDialog,
    /// Open the file information dialog.
    FileInfo,
    /// Open the global system settings dialog.
    SystemSettings,
    /// Start the background HTTP MCP server.
    StartMcpServer,
    /// Stop the background HTTP MCP server.
    StopMcpServer,
    /// Cycle the immediate operand format forward at the current address.
    NextImmediateFormat,
    /// Cycle the immediate operand format backward at the current address.
    PreviousImmediateFormat,
    /// Open the search dialog.
    Search,
    /// Jump to the next search match.
    FindNext,
    /// Jump to the previous search match.
    FindPrevious,
    /// Cycle the Hex Dump character encoding mode forward.
    HexdumpViewModeNext,
    /// Cycle the Hex Dump character encoding mode backward.
    HexdumpViewModePrev,
    /// Toggle multicolor rendering in the Sprites view.
    ToggleSpriteMulticolor,
    /// Toggle the Charset companion pane.
    ToggleCharsetView,
    /// Toggle multicolor rendering in the Charset view.
    ToggleCharsetMulticolor,
    /// Toggle the Bitmap companion pane.
    ToggleBitmapView,
    /// Toggle multicolor rendering in the Bitmap view.
    ToggleBitmapMulticolor,
    /// Toggle the Blocks companion pane.
    ToggleBlocksView,
    /// Collapse or expand the block at the current cursor position.
    ToggleCollapsedBlock,
    /// Insert or remove a manual block splitter at the current address.
    ToggleSplitter,
    /// Open the cross-references dialog for the current address.
    FindReferences,
    /// Navigate the disassembly view to the specified [`Addr`](super::Addr).
    NavigateToAddress(super::Addr),
    /// Mark the byte offset range `start..=end` as [`BlockType::DataByte`](super::types::BlockType::DataByte).
    SetBytesBlockByOffset {
        /// Inclusive starting byte offset.
        start: usize,
        /// Inclusive ending byte offset.
        end: usize,
    },
    /// Open the label editor for the current address.
    SetLabel,
    /// Open the go-to-symbol dialog.
    GoToSymbol,
    /// Open the import VICE labels dialog.
    ImportViceLabels,
    /// Open the export VICE labels dialog.
    ExportViceLabels,
    /// Toggle a bookmark at the current address.
    ToggleBookmark,
    /// Open the bookmarks list dialog.
    ListBookmarks,
    /// Open the VICE live debugger connection dialog.
    ViceConnect,
    /// Connect the VICE live debugger to the specified `host:port` address.
    ViceConnectAddress(String),
    /// Disconnect from the active VICE live debugger session.
    ViceDisconnect,
    /// Single-step one instruction in the connected VICE emulator.
    ViceStep,
    /// Resume execution in the connected VICE emulator.
    ViceContinue,
    /// Step over the subroutine call at the current PC in VICE.
    ViceStepOver,
    /// Step out of the current subroutine in VICE.
    ViceStepOut,
    /// Run execution in VICE until PC reaches the cursor address.
    ViceRunToCursor,
    /// Toggle an execution breakpoint at the cursor address in VICE.
    ViceToggleBreakpoint,
    /// Open the VICE breakpoint dialog.
    ViceBreakpointDialog,
    /// Set an execution breakpoint at `address` in VICE.
    ViceSetBreakpointAt {
        /// Target breakpoint [`Addr`](super::Addr).
        address: super::Addr,
    },
    /// Open the VICE watchpoint dialog for the cursor address.
    ViceToggleWatchpoint,
    /// Set a memory watchpoint of `kind` at `address` in VICE.
    ViceSetWatchpoint {
        /// Target watchpoint [`Addr`](super::Addr).
        address: super::Addr,
        /// Watchpoint access trigger mode.
        kind: crate::vice::state::BreakpointKind,
    },
    /// Open the VICE memory dump address dialog.
    ViceMemoryDumpDialog,
    /// Set the base `address` for the live VICE memory dump view.
    ViceSetMemoryDumpAddress {
        /// Base memory dump [`Addr`](super::Addr).
        address: super::Addr,
    },
    /// Toggle the live VICE debugger companion pane.
    ToggleDebuggerView,
    /// Navigate back to the previous cursor position in navigation history.
    NavigateBack,
    /// Apply a user label `name` at `address`.
    ApplyLabel {
        /// Target [`Addr`](super::Addr) for the label.
        address: super::Addr,
        /// New label name (an empty string removes the label).
        name: String,
        /// Whether the label is scoped locally (`LocalUserDefined`).
        is_local: bool,
    },
    /// Apply or clear an enum operand mapping at `address`.
    ApplyEnumUsage {
        /// Target instruction [`Addr`](super::Addr).
        address: super::Addr,
        /// Enum name to apply, or `None` to clear the enum mapping.
        enum_name: Option<String>,
    },
    /// Apply a side or line comment `text` of `kind` at `address`.
    ApplyComment {
        /// Target [`Addr`](super::Addr) for the comment.
        address: super::Addr,
        /// Comment text (an empty string removes the comment).
        text: String,
        /// Whether this is a side comment or a full-line comment.
        kind: super::types::CommentKind,
    },
    /// Change the binary load origin to the specified [`Addr`](super::Addr).
    ApplyOrigin(super::Addr),
    /// Cycle keyboard focus to the next visible UI pane.
    CyclePane,
    /// Cancel the active selection or modal dialog.
    Cancel,
    /// Expand or shrink the boundary of the lexical scope at the cursor.
    NudgeScopeBoundary {
        /// When `true`, expand the boundary outward. When `false`, shrink it inward.
        expand: bool,
    },
    /// Delete the lexical scope that encloses the current cursor address.
    RemoveScope,
    /// Add the external label target address to the per-project excluded set and re-analyze.
    ExcludeExternalAddress,
    /// Wrap a pre-approved action so core skips destructive confirmation checks.
    Confirmed(Box<AppAction>),
    /// Open the enum management dialog.
    ManageEnums,
    /// Open the apply-enum dialog for the current operand.
    ApplyEnumDialog,
    /// Create, update, rename, or delete a project-local enum definition.
    ApplyEnumDefinition {
        /// Target enum name.
        name: String,
        /// Updated [`EnumDefinition`](super::types::EnumDefinition), or `None` to delete the enum.
        definition: Option<super::types::EnumDefinition>,
        /// Previous enum name when renaming an existing enum.
        rename_from: Option<String>,
    },
    /// Create, update, rename, or delete a user-global enum definition.
    ApplyGlobalEnumDefinition {
        /// Target global enum name.
        name: String,
        /// Updated [`EnumDefinition`](super::types::EnumDefinition), or `None` to delete the enum.
        definition: Option<super::types::EnumDefinition>,
        /// Previous enum name when renaming an existing global enum.
        rename_from: Option<String>,
    },
    /// Open the symbolic offset expression editor (`o`) for the current operand, table entry,
    /// visual selection, or address.
    SymbolicOffsetDialog,
    /// Increment (`+` / `=`) or decrement (`-`) the symbolic offset displacement on the current
    /// operand, visual selection, or external equate.
    NudgeSymbolicOffset {
        /// Signed displacement delta (`+1` to increment, `-1` to decrement).
        delta: i16,
    },
    /// Apply or clear a Tier 1 (`operand_offset`) or Tier 2 (`address_offset`) symbolic offset expression.
    ///
    /// Modal frontends check submissions via
    /// [`validate_symbolic_offset_submission`](crate::action_handlers::disassembly_handler::validate_symbolic_offset_submission)
    /// before emitting this action because [`AppAction::closes_dialog`] returns `true` for it.
    ApplySymbolicOffset {
        /// Referencing instruction or table entry [`Addr`](super::Addr), when editing an operand site.
        site_address: Option<super::Addr>,
        /// Primary effective target [`Addr`](super::Addr).
        target_address: super::Addr,
        /// Optional inclusive `(start, end)` [`Addr`](super::Addr) range when applying across a selection.
        address_range: Option<(super::Addr, super::Addr)>,
        /// User-entered symbolic offset expression (empty or `"0"` clears the offset).
        input: String,
        /// When `true`, sets Tier 2 (`address_offset`) globally. When `false`, sets Tier 1 (`operand_offset`).
        is_global: bool,
        /// `true` when checking a 2-byte zero-page instruction operand (`base <= $00FF`).
        is_zero_page_mode: bool,
    },
}

impl AppAction {
    /// Returns `true` if this action requires a loaded binary document.
    #[must_use]
    pub fn requires_document(&self) -> bool {
        let action = match self {
            AppAction::Confirmed(a) => a.as_ref(),
            other => other,
        };
        !matches!(
            action,
            AppAction::Exit
                | AppAction::Open
                | AppAction::OpenRecent
                | AppAction::About
                | AppAction::OpenExamples
                | AppAction::OpenDocumentation
                | AppAction::KeyboardShortcuts
                | AppAction::SystemSettings
                | AppAction::StartMcpServer
                | AppAction::StopMcpServer
                | AppAction::Search
                | AppAction::ToggleDebuggerView
                | AppAction::ViceContinue
                | AppAction::ViceStepOver
                | AppAction::ViceStepOut
                | AppAction::ViceRunToCursor
                | AppAction::ViceToggleBreakpoint
                | AppAction::ViceBreakpointDialog
                | AppAction::ViceSetBreakpointAt { .. }
                | AppAction::ViceToggleWatchpoint
                | AppAction::ViceMemoryDumpDialog
        )
    }

    /// Returns `true` if this action must check for unsaved changes before execution.
    #[must_use]
    pub fn is_destructive(&self) -> bool {
        // Confirmed actions already bypassed the check.
        if matches!(self, AppAction::Confirmed(_)) {
            return false;
        }

        matches!(
            self,
            AppAction::Exit
                | AppAction::Open
                | AppAction::OpenRecent
                | AppAction::UnpackBinary
                | AppAction::UnpackBinaryWithConfig(_)
        )
    }

    /// Returns `true` if this action closes its modal dialog and advances `UIState::dialog_queue`.
    #[must_use]
    pub fn closes_dialog(&self) -> bool {
        let action = match self {
            AppAction::Confirmed(a) => a.as_ref(),
            other => other,
        };

        matches!(
            action,
            AppAction::ViceConnectAddress(_)
                | AppAction::ViceSetWatchpoint { .. }
                | AppAction::ViceSetBreakpointAt { .. }
                | AppAction::ViceSetMemoryDumpAddress { .. }
                | AppAction::NavigateToAddress(_)
                // `LabelDialog` and `SymbolicOffsetDialog` pre-validate user input on `Enter`
                // before emitting `ApplyLabel` / `ApplySymbolicOffset` so invalid expressions
                // keep the modal open rather than closing and prematurely draining `dialog_queue`.
                | AppAction::ApplyLabel { .. }
                | AppAction::ApplySymbolicOffset { .. }
                | AppAction::ApplyComment { .. }
                | AppAction::ApplyEnumUsage { .. }
                | AppAction::Search
                // Pressing `Enter` in `SearchDialog` dispatches `AppAction::FindNext` to execute
                // unified search in `Core`, which must also dismiss `SearchDialog` and promote
                // any queued dialog in `dialog_queue`.
                | AppAction::FindNext
                | AppAction::FindPrevious
                | AppAction::ImportViceLabels
                | AppAction::ExportViceLabels
                | AppAction::ExportAsm
                | AppAction::ExportAsmAs
                | AppAction::ExportHtml
                | AppAction::ExportHtmlAs
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::ChangeOrigin
                | AppAction::ApplyOrigin(_)
                | AppAction::Open
                | AppAction::OpenRecent
                | AppAction::UnpackBinary
                | AppAction::UnpackBinaryWithConfig(_)
        )
    }
}
