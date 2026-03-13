//! Semantic actions that any frontend (TUI, GUI, Web, MCP) can produce.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Exit,
    Open,
    OpenRecent,
    Save,
    SaveAs,
    ExportProject,
    ExportProjectAs,
    Undo,
    Redo,
    Code,
    Byte,
    Word,
    Address,
    PetsciiText,
    ScreencodeText,
    Analyze,
    DocumentSettings,
    JumpToAddress,
    JumpToLine,
    JumpToOperand,

    PackLoHiAddress,
    PackHiLoAddress,

    SetLoHiAddress,
    SetHiLoAddress,
    SetLoHiWord,
    SetHiLoWord,
    SetExternalFile,
    SideComment,
    LineComment,
    ToggleHexDump,
    ToggleSpritesView,
    About,
    ChangeOrigin,
    KeyboardShortcuts,
    Undefined,
    SystemSettings,
    NextImmediateFormat,
    PreviousImmediateFormat,
    Search,
    FindNext,
    FindPrevious,
    HexdumpViewModeNext,
    HexdumpViewModePrev,
    ToggleSpriteMulticolor,
    ToggleCharsetView,
    ToggleCharsetMulticolor,
    ToggleBitmapView,
    ToggleBitmapMulticolor,
    ToggleBlocksView,
    ToggleCollapsedBlock,
    ToggleSplitter,
    FindReferences,
    NavigateToAddress(super::Addr),
    SetBytesBlockByOffset {
        start: usize,
        end: usize,
    },
    SetLabel,
    GoToSymbol,
    ImportViceLabels,
    ExportViceLabels,
    ToggleBookmark,
    ListBookmarks,
    ViceConnect,
    ViceConnectAddress(String),
    ViceDisconnect,
    ViceStep,
    ViceContinue,
    ViceStepOver,
    ViceStepOut,
    ViceRunToCursor,
    ViceToggleBreakpoint,
    ViceBreakpointDialog,
    ViceSetBreakpointAt {
        address: super::Addr,
    },
    ViceToggleWatchpoint,
    ViceSetWatchpoint {
        address: super::Addr,
        kind: crate::vice::state::BreakpointKind,
    },
    ToggleDebuggerView,
    NavigateBack,
    ApplyLabel {
        address: super::Addr,
        name: String,
    },
    ApplyComment {
        address: super::Addr,
        text: String,
        kind: super::types::CommentKind,
    },
}

impl AppAction {
    #[must_use]
    pub fn requires_document(&self) -> bool {
        !matches!(
            self,
            AppAction::Exit
                | AppAction::Open
                | AppAction::OpenRecent
                | AppAction::About
                | AppAction::KeyboardShortcuts
                | AppAction::SystemSettings
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
        )
    }

    /// Whether this action should close the dialog that produced it.
    ///
    /// Actions like VICE connect, set-breakpoint, and set-watchpoint resolve
    /// the dialog — the user is done interacting with it once they confirm.
    #[must_use]
    pub fn closes_dialog(&self) -> bool {
        matches!(
            self,
            AppAction::ViceConnectAddress(_)
                | AppAction::ViceSetWatchpoint { .. }
                | AppAction::ViceSetBreakpointAt { .. }
        )
    }
}
