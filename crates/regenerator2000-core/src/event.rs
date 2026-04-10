//! Frontend-agnostic events and dialog requests.

use crate::state::Addr;
use crate::state::actions::AppAction;
pub use crate::state::types::CommentKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// AppState has changed meaningfully (e.g. after a command or analysis).
    StateChanged,
    /// CoreViewState has changed (e.g. cursor moved, active pane changed).
    ViewChanged,
    /// A message to be displayed to the user.
    StatusMessage(String),
    /// The core requests to show a dialog.
    DialogRequested(DialogType),
    /// The core requests to close any active dialog.
    DialogDismissalRequested,
    /// The core requests to quit the application.
    QuitRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Asm,
    Lst,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogType {
    About,
    Bookmarks,
    BreakpointAddress(Option<u16>),
    Comment {
        address: Addr,
        current: Option<String>,
        kind: CommentKind,
    },
    Confirmation {
        title: String,
        message: String,
        action: AppAction,
    },
    DocumentSettings,
    ExportAs {
        initial_filename: Option<String>,
        format: ExportFormat,
    },
    ExportLabels {
        initial_filename: Option<String>,
    },
    FindReferences(Addr),
    GoToSymbol,
    ImportViceLabels,
    JumpToAddress,
    JumpToLine,
    KeyboardShortcuts,
    Label {
        address: Addr,
        initial_name: String,
        is_external: bool,
    },
    Open,
    OpenRecent,
    SaveAs {
        initial_filename: Option<String>,
    },
    Search {
        query: String,
        filters: crate::state::search::SearchFilters,
    },
    Settings,
    Origin,
    ViceConnect,
    WatchpointAddress(Option<u16>),
    MemoryDumpAddress(Option<u16>),
    CompleteAddress {
        known_byte: u8,
        lo_first: bool,
        address: Addr,
    },
}
