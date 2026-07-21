//! File domain action handler for file I/O, projects, exports, and binary unpacking.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::event::CoreEvent;
use crate::state::Addr;
use crate::state::actions::AppAction;

/// Handler for file management actions (Save, Open, Export, Unpack, FileInfo).
#[derive(Debug, Default)]
pub struct FileActionHandler;

impl FileActionHandler {
    /// Creates a new [`FileActionHandler`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Helper to transactionally clear pre-existing undo/redo stack on new document load.
    pub fn handle_file_open(&self, ctx: &mut ActionContext<'_>, origin: Addr, data: Vec<u8>) {
        ctx.state.undo_stack = crate::commands::UndoStack::new();
        let _ = ctx.state.load_binary(origin, data);
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
    }
}

/// Helper function to retrieve the default filename stem for dialogs.
fn get_default_filename_stem(ctx: &ActionContext<'_>) -> Option<String> {
    ctx.state
        .file_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().to_string())
}

impl DomainActionHandler for FileActionHandler {
    fn handle_action(
        &self,
        action: &AppAction,
        ctx: &mut ActionContext<'_>,
    ) -> Result<bool, CoreError> {
        match action {
            AppAction::Open => {
                ctx.events
                    .push(CoreEvent::DialogRequested(crate::event::DialogType::Open));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Select a file to open".to_string(),
                ));
                Ok(true)
            }
            AppAction::OpenRecent => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::OpenRecent,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Open recent project".to_string()));
                Ok(true)
            }
            AppAction::Save => {
                if let Some(path) = ctx.state.project_path.clone() {
                    let context = crate::navigation::create_save_context(ctx.state, ctx.view);
                    match ctx.state.save_project(context, true) {
                        Ok(_) => {
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            ctx.events
                                .push(CoreEvent::StatusMessage(format!("Saved: {filename}")));
                        }
                        Err(e) => {
                            ctx.events
                                .push(CoreEvent::StatusMessage(format!("Error saving: {e}")));
                        }
                    }
                } else {
                    let initial = ctx
                        .state
                        .last_save_as_filename
                        .clone()
                        .or_else(|| get_default_filename_stem(ctx));
                    ctx.events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::SaveAs {
                            initial_filename: initial,
                        },
                    ));
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Enter Project filename".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::SaveAs => {
                let initial = ctx
                    .state
                    .last_save_as_filename
                    .clone()
                    .or_else(|| get_default_filename_stem(ctx));
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::SaveAs {
                        initial_filename: initial,
                    },
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Enter Project filename".to_string(),
                ));
                Ok(true)
            }
            AppAction::ExportAsm => {
                if let Some(path) = ctx.state.export_asm_path.clone() {
                    match crate::exporter::export_asm(ctx.state, &path) {
                        Ok(_) => {
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            ctx.events
                                .push(CoreEvent::StatusMessage(format!("Exported: {filename}")));
                        }
                        Err(e) => {
                            ctx.events
                                .push(CoreEvent::StatusMessage(format!("Error exporting: {e}")));
                        }
                    }
                } else {
                    let initial = ctx
                        .state
                        .last_export_asm_filename
                        .clone()
                        .or_else(|| get_default_filename_stem(ctx));
                    ctx.events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::ExportAs {
                            initial_filename: initial,
                            format: crate::event::ExportFormat::Asm,
                        },
                    ));
                    ctx.events
                        .push(CoreEvent::StatusMessage("Enter .asm filename".to_string()));
                }
                Ok(true)
            }
            AppAction::ExportAsmAs => {
                let initial = ctx
                    .state
                    .last_export_asm_filename
                    .clone()
                    .or_else(|| get_default_filename_stem(ctx));
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ExportAs {
                        initial_filename: initial,
                        format: crate::event::ExportFormat::Asm,
                    },
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Enter .asm filename".to_string()));
                Ok(true)
            }
            AppAction::ExportHtml => {
                if let Some(mut path) = ctx.state.export_html_path.clone() {
                    path.set_extension("html");
                    match crate::exporter::export_html(ctx.state, &path) {
                        Ok(_) => {
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            ctx.events
                                .push(CoreEvent::StatusMessage(format!("Exported: {filename}")));
                        }
                        Err(e) => {
                            ctx.events
                                .push(CoreEvent::StatusMessage(format!("Error exporting: {e}")));
                        }
                    }
                } else {
                    let initial = ctx
                        .state
                        .last_export_html_filename
                        .clone()
                        .or_else(|| get_default_filename_stem(ctx));
                    ctx.events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::ExportAs {
                            initial_filename: initial,
                            format: crate::event::ExportFormat::Html,
                        },
                    ));
                    ctx.events
                        .push(CoreEvent::StatusMessage("Enter .html filename".to_string()));
                }
                Ok(true)
            }
            AppAction::ExportHtmlAs => {
                let initial = ctx
                    .state
                    .last_export_html_filename
                    .clone()
                    .or_else(|| get_default_filename_stem(ctx));
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ExportAs {
                        initial_filename: initial,
                        format: crate::event::ExportFormat::Html,
                    },
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Enter .html filename".to_string()));
                Ok(true)
            }
            AppAction::ImportViceLabels => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ImportViceLabels,
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Select a VICE label file to import".to_string(),
                ));
                Ok(true)
            }
            AppAction::ExportViceLabels => {
                let initial = ctx
                    .state
                    .last_export_labels_filename
                    .clone()
                    .or_else(|| get_default_filename_stem(ctx));
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ExportLabels {
                        initial_filename: initial,
                    },
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Enter VICE label filename".to_string(),
                ));
                Ok(true)
            }
            AppAction::UnpackBinary => {
                let load_addr = ctx.state.origin.0;
                let raw_data = ctx.state.raw_data.clone();
                ctx.events.push(CoreEvent::UnpackStarted {
                    raw_data,
                    load_addr,
                    config: crate::unpacker::UnpackConfig::default(),
                });
                Ok(true)
            }
            AppAction::UnpackBinaryWithConfig(config) => {
                let load_addr = ctx.state.origin.0;
                let raw_data = ctx.state.raw_data.clone();
                ctx.events.push(CoreEvent::UnpackStarted {
                    raw_data,
                    load_addr,
                    config: config.clone(),
                });
                Ok(true)
            }
            AppAction::UnpackDialog => {
                ctx.events
                    .push(CoreEvent::DialogRequested(crate::event::DialogType::Unpack));
                Ok(true)
            }
            AppAction::FileInfo => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::FileInfo,
                ));
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::view_state::CoreViewState;

    #[test]
    fn test_handle_file_open_clears_undo_stack_and_loads_binary() {
        let mut state = AppState::new();
        let mut view = CoreViewState::new();
        let mut events = Vec::new();

        // Push dummy command to undo stack first
        state
            .undo_stack
            .push(crate::commands::Command::ToggleSplitter {
                address: Addr(0x1000),
            });
        assert!(!state.undo_stack.is_empty());

        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = FileActionHandler::new();
        let origin = Addr(0x2000);
        let data = vec![0xEA, 0x60]; // NOP, RTS
        handler.handle_file_open(&mut ctx, origin, data);

        assert!(ctx.state.undo_stack.is_empty());
        assert_eq!(ctx.state.origin, origin);
        assert_eq!(ctx.state.raw_data, vec![0xEA, 0x60]);
        assert!(ctx.events.contains(&CoreEvent::StateChanged));
        assert!(ctx.events.contains(&CoreEvent::ViewChanged));
    }
}
