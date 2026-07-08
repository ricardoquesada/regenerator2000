//! Semantic actions that any frontend (TUI, GUI, Web, MCP) can produce.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Exit,
    Open,
    OpenRecent,
    Save,
    SaveAs,
    ExportAsm,
    ExportAsmAs,
    ExportHtml,
    ExportHtmlAs,
    Undo,
    Redo,
    Code,
    DisassembleAddress,
    Scope,
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
    JumpNextUnexplored,
    JumpPrevUnexplored,

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
    OpenExamples,
    OpenDocumentation,
    ChangeOrigin,
    KeyboardShortcuts,
    Undefined,
    UnpackBinary,
    UnpackBinaryWithConfig(crate::unpacker::UnpackConfig),
    UnpackDialog,
    FileInfo,
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
    ViceMemoryDumpDialog,
    ViceSetMemoryDumpAddress {
        address: super::Addr,
    },
    ToggleDebuggerView,
    NavigateBack,
    ApplyLabel {
        address: super::Addr,
        name: String,
        is_local: bool,
    },
    ApplyEnumUsage {
        address: super::Addr,
        enum_name: Option<String>,
    },
    ApplyComment {
        address: super::Addr,
        text: String,
        kind: super::types::CommentKind,
    },
    ApplyOrigin(super::Addr),
    CyclePane,
    Cancel,
    NudgeScopeBoundary {
        expand: bool,
    },
    RemoveScope,
    /// Adds the external label's target address to the per-project excluded
    /// addresses set and re-analyzes, so it no longer appears as a symbolic
    /// external reference.
    ExcludeExternalAddress,
    /// Wraps an action that has been explicitly confirmed by the user.
    /// Core will bypass destructive checks for this action.
    Confirmed(Box<AppAction>),
    ManageEnums,
    ApplyEnumDefinition {
        name: String,
        definition: Option<super::types::EnumDefinition>,
        rename_from: Option<String>,
    },
    ApplyGlobalEnumDefinition {
        name: String,
        definition: Option<super::types::EnumDefinition>,
        rename_from: Option<String>,
    },
}

impl AppAction {
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

    /// Whether this action should check for unsaved changes before proceeding.
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

    /// Whether this action should close the dialog that produced it.
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
                | AppAction::ApplyLabel { .. }
                | AppAction::ApplyComment { .. }
                | AppAction::ApplyEnumUsage { .. }
                | AppAction::Search
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
                | AppAction::ApplyEnumDefinition { .. }
                | AppAction::ApplyGlobalEnumDefinition { .. }
        )
    }
}
