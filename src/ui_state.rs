pub use crate::menu::{MenuAction, MenuState};
use crate::theme::Theme;
use image::DynamicImage;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Disassembly,
    HexDump,
    Sprites,

    Charset,
    Blocks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPane {
    None,
    #[default]
    HexDump,
    Sprites,

    Charset,
    Blocks,
}

use crate::state::PetsciiMode;

pub struct UIState {
    pub open_dialog: crate::dialog_open::OpenDialog,
    pub jump_to_address_dialog: crate::dialog_jump_to_address::JumpToAddressDialog,
    pub jump_to_line_dialog: crate::dialog_jump_to_line::JumpToLineDialog,
    pub save_as_dialog: crate::dialog_save_as::SaveAsDialog,
    pub export_as_dialog: crate::dialog_export_as::ExportAsDialog,
    pub label_dialog: crate::dialog_label::LabelDialogState,
    pub comment_dialog: crate::dialog_comment::CommentDialogState,
    pub settings_dialog: crate::dialog_document_settings::DocumentSettingsDialog,
    pub about_dialog: crate::dialog_about::AboutDialog,
    pub shortcuts_dialog: crate::dialog_keyboard_shortcut::ShortcutsDialog,
    pub origin_dialog: crate::dialog_origin::OriginDialogState,
    pub confirmation_dialog: crate::dialog_confirmation::ConfirmationDialogState,
    pub system_settings_dialog: crate::dialog_settings::SettingsDialog,
    pub search_dialog: crate::dialog_search::SearchDialog,
    pub menu: MenuState,

    pub navigation_history: Vec<(ActivePane, usize)>,
    #[allow(dead_code)]
    pub disassembly_state: ListState,

    // UI Selection/Cursor
    pub selection_start: Option<usize>,
    pub cursor_index: usize,
    pub sub_cursor_index: usize,
    #[allow(dead_code)]
    pub scroll_index: usize,

    // Hex View State
    pub hex_cursor_index: usize,
    pub sprites_cursor_index: usize,
    pub charset_cursor_index: usize,

    pub blocks_list_state: ListState,
    #[allow(dead_code)]
    pub hex_scroll_index: usize,
    pub right_pane: RightPane,
    pub sprite_multicolor_mode: bool,
    pub charset_multicolor_mode: bool,
    pub petscii_mode: PetsciiMode,

    pub active_pane: ActivePane,
    pub should_quit: bool,
    pub status_bar: crate::statusbar::StatusBarState,

    pub logo: Option<DynamicImage>,
    pub picker: Option<Picker>,
    pub dismiss_logo: bool,
    pub is_visual_mode: bool,
    pub input_buffer: String,

    pub theme: Theme,

    // Vim-like search
    pub vim_search_active: bool,
    pub vim_search_input: String,
}

