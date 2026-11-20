//! File domain action handler for file I/O, projects, exports, and binary unpacking.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::event::{CoreEvent, DialogType, ExportFormat};
use crate::exporter;
use crate::navigation;
use crate::state::GlobalConfigUpdate;
use crate::state::actions::AppAction;
use crate::unpacker::UnpackConfig;

/// Handler for file management actions (`Save`, `Open`, `Export`, `Unpack`, and `FileInfo`).
#[derive(Debug, Default)]
pub struct FileActionHandler;

impl FileActionHandler {
    /// Creates a new [`FileActionHandler`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Returns the filename stem of the loaded binary file, if available.
fn get_default_filename_stem(ctx: &ActionContext<'_>) -> Option<String> {
    ctx.state
        .file_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().to_string())
}

/// Emits a [`DialogType::SaveAs`](DialogType::SaveAs) request with the default project filename.
fn request_save_as_dialog(ctx: &mut ActionContext<'_>) {
    let initial = ctx
        .state
        .last_save_as_filename
        .clone()
        .or_else(|| get_default_filename_stem(ctx));
    ctx.events
        .push(CoreEvent::DialogRequested(DialogType::SaveAs {
            initial_filename: initial,
        }));
    ctx.events.push(CoreEvent::StatusMessage(
        "Enter Project filename".to_string(),
    ));
}

/// Emits a [`DialogType::ExportAs`](DialogType::ExportAs) request for `format`.
fn request_export_dialog(ctx: &mut ActionContext<'_>, format: ExportFormat) {
    let (last_filename, status) = match format {
        ExportFormat::Asm => (&ctx.state.last_export_asm_filename, "Enter .asm filename"),
        ExportFormat::Html => (&ctx.state.last_export_html_filename, "Enter .html filename"),
    };
    let initial = last_filename
        .clone()
        .or_else(|| get_default_filename_stem(ctx));
    ctx.events
        .push(CoreEvent::DialogRequested(DialogType::ExportAs {
            initial_filename: initial,
            format,
        }));
    ctx.events
        .push(CoreEvent::StatusMessage(status.to_string()));
}

/// Emits a [`CoreEvent::UnpackStarted`] event with `config`.
fn start_unpack(ctx: &mut ActionContext<'_>, config: UnpackConfig) {
    let load_addr = ctx.state.origin.0;
    let raw_data = ctx.state.raw_data.clone();
    ctx.events.push(CoreEvent::UnpackStarted {
        raw_data,
        load_addr,
        config,
    });
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
                    .push(CoreEvent::DialogRequested(DialogType::Open));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Select a file to open".to_string(),
                ));
                Ok(true)
            }
            AppAction::OpenRecent => {
                ctx.events
                    .push(CoreEvent::DialogRequested(DialogType::OpenRecent));
                ctx.events
                    .push(CoreEvent::StatusMessage("Open recent project".to_string()));
                Ok(true)
            }
            AppAction::Save => {
                if let Some(path) = ctx.state.project_path.clone() {
                    let context = navigation::create_save_context(ctx.state, ctx.view);
                    match ctx.state.save_project(context, GlobalConfigUpdate::Update) {
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
                    request_save_as_dialog(ctx);
                }
                Ok(true)
            }
            AppAction::SaveAs => {
                request_save_as_dialog(ctx);
                Ok(true)
            }
            AppAction::ExportAsm => {
                if let Some(path) = ctx.state.export_asm_path.clone() {
                    match exporter::export_asm(ctx.state, &path) {
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
                    request_export_dialog(ctx, ExportFormat::Asm);
                }
                Ok(true)
            }
            AppAction::ExportAsmAs => {
                request_export_dialog(ctx, ExportFormat::Asm);
                Ok(true)
            }
            AppAction::ExportHtml => {
                if let Some(mut path) = ctx.state.export_html_path.clone() {
                    path.set_extension("html");
                    match exporter::export_html(ctx.state, &path) {
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
                    request_export_dialog(ctx, ExportFormat::Html);
                }
                Ok(true)
            }
            AppAction::ExportHtmlAs => {
                request_export_dialog(ctx, ExportFormat::Html);
                Ok(true)
            }
            AppAction::ImportViceLabels => {
                ctx.events
                    .push(CoreEvent::DialogRequested(DialogType::ImportViceLabels));
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
                ctx.events
                    .push(CoreEvent::DialogRequested(DialogType::ExportLabels {
                        initial_filename: initial,
                    }));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Enter VICE label filename".to_string(),
                ));
                Ok(true)
            }
            AppAction::UnpackBinary => {
                start_unpack(ctx, UnpackConfig::default());
                Ok(true)
            }
            AppAction::UnpackBinaryWithConfig(config) => {
                start_unpack(ctx, config.clone());
                Ok(true)
            }
            AppAction::UnpackDialog => {
                ctx.events
                    .push(CoreEvent::DialogRequested(DialogType::Unpack));
                Ok(true)
            }
            AppAction::FileInfo => {
                ctx.events
                    .push(CoreEvent::DialogRequested(DialogType::FileInfo));
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, AppState};
    use crate::view_state::CoreViewState;
    use std::path::PathBuf;

    #[test]
    fn test_save_and_export_fallback_to_dialog_with_filename_stem() {
        let mut state = AppState::new();
        state.file_path = Some(PathBuf::from("/tmp/demo_intro.prg"));
        let mut view = CoreViewState::new();
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = FileActionHandler::new();
        handler
            .handle_action(&AppAction::Save, &mut ctx)
            .expect("Save action should succeed");
        assert!(
            ctx.events
                .contains(&CoreEvent::DialogRequested(DialogType::SaveAs {
                    initial_filename: Some("demo_intro".to_string()),
                }))
        );

        ctx.events.clear();
        handler
            .handle_action(&AppAction::ExportAsm, &mut ctx)
            .expect("ExportAsm action should succeed");
        assert!(
            ctx.events
                .contains(&CoreEvent::DialogRequested(DialogType::ExportAs {
                    initial_filename: Some("demo_intro".to_string()),
                    format: ExportFormat::Asm,
                }))
        );

        ctx.events.clear();
        handler
            .handle_action(&AppAction::ExportHtml, &mut ctx)
            .expect("ExportHtml action should succeed");
        assert!(
            ctx.events
                .contains(&CoreEvent::DialogRequested(DialogType::ExportAs {
                    initial_filename: Some("demo_intro".to_string()),
                    format: ExportFormat::Html,
                }))
        );
    }

    #[test]
    fn test_unpack_binary_emits_unpack_started() {
        let mut state = AppState::new();
        state
            .load_binary(Addr(0x0801), vec![0xEA, 0x60])
            .expect("test binary should load");
        let mut view = CoreViewState::new();
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = FileActionHandler::new();
        handler
            .handle_action(&AppAction::UnpackBinary, &mut ctx)
            .expect("UnpackBinary action should succeed");
        assert!(ctx.events.iter().any(|e| matches!(
            e,
            CoreEvent::UnpackStarted {
                load_addr: 0x0801,
                ..
            }
        )));
    }
}
