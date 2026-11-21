//! Top-level application menu bar state, rendering, and action dispatch.

mod menu_action;
mod menu_model;
mod menu_render;
mod menu_widget;

pub use menu_action::handle_menu_action;
pub use menu_model::{MenuState, SearchHistory};
pub use menu_render::render_menu_popup;
pub use menu_widget::Menu;
