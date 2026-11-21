//! Top-level TUI rendering composition and widget module registry.
//!
//! Splits the terminal frame into the menu bar, minimap bar, main workspace, and status bar.
//! Overlays active menu popups and modal [`widget::Widget`] dialogs on top.

pub(crate) mod dialog_about;
pub(crate) mod dialog_apply_enum;
pub(crate) mod dialog_bookmarks;
pub(crate) mod dialog_comment;
pub(crate) mod dialog_complete_address;
pub(crate) mod dialog_confirmation;
pub mod dialog_container_picker;
pub mod dialog_crt_picker;
pub(crate) mod dialog_document_settings;
pub(crate) mod dialog_edit_enum;
pub(crate) mod dialog_file_browser;
pub(crate) mod dialog_file_info;
pub(crate) mod dialog_find_references;
pub(crate) mod dialog_go_to_symbol;
pub(crate) mod dialog_import_context;
pub(crate) mod dialog_jump_to_operand;
pub(crate) mod dialog_keyboard_shortcut;
pub(crate) mod dialog_label;
pub(crate) mod dialog_manage_enums;
pub(crate) mod dialog_open_recent;
pub(crate) mod dialog_prompt;
pub(crate) mod dialog_search;
pub(crate) mod dialog_settings;
pub(crate) mod dialog_symbolic_offset;
pub(crate) mod dialog_unpack;
pub(crate) mod dialog_vice_connect;
pub(crate) mod dialog_watchpoint_address;
pub(crate) mod graphics_common;
pub(crate) mod menu;
pub(crate) mod minimap_bar;
pub(crate) mod navigable;
mod render;
pub(crate) mod scoped_input;
pub(crate) mod statusbar;
pub(crate) mod view_bitmap;
pub(crate) mod view_blocks;
pub(crate) mod view_charset;
pub(crate) mod view_debugger;
pub(crate) mod view_disassembly;
pub(crate) mod view_hexdump;
pub(crate) mod view_sprites;
pub mod widget;

pub use render::ui;