impl UIState {
    pub fn new(theme: Theme) -> Self {
        Self {
            open_dialog: crate::dialog_open::OpenDialog::new(),
            jump_to_address_dialog: crate::dialog_jump_to_address::JumpToAddressDialog::new(),
            jump_to_line_dialog: crate::dialog_jump_to_line::JumpToLineDialog::new(),
            save_as_dialog: crate::dialog_save_as::SaveAsDialog::new(),
            export_as_dialog: crate::dialog_export_as::ExportAsDialog::new(),
            label_dialog: crate::dialog_label::LabelDialogState::new(),
            comment_dialog: crate::dialog_comment::CommentDialogState::new(),
            settings_dialog: crate::dialog_document_settings::DocumentSettingsDialog::new(),
            about_dialog: crate::dialog_about::AboutDialog::new(),
            shortcuts_dialog: crate::dialog_keyboard_shortcut::ShortcutsDialog::new(),
            origin_dialog: crate::dialog_origin::OriginDialogState::new(),
            confirmation_dialog: crate::dialog_confirmation::ConfirmationDialogState::new(),
            system_settings_dialog: crate::dialog_settings::SettingsDialog::new(),
            search_dialog: crate::dialog_search::SearchDialog::new(),
            menu: MenuState::new(),
            navigation_history: Vec::new(),
            disassembly_state: ListState::default(),
            selection_start: None,
            cursor_index: 0,
            sub_cursor_index: 0,
            scroll_index: 0,
            hex_cursor_index: 0,
            sprites_cursor_index: 0,

            charset_cursor_index: 0,
            blocks_list_state: ListState::default(),
            hex_scroll_index: 0,

            right_pane: RightPane::HexDump,
            sprite_multicolor_mode: false,
            charset_multicolor_mode: false,
            petscii_mode: PetsciiMode::Unshifted,
            active_pane: ActivePane::Disassembly,
            should_quit: false,
            status_bar: crate::statusbar::StatusBarState::new(),
            logo: crate::utils::load_logo(),
            picker: crate::utils::create_picker(),
            dismiss_logo: false,
            is_visual_mode: false,
            input_buffer: String::new(),
            vim_search_active: false,
            vim_search_input: String::new(),
            theme,
        }
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_bar.set_message(message);
    }

    pub fn restore_session(
        &mut self,
        loaded_data: &crate::state::LoadedProjectData,
        app_state: &crate::state::AppState,
    ) {
        let loaded_cursor = loaded_data.cursor_address;
        let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
        let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
        let loaded_right_pane = &loaded_data.right_pane_visible;
        let loaded_charset_cursor = loaded_data.charset_cursor_address;

        self.sprite_multicolor_mode = loaded_data.sprite_multicolor_mode;
        self.charset_multicolor_mode = loaded_data.charset_multicolor_mode;
        self.petscii_mode = loaded_data.petscii_mode;
        let initial_addr = loaded_cursor.unwrap_or(app_state.origin);
        if let Some(idx) = app_state.get_line_index_for_address(initial_addr) {
            self.cursor_index = idx;
        }

        // Also restore hex cursor if present
        if let Some(hex_addr) = loaded_hex_cursor
            && !app_state.raw_data.is_empty()
        {
            let origin = app_state.origin as usize;
            let alignment_padding = origin % 16;
            let aligned_origin = origin - alignment_padding;
            let target = hex_addr as usize;

            if target >= aligned_origin {
                let offset = target - aligned_origin;
                let row = offset / 16;
                // Ensure row is within bounds
                let total_len = app_state.raw_data.len() + alignment_padding;
                let max_rows = total_len.div_ceil(16);
                if row < max_rows {
                    self.hex_cursor_index = row;
                }
            }
        }

        // Restore Right Pane and Sprites Cursor
        if let Some(pane_str) = loaded_right_pane {
            match pane_str.as_str() {
                "HexDump" => self.right_pane = RightPane::HexDump,
                "Sprites" => self.right_pane = RightPane::Sprites,
                "Charset" => self.right_pane = RightPane::Charset,
                "Blocks" => self.right_pane = RightPane::Blocks,
                _ => {}
            }
        }
        if let Some(idx) = loaded_data.blocks_view_cursor {
            self.blocks_list_state.select(Some(idx));
        }
        if let Some(sprites_addr) = loaded_sprites_cursor {
            let origin = app_state.origin as usize;
            let padding = (64 - (origin % 64)) % 64;
            let addr = sprites_addr as usize;
            if addr >= origin + padding {
                let offset = addr - (origin + padding);
                self.sprites_cursor_index = offset / 64;
            }
        }
        if let Some(charset_addr) = loaded_charset_cursor {
            let origin = app_state.origin as usize;
            let base_alignment = 0x400;
            let aligned_start_addr = (origin / base_alignment) * base_alignment;
            let addr = charset_addr as usize;
            if addr >= aligned_start_addr {
                let offset = addr - aligned_start_addr;
                self.charset_cursor_index = offset / 8;
            }
        }
    }
}
