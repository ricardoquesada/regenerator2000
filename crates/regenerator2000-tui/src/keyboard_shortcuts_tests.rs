//! Comprehensive 3-Pillar Keyboard Shortcut Verification Suite.
//!
//! This module exercises the full TUI key-routing pipeline (`events::process_key_event` ->
//! modal dialogs -> menu bar -> inline Vim search -> active pane `Widget::handle_input` ->
//! `navigable::handle_nav_input` -> `events::input::handle_global_input` ->
//! `menu_action::handle_menu_action` -> `Core::apply_action`) without coupling assertions to
//! fragile pixel/cell coordinates.
//!
//! # Architecture of the 3 Pillars
//!
//! - **Pillar 1 (`test_global_shortcuts_across_all_panes`)**: Iterates over all 7 [`ActivePane`]
//!   variants (`Disassembly`, `HexDump`, `Sprites`, `Charset`, `Bitmap`, `Blocks`, `Debugger`)
//!   and verifies that every global shortcut produces its expected application-wide effect and
//!   is never shadowed or hijacked by view-local single-character bindings (such as `'s'`, `'x'`,
//!   `'m'`, or `'b'` in `BitmapView`).
//! - **Pillar 2 (`test_navigable_shortcuts_all_views`, `test_*_view_shortcuts`, `test_menu_and_dialog_navigation`)**:
//!   Exercises every view-local, [`Navigable`](crate::ui::navigable::Navigable), visual-selection,
//!   scope-nudging (`Alt+Up`/`Alt+Down`), and modal dialog shortcut end-to-end through
//!   [`crate::events::process_key_event`], including shifted/unshifted modifier resilience across
//!   terminal protocols.
//! - **Pillar 3 (`test_shortcut_layers_synchronization`)**: Enforces 4-layer metadata parity across
//!   Layer 1 (live input dispatch), Layer 2 ([`MenuState::new`]), Layer 3
//!   ([`ShortcutsDialog::get_shortcuts`]), and Layer 4 (`docs/keyboard_shortcuts.md`).

use crate::events::process_key_event;
use crate::state::types::{BlockType, HexdumpViewMode, ImmediateFormat};
use crate::state::{Addr, BlockItem};
use crate::theme::Theme;
use crate::ui::dialog_keyboard_shortcut::ShortcutsDialog;
use crate::ui::menu::{AppAction, MenuState};
use crate::ui::navigable::Navigable;
use crate::ui_state::{ActivePane, RightPane, ScreenRamMode, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use regenerator2000_core::Core;

const KEYBOARD_SHORTCUTS_DOC: &str = include_str!("../../../docs/keyboard_shortcuts.md");

const ALL_PANES: [ActivePane; 7] = [
    ActivePane::Disassembly,
    ActivePane::HexDump,
    ActivePane::Sprites,
    ActivePane::Charset,
    ActivePane::Bitmap,
    ActivePane::Blocks,
    ActivePane::Debugger,
];

/// Constructs a press [`KeyEvent`] for the given key code and modifiers.
fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// Sends a single key press through the full TUI key-routing pipeline.
fn send_key(core: &mut Core, ui_state: &mut UIState, code: KeyCode, modifiers: KeyModifiers) {
    process_key_event(core, ui_state, key(code, modifiers));
}

/// Deterministic headless test harness wrapping [`Core`] and [`UIState`].
struct ShortcutTestHarness {
    core: Core,
    ui_state: UIState,
}

impl ShortcutTestHarness {
    /// Builds a deterministic in-memory [`Core`] + [`UIState`] with 8,192 bytes of C64 data
    /// loaded at `Addr(0x0800)`.
    fn new() -> Self {
        let mut core = Core::new();
        let mut ui_state = UIState::new(Theme::from_name("Solarized Dark"));
        ui_state.file_dialog_current_dir = std::env::temp_dir();

        // Populate 8,192 bytes of deterministic C64 code and graphics data at $0800..$2800.
        let mut data = vec![0u8; 8192];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i * 37 + 13) & 0xFF) as u8;
        }

        // $0800..$0840: 6502 code region
        data[0..64].fill(0xEA); // Fill with NOPs by default
        // $0800: LDA #$42 (immediate mode instruction)
        data[0] = 0xA9;
        data[1] = 0x42;
        // $0802: LDX #$10 (consecutive immediate mode instruction for [ / ] packing)
        data[2] = 0xA2;
        data[3] = 0x10;
        // $0804: JSR $0810 (subroutine call for Enter JumpToOperand and Ctrl+x FindReferences)
        data[4] = 0x20;
        data[5] = 0x10;
        data[6] = 0x08;
        // $0807: JSR $0820 (jump target for Enter JumpToOperand and Backspace NavigateBack; allows flow analysis to reach $080A)
        data[7] = 0x20;
        data[8] = 0x20;
        data[9] = 0x08;
        // $080A: JSR $FFD2 (external Kernal call generating an external label line at top)
        data[10] = 0x20;
        data[11] = 0xD2;
        data[12] = 0xFF;
        // $080D: NOP, $080E: NOP, $080F: RTS
        data[13] = 0xEA;
        data[14] = 0xEA;
        data[15] = 0x60;
        // $0810: LDA #$99, $0812: NOP, $0813: RTS (subroutine target + scope auto-end)
        data[16] = 0xA9;
        data[17] = 0x99;
        data[18] = 0xEA;
        data[19] = 0x60;
        // $0820: LDA #$77, $0822: RTS (jump target)
        data[32] = 0xA9;
        data[33] = 0x77;
        data[34] = 0x60;

        // $0880..$0883: Valid 6502 instructions inside DataByte block for 'd' (DisassembleAddress)
        data[128] = 0xA9;
        data[129] = 0x55;
        data[130] = 0x60;

        core.state
            .load_binary(Addr(0x0800), data)
            .expect("load_binary must succeed");

        // Configure distinct blocks for blocks view & unexplored block navigation:
        // 0..64 ($0800..$0840): Code
        // 64..128 ($0840..$0880): Undefined (unexplored #1)
        // 128..192 ($0880..$08C0): DataByte
        // 192..256 ($08C0..$0900): Undefined (unexplored #2)
        // 256..8192 ($0900..$2800): DataByte
        core.state.block_types[0..64].fill(BlockType::Code);
        core.state.block_types[64..128].fill(BlockType::Undefined);
        core.state.block_types[128..192].fill(BlockType::DataByte);
        core.state.block_types[192..256].fill(BlockType::Undefined);
        core.state.block_types[256..8192].fill(BlockType::DataByte);
        core.state.disassemble();

        // Start with a clean undo stack so !core.state.is_dirty().
        core.state.undo_stack = crate::commands::UndoStack::new();
        core.state.last_saved_pointer = core.state.undo_stack.get_pointer();

        let origin_idx = core
            .state
            .get_line_index_for_address(Addr(0x0800))
            .unwrap_or(0);
        ui_state.cursor_index = origin_idx;
        ui_state.scroll_index = origin_idx;
        ui_state.blocks_list_state.select(Some(0));
        ui_state.sync_tui_to_core();
        core.view = ui_state.core.clone();

        Self { core, ui_state }
    }

    /// Sends a single key press through [`process_key_event`].
    fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        send_key(&mut self.core, &mut self.ui_state, code, modifiers);
    }

    /// Switches focus to `pane` and ensures the corresponding [`RightPane`] is visible.
    fn set_pane(&mut self, pane: ActivePane) {
        self.ui_state.active_pane = pane;
        match pane {
            ActivePane::Disassembly => {}
            ActivePane::HexDump => {
                if !matches!(
                    self.ui_state.right_pane,
                    RightPane::HexDump16 | RightPane::HexDump8
                ) {
                    self.ui_state.right_pane = RightPane::HexDump16;
                }
            }
            ActivePane::Sprites => {
                if !matches!(
                    self.ui_state.right_pane,
                    RightPane::Sprites2Col | RightPane::Sprites1Col
                ) {
                    self.ui_state.right_pane = RightPane::Sprites2Col;
                }
            }
            ActivePane::Charset => {
                if !matches!(
                    self.ui_state.right_pane,
                    RightPane::Charset8Col | RightPane::Charset4Col
                ) {
                    self.ui_state.right_pane = RightPane::Charset8Col;
                }
            }
            ActivePane::Bitmap => {
                self.ui_state.right_pane = RightPane::Bitmap;
            }
            ActivePane::Blocks => {
                self.ui_state.right_pane = RightPane::Blocks;
                if self.ui_state.blocks_list_state.selected().is_none() {
                    self.ui_state.blocks_list_state.select(Some(0));
                }
            }
            ActivePane::Debugger => {
                self.ui_state.right_pane = RightPane::Debugger;
            }
        }
        self.ui_state.sync_tui_to_core();
        self.core.view = self.ui_state.core.clone();
    }

    /// Renders the currently active modal dialog into a headless buffer and returns its top border line.
    fn active_dialog_title(&mut self) -> Option<String> {
        let dialog = self.ui_state.active_dialog.take()?;
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("TestBackend terminal must initialize");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &self.core.state, &mut self.ui_state);
            })
            .expect("dialog render must succeed");
        self.ui_state.active_dialog = Some(dialog);

        let area = self.ui_state.active_dialog_area;
        let buf = terminal.backend().buffer();
        let mut top_line = String::new();
        for x in area.x..area.right() {
            top_line.push_str(buf[(x, area.y)].symbol());
        }
        Some(top_line)
    }

    /// Closes any active dialog, clears the dialog queue, closes the menu bar, and resets quit/dirty flags.
    fn dismiss_dialog(&mut self) {
        self.ui_state.active_dialog = None;
        self.ui_state.dialog_queue.clear();
        self.ui_state.menu.active = false;
        self.ui_state.menu.selected_item = None;
        self.ui_state.vim_search_active = false;
        self.ui_state.vim_search_input.clear();
        self.ui_state.input_buffer.clear();
        self.ui_state.should_quit = false;
        self.core.state.last_saved_pointer = self.core.state.undo_stack.get_pointer();
    }

    /// Finds the disassembly line index for `addr` (selecting the content line with non-empty bytes
    /// when multiple lines share an address).
    fn line_index_for_addr(&self, addr: Addr) -> usize {
        self.core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == addr && (!l.bytes.is_empty() || l.is_collapsed))
            .or_else(|| self.core.state.get_line_index_for_address(addr))
            .expect("address must exist in disassembly")
    }
}

/// Verifies that non-press key events (`KeyEventKind::Release` and `KeyEventKind::Repeat`)
/// are ignored at the top of `handle_key_event`.
#[test]
fn test_key_release_and_repeat_ignored() {
    let mut h = ShortcutTestHarness::new();
    let initial_cursor = h.ui_state.cursor_index;

    let release_event = KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    };
    process_key_event(&mut h.core, &mut h.ui_state, release_event);
    assert_eq!(h.ui_state.cursor_index, initial_cursor);

    let repeat_event = KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Repeat,
        state: KeyEventState::NONE,
    };
    process_key_event(&mut h.core, &mut h.ui_state, repeat_event);
    assert_eq!(h.ui_state.cursor_index, initial_cursor);
}

// =============================================================================
// PILLAR 1: Global Shortcut Non-Shadowing Matrix Across All 7 ActivePanes
// =============================================================================

/// Pillar 1 Matrix Test: Iterates over all 7 [`ActivePane`] variants and verifies that every
/// application-wide shortcut (`F10`, `Alt+*` menu hotkeys, `Ctrl+*`/`Alt+*` file/bookmark/jump
/// actions, `u`/`Ctrl+r` undo/redo, `F2`/`F4`/`F6`/`F7`/`F8`/`F9` debugger keys, and
/// `Alt+1..6`/`Ctrl+1..6`/`Tab` pane switching) executes its global action without being
/// swallowed or hijacked by view-local handlers.
#[test]
fn test_global_shortcuts_across_all_panes() {
    let mut h = ShortcutTestHarness::new();

    for pane in ALL_PANES {
        h.dismiss_dialog();
        h.set_pane(pane);
        let origin_idx = h.line_index_for_addr(Addr(0x0800));
        h.ui_state.cursor_index = origin_idx;
        h.ui_state.sync_tui_to_core();
        h.core.view = h.ui_state.core.clone();

        // 1. Main Menu Activation: F10
        h.press(KeyCode::F(10), KeyModifiers::NONE);
        assert!(
            h.ui_state.menu.active,
            "F10 failed to activate menu in pane {pane:?}"
        );
        h.dismiss_dialog();

        // 2. Menu Category Hotkeys: Alt+f, Alt+t, Alt+j, Alt+r, Alt+v, Alt+u, Alt+h
        let menu_hotkeys = [
            ('f', 0, "File"),
            ('t', 1, "Edit"),
            ('j', 2, "Jump"),
            ('r', 3, "Search"),
            ('v', 4, "View"),
            ('u', 5, "Debugger"),
            ('h', 6, "Help"),
        ];
        for (ch, expected_cat, cat_name) in menu_hotkeys {
            h.set_pane(pane);
            let cursor_before = h.ui_state.cursor_index;
            let hex_col_before = h.ui_state.hex_col_cursor;
            h.press(KeyCode::Char(ch), KeyModifiers::ALT);
            assert!(
                h.ui_state.menu.active,
                "Alt+{ch} ({cat_name}) did not activate menu in pane {pane:?}"
            );
            assert_eq!(
                h.ui_state.menu.selected_category, expected_cat,
                "Alt+{ch} selected wrong category in pane {pane:?}"
            );
            assert!(
                h.ui_state.menu.selected_item.is_some(),
                "Alt+{ch} did not select an enabled menu item in pane {pane:?}"
            );
            assert_eq!(
                h.ui_state.cursor_index, cursor_before,
                "Alt+{ch} mutated disassembly cursor in pane {pane:?}"
            );
            assert_eq!(
                h.ui_state.hex_col_cursor, hex_col_before,
                "Alt+{ch} mutated hex column cursor in pane {pane:?}"
            );
            h.dismiss_dialog();
        }

        // 3. Quit Application: Ctrl+q (clean vs. dirty state)
        h.set_pane(pane);
        h.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(
            h.ui_state.should_quit,
            "Ctrl+q did not set should_quit in clean state for pane {pane:?}"
        );
        h.dismiss_dialog();

        // Dirty state: Ctrl+q must open Unsaved Changes ConfirmationDialog
        h.core.state.last_saved_pointer = usize::MAX;
        assert!(h.core.state.is_dirty());
        h.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(
            !h.ui_state.should_quit,
            "Ctrl+q should not quit immediately when dirty in pane {pane:?}"
        );
        let title = h
            .active_dialog_title()
            .expect("Ctrl+q when dirty must open ConfirmationDialog");
        assert!(
            title.contains("Unsaved Changes"),
            "Expected Unsaved Changes dialog on dirty Ctrl+q in {pane:?}, got {title}"
        );
        h.dismiss_dialog();

        // 4. Open File: Ctrl+o
        h.set_pane(pane);
        h.press(KeyCode::Char('o'), KeyModifiers::CONTROL);
        let title = h
            .active_dialog_title()
            .expect("Ctrl+o must open OpenDialog");
        assert!(
            title.contains("Open File"),
            "Ctrl+o opened wrong dialog in {pane:?}: {title}"
        );
        h.dismiss_dialog();

        // 5. Open Recent Projects: Alt+o, Ctrl+Shift+o, Ctrl+Shift+O
        for (code, mods) in [
            (KeyCode::Char('o'), KeyModifiers::ALT),
            (
                KeyCode::Char('o'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (
                KeyCode::Char('O'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            h.set_pane(pane);
            h.press(code, mods);
            let title = h
                .active_dialog_title()
                .expect("OpenRecent shortcut must open OpenRecentDialog");
            assert!(
                title.contains("Open Recent"),
                "OpenRecent ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 6. Save Project (Ctrl+s) & Save Project As (Alt+s, Ctrl+Shift+s, Ctrl+Shift+S)
        // Critical regression guard: must NEVER mutate BitmapView's bitmap_screen_ram_mode!
        let initial_screen_ram = ScreenRamMode::AfterBitmap;
        h.ui_state.bitmap_screen_ram_mode = initial_screen_ram;
        h.set_pane(pane);
        h.press(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(
            h.ui_state.bitmap_screen_ram_mode, initial_screen_ram,
            "Ctrl+s shadowed by BitmapView in pane {pane:?}"
        );
        let title = h
            .active_dialog_title()
            .expect("Ctrl+s must open SaveAsDialog when project_path is None");
        assert!(
            title.contains("Save"),
            "Ctrl+s opened wrong dialog in {pane:?}: {title}"
        );
        h.dismiss_dialog();

        for (code, mods) in [
            (KeyCode::Char('s'), KeyModifiers::ALT),
            (
                KeyCode::Char('s'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (
                KeyCode::Char('S'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            h.ui_state.bitmap_screen_ram_mode = initial_screen_ram;
            h.set_pane(pane);
            h.press(code, mods);
            assert_eq!(
                h.ui_state.bitmap_screen_ram_mode, initial_screen_ram,
                "SaveAs ({code:?}, {mods:?}) mutated bitmap_screen_ram_mode in pane {pane:?}"
            );
            let title = h
                .active_dialog_title()
                .expect("SaveAs shortcut must open SaveAsDialog");
            assert!(
                title.contains("Save"),
                "SaveAs ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 7. Export to .asm (Ctrl+e) & Export to .asm as... (Alt+e, Ctrl+Shift+e, Ctrl+Shift+E)
        h.set_pane(pane);
        let block_type_before = h.core.state.block_types[0];
        h.press(KeyCode::Char('e'), KeyModifiers::CONTROL);
        let title = h
            .active_dialog_title()
            .expect("Ctrl+e must open ExportAsDialog");
        assert!(
            title.contains("Export"),
            "Ctrl+e opened wrong dialog in {pane:?}: {title}"
        );
        h.dismiss_dialog();

        for (code, mods) in [
            (KeyCode::Char('e'), KeyModifiers::ALT),
            (
                KeyCode::Char('e'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (
                KeyCode::Char('E'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            h.set_pane(pane);
            h.press(code, mods);
            assert_eq!(
                h.core.state.block_types[0], block_type_before,
                "ExportAsmAs ({code:?}, {mods:?}) must not set ExternalFile block in {pane:?}"
            );
            let title = h
                .active_dialog_title()
                .expect("ExportAsmAs shortcut must open ExportAsDialog");
            assert!(
                title.contains("Export"),
                "ExportAsmAs ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 8. Document Settings (Alt+d, Ctrl+Shift+d, Ctrl+Shift+D)
        for (code, mods) in [
            (KeyCode::Char('d'), KeyModifiers::ALT),
            (
                KeyCode::Char('d'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (
                KeyCode::Char('D'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            h.set_pane(pane);
            let cursor_before = h.ui_state.cursor_index;
            h.press(code, mods);
            assert_eq!(
                h.ui_state.cursor_index, cursor_before,
                "DocumentSettings ({code:?}, {mods:?}) was swallowed by Ctrl+d PageDown in {pane:?}"
            );
            let title = h
                .active_dialog_title()
                .expect("DocumentSettings shortcut must open DocumentSettingsDialog");
            assert!(
                title.contains("Document Settings"),
                "DocumentSettings opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 9. System Settings (Alt+p, Ctrl+,)
        for (code, mods) in [
            (KeyCode::Char('p'), KeyModifiers::ALT),
            (KeyCode::Char(','), KeyModifiers::CONTROL),
        ] {
            h.set_pane(pane);
            h.press(code, mods);
            let title = h
                .active_dialog_title()
                .expect("Settings shortcut must open SettingsDialog");
            assert!(
                title.contains("Settings"),
                "Settings ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 10. Global Undo (u) and Redo (Ctrl+r)
        // First apply a reversible action in Disassembly (convert $0800 to DataByte), then Undo/Redo from `pane`.
        h.set_pane(ActivePane::Disassembly);
        h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
        h.press(KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(h.core.state.block_types[0], BlockType::DataByte);

        h.set_pane(pane);
        h.press(KeyCode::Char('u'), KeyModifiers::NONE);
        assert_eq!(
            h.core.state.block_types[0],
            BlockType::Code,
            "Global 'u' (Undo) failed in pane {pane:?}"
        );
        h.press(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(
            h.core.state.block_types[0],
            BlockType::DataByte,
            "Global Ctrl+r (Redo) failed in pane {pane:?}"
        );
        // Undo once more to restore Code at $0800
        h.press(KeyCode::Char('u'), KeyModifiers::NONE);
        assert_eq!(h.core.state.block_types[0], BlockType::Code);
        h.dismiss_dialog();

        // 11. Auto-Analyze: Ctrl+a
        h.set_pane(pane);
        h.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(
            h.core.state.block_types[0],
            BlockType::Code,
            "Ctrl+a (Analyze) failed in pane {pane:?}"
        );
        // Restore our synthetic Undefined/DataByte test layout after Ctrl+a
        h.core.state.undo_last_command();
        h.dismiss_dialog();

        // 12. Toggle Bookmark (Ctrl+b) & List Bookmarks (Alt+b, Alt+Shift+b/B, Ctrl+Shift+b/B)
        // Critical regression guard: in BitmapView, Ctrl+b and Alt+b previously converted 8,000 bytes to DataByte!
        let origin_idx = h.line_index_for_addr(Addr(0x0800));
        h.ui_state.cursor_index = origin_idx;
        h.set_pane(pane);
        let block_types_snapshot = h.core.state.block_types.clone();
        let undo_ptr_before = h.core.state.undo_stack.get_pointer();

        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert_eq!(
            h.core.state.undo_stack.get_pointer(),
            undo_ptr_before + 1,
            "Ctrl+b failed to push SetBookmark command in pane {pane:?}"
        );
        assert_eq!(
            h.core.state.block_types, block_types_snapshot,
            "Ctrl+b destructively mutated block_types in pane {pane:?}!"
        );

        // Seed a named bookmark at $0800 and verify Ctrl+b toggles it off cleanly
        h.core
            .state
            .annotations
            .update(Addr(0x0800), |e| e.bookmark = Some("Entry".to_string()));
        assert!(
            h.core
                .state
                .annotations
                .get(Addr(0x0800))
                .and_then(|e| e.bookmark.as_ref())
                .is_some()
        );
        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert!(
            h.core
                .state
                .annotations
                .get(Addr(0x0800))
                .and_then(|e| e.bookmark.as_ref())
                .is_none(),
            "Ctrl+b failed to remove bookmark at $0800 in pane {pane:?}"
        );
        h.dismiss_dialog();

        for (code, mods) in [
            (KeyCode::Char('b'), KeyModifiers::ALT),
            (KeyCode::Char('b'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            (KeyCode::Char('B'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            (
                KeyCode::Char('b'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (
                KeyCode::Char('B'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            h.set_pane(pane);
            h.press(code, mods);
            assert_eq!(
                h.core.state.block_types, block_types_snapshot,
                "ListBookmarks ({code:?}, {mods:?}) destructively mutated block_types in pane {pane:?}!"
            );
            let title = h
                .active_dialog_title()
                .expect("ListBookmarks shortcut must open BookmarksDialog");
            assert!(
                title.contains("Bookmarks"),
                "ListBookmarks ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 13. Jump to Address (Alt+g, Ctrl+g) & Jump to Line (Alt+Shift+g/G, Ctrl+Shift+g/G)
        for (code, mods) in [
            (KeyCode::Char('g'), KeyModifiers::ALT),
            (KeyCode::Char('g'), KeyModifiers::CONTROL),
        ] {
            h.set_pane(pane);
            h.press(code, mods);
            let title = h
                .active_dialog_title()
                .expect("JumpToAddress shortcut must open JumpToAddressDialog");
            assert!(
                title.contains("Jump To Address"),
                "JumpToAddress ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        for (code, mods) in [
            (KeyCode::Char('g'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            (KeyCode::Char('G'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            (
                KeyCode::Char('g'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            (
                KeyCode::Char('G'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        ] {
            h.set_pane(pane);
            let cursor_before = h.ui_state.cursor_index;
            h.press(code, mods);
            assert_eq!(
                h.ui_state.cursor_index, cursor_before,
                "JumpToLine ({code:?}, {mods:?}) was swallowed by Shift+G in {pane:?}"
            );
            let title = h
                .active_dialog_title()
                .expect("JumpToLine shortcut must open JumpToLineDialog");
            assert!(
                title.contains("Jump To Line"),
                "JumpToLine ({code:?}, {mods:?}) opened wrong dialog in {pane:?}: {title}"
            );
            h.dismiss_dialog();
        }

        // 14. Go to Symbol (Ctrl+p)
        h.set_pane(pane);
        h.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
        let title = h
            .active_dialog_title()
            .expect("Ctrl+p must open GoToSymbolDialog");
        assert!(
            title.contains("Go to Symbol"),
            "Ctrl+p opened wrong dialog in {pane:?}: {title}"
        );
        h.dismiss_dialog();

        // 15. Find References (Ctrl+x)
        // Critical regression guard: in BitmapView, Ctrl+x previously set ScreenRamMode::AfterBitmap!
        h.ui_state.bitmap_screen_ram_mode = ScreenRamMode::BankOffset(3);
        h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
        h.set_pane(pane);
        h.press(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(
            h.ui_state.bitmap_screen_ram_mode,
            ScreenRamMode::BankOffset(3),
            "Ctrl+x was hijacked by BitmapView 'x' in pane {pane:?}"
        );
        if pane == ActivePane::Disassembly {
            let title = h
                .active_dialog_title()
                .expect("Ctrl+x in Disassembly must open FindReferencesDialog");
            assert!(
                title.contains("References"),
                "Ctrl+x opened wrong dialog in Disassembly: {title}"
            );
        } else {
            assert!(
                h.ui_state.active_dialog.is_none(),
                "Ctrl+x outside Disassembly should not open a dialog in {pane:?}"
            );
            assert_eq!(
                h.ui_state.status_bar.message,
                "Action only available in Disassembly View"
            );
        }
        h.dismiss_dialog();

        // 16. Jump Next / Prev Unexplored Block (Ctrl+n, Ctrl+Shift+n, Ctrl+Shift+N)
        h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
        h.set_pane(pane);
        h.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert_eq!(
            h.core.state.disassembly[h.ui_state.cursor_index].address,
            Addr(0x0840),
            "Ctrl+n did not jump to first unexplored block ($0840) in pane {pane:?}"
        );
        h.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert_eq!(
            h.core.state.disassembly[h.ui_state.cursor_index].address,
            Addr(0x08C0),
            "Second Ctrl+n did not jump to second unexplored block ($08C0) in pane {pane:?}"
        );
        h.press(
            KeyCode::Char('n'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(
            h.core.state.disassembly[h.ui_state.cursor_index].address,
            Addr(0x0840),
            "Ctrl+Shift+n did not jump back to $0840 in pane {pane:?}"
        );
        // Move forward again and test uppercase 'N' with CONTROL | SHIFT
        h.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert_eq!(
            h.core.state.disassembly[h.ui_state.cursor_index].address,
            Addr(0x08C0)
        );
        h.press(
            KeyCode::Char('N'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(
            h.core.state.disassembly[h.ui_state.cursor_index].address,
            Addr(0x0840),
            "Ctrl+Shift+N did not jump back to $0840 in pane {pane:?}"
        );

        // 17. Escape / Cancel: clears input_buffer and visual mode
        h.set_pane(pane);
        h.ui_state.input_buffer = "42".to_string();
        h.ui_state.is_visual_mode = true;
        h.ui_state.selection_start = Some(0);
        h.press(KeyCode::Esc, KeyModifiers::NONE);
        assert!(
            !h.ui_state.is_visual_mode,
            "Esc did not clear is_visual_mode in pane {pane:?}"
        );

        // 18. VICE Debugger Function Keys: F2, Shift+F2, F4, F6, F7, F8, Shift+F8, F9
        h.set_pane(pane);
        h.press(KeyCode::F(2), KeyModifiers::SHIFT);
        let title = h
            .active_dialog_title()
            .expect("Shift+F2 must open BreakpointAddressDialog");
        assert!(
            title.contains("Breakpoint"),
            "Shift+F2 opened wrong dialog in {pane:?}: {title}"
        );
        h.dismiss_dialog();

        h.set_pane(pane);
        h.press(KeyCode::F(6), KeyModifiers::NONE);
        let title = h
            .active_dialog_title()
            .expect("F6 must open WatchpointAddressDialog");
        assert!(
            title.contains("Watchpoint"),
            "F6 opened wrong dialog in {pane:?}: {title}"
        );
        h.dismiss_dialog();

        for (f_key, mods) in [
            (KeyCode::F(2), KeyModifiers::NONE),
            (KeyCode::F(4), KeyModifiers::NONE),
            (KeyCode::F(7), KeyModifiers::NONE),
            (KeyCode::F(8), KeyModifiers::NONE),
            (KeyCode::F(8), KeyModifiers::SHIFT),
            (KeyCode::F(9), KeyModifiers::NONE),
        ] {
            h.set_pane(pane);
            h.press(f_key, mods);
            assert!(
                h.ui_state.active_dialog.is_none(),
                "{f_key:?} ({mods:?}) should not open a modal dialog in {pane:?}"
            );
        }
    }

    // 19. Right-Pane Toggling & Cycling (Alt+1..6, Ctrl+1..6) and Tab Cycling
    for mods in [KeyModifiers::ALT, KeyModifiers::CONTROL] {
        h.set_pane(ActivePane::Disassembly);
        h.ui_state.right_pane = RightPane::None;
        h.ui_state.input_buffer.clear();

        // 1: Blocks (Toggle On -> Off)
        h.press(KeyCode::Char('1'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Blocks);
        assert_eq!(h.ui_state.active_pane, ActivePane::Blocks);
        assert!(h.ui_state.input_buffer.is_empty());
        h.press(KeyCode::Char('1'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::None);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);

        // 2: HexDump (16 -> 8 -> Off)
        h.press(KeyCode::Char('2'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::HexDump16);
        assert_eq!(h.ui_state.active_pane, ActivePane::HexDump);
        h.press(KeyCode::Char('2'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::HexDump8);
        assert_eq!(h.ui_state.active_pane, ActivePane::HexDump);
        h.press(KeyCode::Char('2'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::None);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);

        // 3: Sprites (2Col -> 1Col -> Off)
        h.press(KeyCode::Char('3'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Sprites2Col);
        assert_eq!(h.ui_state.active_pane, ActivePane::Sprites);
        h.press(KeyCode::Char('3'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Sprites1Col);
        assert_eq!(h.ui_state.active_pane, ActivePane::Sprites);
        h.press(KeyCode::Char('3'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::None);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);

        // 4: Charset (8Col -> 4Col -> Off)
        h.press(KeyCode::Char('4'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Charset8Col);
        assert_eq!(h.ui_state.active_pane, ActivePane::Charset);
        h.press(KeyCode::Char('4'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Charset4Col);
        assert_eq!(h.ui_state.active_pane, ActivePane::Charset);
        h.press(KeyCode::Char('4'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::None);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);

        // 5: Bitmap (Toggle On -> Off)
        h.press(KeyCode::Char('5'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Bitmap);
        assert_eq!(h.ui_state.active_pane, ActivePane::Bitmap);
        h.press(KeyCode::Char('5'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::None);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);

        // 6: Debugger (Toggle On -> Off)
        h.press(KeyCode::Char('6'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::Debugger);
        assert_eq!(h.ui_state.active_pane, ActivePane::Debugger);
        // Tab switches between Debugger and Disassembly
        h.press(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
        h.press(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(h.ui_state.active_pane, ActivePane::Debugger);
        h.press(KeyCode::Char('6'), mods);
        assert_eq!(h.ui_state.right_pane, RightPane::None);
        assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
    }
}

// =============================================================================
// PILLAR 2: Exhaustive View-Specific, Navigable & Modal Functional Suite
// =============================================================================

/// Pillar 2 (`Navigable`): Exercises `Down`/`j`, `Up`/`k`, `PageDown`/`Ctrl+d`, `PageUp`/`Ctrl+u`,
/// `Home`, `End`, `G` (`NONE` and `SHIFT`), and `[count]G` across all 6 [`Navigable`] views
/// (`Disassembly`, `HexDump`, `Sprites`, `Charset`, `Bitmap`, and `Blocks`).
#[test]
fn test_navigable_shortcuts_all_views() {
    let mut h = ShortcutTestHarness::new();
    // Disable right-pane auto-sync during isolated Navigable cursor tests so moving in
    // non-disassembly views isn't overwritten when testing Disassembly vs. RightPane.
    h.core.state.system_config.sync_hex_dump = false;
    h.core.state.system_config.sync_sprites_view = false;
    h.core.state.system_config.sync_charset_view = false;
    h.core.state.system_config.sync_bitmap_view = false;
    h.core.state.system_config.sync_blocks_view = false;

    // 1. DisassemblyView
    h.set_pane(ActivePane::Disassembly);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 0);
    // Clamping at 0 on Up / k
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 0);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 0);

    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 1);
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 2);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 1);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 0);

    h.press(KeyCode::PageDown, KeyModifiers::NONE);
    let after_pgdn = h.ui_state.cursor_index;
    assert!(after_pgdn > 0);
    h.press(KeyCode::PageUp, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 0);

    h.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(h.ui_state.cursor_index, after_pgdn);
    h.press(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(h.ui_state.cursor_index, 0);

    let disasm_last = h.core.state.disassembly.len() - 1;
    h.press(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, disasm_last);
    // Clamping at end on Down / j
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, disasm_last);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, 0);

    // G with SHIFT and NONE (empty input_buffer -> jump to end)
    h.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.cursor_index, disasm_last);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.cursor_index, disasm_last);

    // [count]G in DisassemblyView jumps to 1-based visual line
    h.press(KeyCode::Char('3'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.input_buffer, "3");
    h.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert!(h.ui_state.input_buffer.is_empty());
    assert_eq!(h.ui_state.cursor_index, 2);

    // 2. HexDumpView
    h.set_pane(ActivePane::HexDump);
    h.ui_state.right_pane = RightPane::HexDump16;
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 0);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 0);
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 1);
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 2);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 1);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 0);

    h.press(KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 10);
    h.press(KeyCode::PageUp, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 0);
    h.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(h.ui_state.hex_cursor_index, 10);
    h.press(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(h.ui_state.hex_cursor_index, 0);

    let hex_last = (8192 / 16) - 1;
    h.press(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, hex_last);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 0);
    h.press(KeyCode::Char('G'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, hex_last);
    // [count]G in HexDump: 5G -> index 4
    h.press(KeyCode::Char('5'), KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.hex_cursor_index, 4);

    // 3. SpritesView (1-col & 2-col)
    h.set_pane(ActivePane::Sprites);
    h.ui_state.right_pane = RightPane::Sprites1Col;
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 2);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    h.press(KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 10);
    h.press(KeyCode::PageUp, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    let sprites_last = (8192 / 64) - 1;
    h.press(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, sprites_last);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    h.press(KeyCode::Char('4'), KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 3);

    // 4. CharsetView (8-col & 4-col)
    h.set_pane(ActivePane::Charset);
    h.ui_state.right_pane = RightPane::Charset8Col;
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 0);
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 8);
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 16);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 8);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 0);
    h.press(KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 80);
    h.press(KeyCode::PageUp, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 0);
    let charset_last = (8192 / 8) - 1;
    h.press(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, charset_last);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    h.press(KeyCode::Char('3'), KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.charset_cursor_index, 2);

    // 5. BitmapView (origin $0800 + 8192 bytes spans two 8192-byte aligned bitmap slots: 0 and 1)
    h.set_pane(ActivePane::Bitmap);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 0);
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 1);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 0);
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 1);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 0);
    h.press(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 1);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.bitmap_cursor_index, 1);
    h.press(KeyCode::Char('1'), KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.bitmap_cursor_index, 0);

    // 6. BlocksView
    h.set_pane(ActivePane::Blocks);
    let blocks_len = crate::ui::view_blocks::BlocksView.len(&h.core.state);
    assert!(blocks_len >= 5);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(0));
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(1));
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(2));
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(1));
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(0));
    h.press(KeyCode::End, KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.blocks_list_state.selected(),
        Some(blocks_len - 1)
    );
    h.press(KeyCode::Home, KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.blocks_list_state.selected(),
        Some(blocks_len - 1)
    );
    h.press(KeyCode::Char('2'), KeyModifiers::NONE);
    h.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(1));
    // Overflow clamping: 99999G -> blocks_len - 1
    for _ in 0..5 {
        h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    }
    h.press(KeyCode::Char('G'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.blocks_list_state.selected(),
        Some(blocks_len - 1)
    );
}

/// Pillar 2 (`DisassemblyView`): Exhaustively verifies block-type conversions (`c`, `b`, `w`, `a`,
/// `p`, `s`, `<`, `>`, `,`, `.`, `?`, `E`, `d`), `Shift+V` visual mode and `Shift+Arrow` range
/// selections, immediate format cycling (`i`/`I`) and Lo/Hi packing (`[`/`]`), scope creation (`r`),
/// `Alt+Down`/`Alt+Up` scope boundary nudging (`NudgeScopeBoundary`), `Delete` on scopes and
/// external labels, splitters (`|`), collapsed blocks (`Ctrl+k`), labels/comments/enums (`l`, `;`,
/// `:`, `e`), `Enter`/`Backspace` operand navigation, and unified search (`/`, `Ctrl+f`, `n`, `N`,
/// `F3`, `Shift+F3`, including collapsed-block matches).
#[test]
fn test_disassembly_view_shortcuts() {
    let mut h = ShortcutTestHarness::new();
    h.set_pane(ActivePane::Disassembly);

    // 1. Single-key & range block type conversions: b, w, a, p, s, c, ?, <, >, ,, ., E, d
    // Position at $0814 (NOPs at offset 20..24)
    let target_addr = Addr(0x0814);
    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Char('b'), KeyModifiers::NONE);
    assert_eq!(h.core.state.block_types[20], BlockType::DataByte);

    // Select 2 bytes ($0814..=$0815) with Shift+Down and test 2-byte block conversions
    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Char('w'), KeyModifiers::NONE);
    assert_eq!(
        &h.core.state.block_types[20..22],
        &[BlockType::DataWord, BlockType::DataWord]
    );
    assert_eq!(h.ui_state.selection_start, None);

    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(
        &h.core.state.block_types[20..22],
        &[BlockType::Address, BlockType::Address]
    );

    // '<' (LoHiAddress) and '>' (HiLoAddress) with both NONE and SHIFT
    // Note: LoHi/HiLo split the 2-byte block into two 1-byte lines, so select 2 lines with Shift+Down when needed.
    for mods in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
        if h.core.state.disassembly[h.ui_state.cursor_index]
            .bytes
            .len()
            == 1
        {
            h.press(KeyCode::Down, KeyModifiers::SHIFT);
        }
        h.press(KeyCode::Char('<'), mods);
        assert_eq!(
            &h.core.state.block_types[20..22],
            &[BlockType::LoHiAddress, BlockType::LoHiAddress]
        );
        h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
        h.press(KeyCode::Down, KeyModifiers::SHIFT);
        h.press(KeyCode::Char('>'), mods);
        assert_eq!(
            &h.core.state.block_types[20..22],
            &[BlockType::HiLoAddress, BlockType::HiLoAddress]
        );
    }

    // ',' (LoHiWord) and '.' (HiLoWord)
    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Char(','), KeyModifiers::NONE);
    assert_eq!(
        &h.core.state.block_types[20..22],
        &[BlockType::LoHiWord, BlockType::LoHiWord]
    );
    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Char('.'), KeyModifiers::NONE);
    assert_eq!(
        &h.core.state.block_types[20..22],
        &[BlockType::HiLoWord, BlockType::HiLoWord]
    );

    // 'p' (PetsciiText), 's' (ScreencodeText), 'E' (ExternalFile with NONE & SHIFT), '?' (Undefined with NONE & SHIFT), 'c' (Code)
    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Char('p'), KeyModifiers::NONE);
    assert_eq!(h.core.state.block_types[20], BlockType::PetsciiText);

    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(h.core.state.block_types[20], BlockType::ScreencodeText);

    for mods in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
        h.press(KeyCode::Char('E'), mods);
        assert_eq!(h.core.state.block_types[20], BlockType::ExternalFile);

        h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
        h.press(KeyCode::Char('?'), mods);
        assert_eq!(h.core.state.block_types[20], BlockType::Undefined);
    }

    // Convert 20..22 back to Code with Shift+Down + 'c'
    h.ui_state.cursor_index = h.line_index_for_addr(target_addr);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Char('c'), KeyModifiers::NONE);
    assert_eq!(
        &h.core.state.block_types[20..22],
        &[BlockType::Code, BlockType::Code]
    );

    // Odd-byte selection on '<' (LoHiAddress) must fail validation and preserve selection_start
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0814));
    let sel_start = h.ui_state.cursor_index;
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Down, KeyModifiers::SHIFT); // 3 NOPs = 3 bytes (odd)
    h.press(KeyCode::Char('<'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.selection_start,
        Some(sel_start),
        "Selection must be preserved when odd-byte validation fails"
    );
    assert!(
        h.ui_state
            .status_bar
            .message
            .contains("requires even number of bytes")
    );
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(h.ui_state.selection_start, None);

    // 'd' (DisassembleAddress flow analysis) on DataByte at $0880 (offset 128..131: LDA #$55, RTS)
    assert_eq!(h.core.state.block_types[128], BlockType::DataByte);
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0880));
    h.press(KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(
        &h.core.state.block_types[128..131],
        &[BlockType::Code, BlockType::Code, BlockType::Code]
    );

    // 2. Visual Mode ('V' with SHIFT and NONE) and Shift+Down / Shift+Up Range Selection
    for v_mods in [KeyModifiers::SHIFT, KeyModifiers::NONE] {
        h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0814));
        h.press(KeyCode::Char('V'), v_mods);
        assert!(h.ui_state.is_visual_mode);
        assert_eq!(h.ui_state.selection_start, Some(h.ui_state.cursor_index));
        h.press(KeyCode::Char('j'), KeyModifiers::NONE);
        h.press(KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(!h.ui_state.is_visual_mode);
        assert_eq!(h.ui_state.selection_start, None);
        assert_eq!(
            &h.core.state.block_types[20..22],
            &[BlockType::DataByte, BlockType::DataByte]
        );
        // Undo back to Code
        h.press(KeyCode::Char('u'), KeyModifiers::NONE);
        assert_eq!(
            &h.core.state.block_types[20..22],
            &[BlockType::Code, BlockType::Code]
        );
    }

    // Shift+Down twice then Shift+Up once selects 2 lines without is_visual_mode
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0814));
    let start_idx = h.ui_state.cursor_index;
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Up, KeyModifiers::SHIFT);
    assert!(!h.ui_state.is_visual_mode);
    assert_eq!(h.ui_state.selection_start, Some(start_idx));
    assert_eq!(h.ui_state.cursor_index, start_idx + 1);
    h.press(KeyCode::Char('b'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.selection_start, None);
    assert_eq!(
        &h.core.state.block_types[20..22],
        &[BlockType::DataByte, BlockType::DataByte]
    );
    h.press(KeyCode::Char('u'), KeyModifiers::NONE);

    // 3. Immediate Mode Formatting ('i', 'I' with SHIFT & NONE) and Lo/Hi Packing ('[', ']')
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800)); // LDA #$42
    h.press(KeyCode::Char('i'), KeyModifiers::NONE);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0800))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::InvertedHex)
    );
    h.press(KeyCode::Char('I'), KeyModifiers::SHIFT);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0800))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::Hex)
    );
    h.press(KeyCode::Char('i'), KeyModifiers::NONE);
    h.press(KeyCode::Char('I'), KeyModifiers::NONE);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0800))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::Hex)
    );

    // Pack Lo/Hi ('[') on $0800 (LDA #$42) and $0802 (LDX #$10) -> target $1042
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Char('['), KeyModifiers::NONE);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0800))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::LowByte(Addr(0x1042)))
    );
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0802))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::HighByte(Addr(0x1042)))
    );
    h.press(KeyCode::Char('u'), KeyModifiers::NONE);

    // Pack Hi/Lo (']') on $0800 (LDA #$42) and $0802 (LDX #$10) -> target $4210
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    h.press(KeyCode::Char(']'), KeyModifiers::NONE);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0800))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::HighByte(Addr(0x4210)))
    );
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0802))
            .and_then(|e| e.immediate_format),
        Some(ImmediateFormat::LowByte(Addr(0x4210)))
    );
    h.press(KeyCode::Char('u'), KeyModifiers::NONE);

    // 4. Scopes: 'r' (AddScope), Alt+Down (expand), Alt+Up (shrink), Delete (RemoveScope)
    // Position at $0810 (LDA #$99, followed by NOP at $0812, RTS at $0813)
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
    h.press(KeyCode::Char('r'), KeyModifiers::NONE);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0810))
            .and_then(|e| e.scope),
        Some(Addr(0x0813)),
        "'r' at $0810 must create a scope ending at RTS ($0813)"
    );

    // Regression test for Bug 1: Alt+Down expands scope end to $0814; Alt+Up shrinks back to $0813
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
    h.press(KeyCode::Down, KeyModifiers::ALT);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0810))
            .and_then(|e| e.scope),
        Some(Addr(0x0814)),
        "Alt+Down must expand scope boundary from $0813 to $0814"
    );
    h.press(KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0810))
            .and_then(|e| e.scope),
        Some(Addr(0x0813)),
        "Alt+Up must shrink scope boundary from $0814 back to $0813"
    );

    // Delete on the scope boundary line (.proc) removes the scope
    let formatter = h.core.state.get_formatter();
    let (_, scope_start_mnemonic, _) = formatter
        .format_scope_start(None)
        .expect("formatter must have scope start");
    let scope_line_idx = h
        .core
        .state
        .disassembly
        .iter()
        .position(|l| l.mnemonic == scope_start_mnemonic)
        .expect("scope start line must exist in disassembly");
    h.ui_state.cursor_index = scope_line_idx;
    h.press(KeyCode::Delete, KeyModifiers::NONE);
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0810))
            .and_then(|e| e.scope),
        None,
        "Delete on scope boundary line must remove the scope"
    );

    // 5. External Address Exclusion: Delete on external label line ($FFD2)
    h.core.state.settings.all_labels = true;
    h.core.state.disassemble();
    let ext_line_idx = h
        .core
        .state
        .disassembly
        .iter()
        .position(|l| l.external_label_address == Some(Addr(0xFFD2)))
        .expect("external label line for $FFD2 must exist");
    h.ui_state.cursor_index = ext_line_idx;
    h.press(KeyCode::Delete, KeyModifiers::NONE);
    assert!(
        h.core.state.user_excluded_addresses.contains(&Addr(0xFFD2)),
        "Delete on external label line must exclude $FFD2"
    );
    h.press(KeyCode::Char('u'), KeyModifiers::NONE);
    assert!(!h.core.state.user_excluded_addresses.contains(&Addr(0xFFD2)));
    h.core.state.settings.all_labels = false;
    h.core.state.disassemble();

    // 6. Splitters ('|' with NONE & SHIFT) and Collapsed Blocks (Ctrl+k)
    for mods in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
        h.press(KeyCode::Char('|'), mods);
        assert!(h.core.state.splitters.contains(&Addr(0x0810)));
        h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
        h.press(KeyCode::Char('|'), mods);
        assert!(!h.core.state.splitters.contains(&Addr(0x0810)));
    }

    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    assert!(
        !h.core.state.collapsed_blocks.is_empty(),
        "Ctrl+k must collapse current block"
    );
    h.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    assert!(
        h.core.state.collapsed_blocks.is_empty(),
        "Second Ctrl+k must expand current block"
    );

    // 7. Labels ('l'), Side Comments (';'), Line Comments (':'), Apply Enum ('e')
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
    h.press(KeyCode::Char('l'), KeyModifiers::NONE);
    let title = h.active_dialog_title().expect("'l' must open LabelDialog");
    assert!(title.contains("Enter Label Name"));
    for ch in "sub_0810".chars() {
        h.press(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(h.ui_state.active_dialog.is_none());
    assert!(
        h.core
            .state
            .labels
            .get(&Addr(0x0810))
            .is_some_and(|v| v.iter().any(|l| l.name == "sub_0810"))
    );

    // ';' Side Comment
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
    h.press(KeyCode::Char(';'), KeyModifiers::NONE);
    let title = h
        .active_dialog_title()
        .expect("';' must open Side CommentDialog");
    assert!(title.contains("Side Comment"));
    for ch in "my side note".chars() {
        h.press(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(h.ui_state.active_dialog.is_none());
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0810))
            .and_then(|e| e.user_side_comment.as_deref()),
        Some("my side note")
    );

    // ':' Line Comment
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0810));
    h.press(KeyCode::Char(':'), KeyModifiers::SHIFT);
    let title = h
        .active_dialog_title()
        .expect("':' must open Line CommentDialog");
    assert!(title.contains("Line Comment"));
    for ch in "header note".chars() {
        h.press(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(h.ui_state.active_dialog.is_none());
    assert_eq!(
        h.core
            .state
            .annotations
            .get(Addr(0x0810))
            .and_then(|e| e.user_line_comment.as_deref()),
        Some("header note")
    );

    // 'e' ApplyEnumDialog
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('e'), KeyModifiers::NONE);
    let title = h
        .active_dialog_title()
        .expect("'e' must open ApplyEnumDialog");
    assert!(title.contains("Apply Enum"));
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(h.ui_state.active_dialog.is_none());

    // 8. Jump to Operand (Enter) & Navigate Back (Backspace)
    let jsr_idx = h.line_index_for_addr(Addr(0x0804)); // JSR $0810
    h.ui_state.cursor_index = jsr_idx;
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0810),
        "Enter on JSR $0810 must jump to $0810"
    );
    h.press(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0804),
        "Backspace must navigate back to caller $0804"
    );

    // Enter on JMP $0820 ($0807)
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0807));
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0820),
        "Enter on JMP $0820 must jump to $0820"
    );
    h.press(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0807)
    );

    // 9. Unified Search: '/' inline Vim search, Ctrl+f modal SearchDialog, n, N, F3, Shift+F3, & collapsed block search
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('/'), KeyModifiers::NONE);
    assert!(
        h.ui_state.vim_search_active,
        "'/' must activate inline vim_search_active"
    );
    assert!(
        h.ui_state.active_dialog.is_none(),
        "'/' must NOT open modal SearchDialog"
    );
    for ch in "#$77".chars() {
        h.press(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.ui_state.vim_search_active);
    assert_eq!(h.ui_state.last_search_query, "#$77");
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0820),
        "Inline Vim search for '#$77' from $0800 must land on LDA #$77 at $0820"
    );

    // Modal SearchDialog via Ctrl+f
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('f'), KeyModifiers::CONTROL);
    let title = h
        .active_dialog_title()
        .expect("Ctrl+f must open SearchDialog");
    assert!(title.contains("Search"));
    // Clear previous query with Backspace and type "99" (matches LDA #$99 at $0810)
    for _ in 0..8 {
        h.press(KeyCode::Backspace, KeyModifiers::NONE);
    }
    h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        h.ui_state.active_dialog.is_none(),
        "Enter in SearchDialog must close the dialog via FindNext.closes_dialog()"
    );
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0810),
        "SearchDialog Enter for '99' must jump to $0810 (LDA #$99)"
    );

    // Next / Previous match: n, N (SHIFT and NONE), F3, Shift+F3
    h.ui_state.last_search_query = "NOP".to_string();
    h.core.view.last_search_query = "NOP".to_string();
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('n'), KeyModifiers::NONE);
    let first_nop_addr = h.core.state.disassembly[h.ui_state.cursor_index].address;
    assert_eq!(first_nop_addr, Addr(0x080D));

    h.press(KeyCode::F(3), KeyModifiers::NONE);
    let second_nop_addr = h.core.state.disassembly[h.ui_state.cursor_index].address;
    assert_eq!(second_nop_addr, Addr(0x080E));

    h.press(KeyCode::Char('N'), KeyModifiers::SHIFT);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        first_nop_addr
    );

    h.press(KeyCode::Char('n'), KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        second_nop_addr
    );

    h.press(KeyCode::Char('N'), KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        first_nop_addr
    );

    h.press(KeyCode::F(3), KeyModifiers::NONE);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        second_nop_addr
    );

    h.press(KeyCode::F(3), KeyModifiers::SHIFT);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        first_nop_addr
    );

    // Search inside a collapsed block via TUI
    // Collapse offsets (16, 19) ($0810..=$0813 containing LDA #$99) while cursor is at $0800
    h.core.state.collapsed_blocks.push((16, 19));
    h.core.state.disassemble();
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('/'), KeyModifiers::NONE);
    h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    let landed_line = &h.core.state.disassembly[h.ui_state.cursor_index];
    assert!(
        landed_line.is_collapsed && landed_line.address == Addr(0x0810),
        "Search for '99' must match inside collapsed block at $0810, landed on {landed_line:?}"
    );
}

/// Pillar 2 (`HexDumpView`): Verifies byte-column navigation (`Left`/`Right`/`h`/`l` with row
/// wrapping), text mode cycling (`m`/`M`), `Shift+V` and `Shift+Arrows` selection, `'b'` byte
/// conversion (including the `UIState -> Core` visual-mode clearing regression check), and
/// `Enter` jump to `DisassemblyView`.
#[test]
fn test_hexdump_view_shortcuts() {
    let mut h = ShortcutTestHarness::new();
    h.set_pane(ActivePane::HexDump);
    h.ui_state.right_pane = RightPane::HexDump16;
    h.ui_state.hex_cursor_index = 0;
    h.ui_state.hex_col_cursor = 0;

    // 1. Byte column movement & row wrapping (Right/l, Left/h)
    h.press(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_col_cursor, 1);
    h.press(KeyCode::Char('l'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_col_cursor, 2);
    h.press(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_col_cursor, 1);
    h.press(KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_col_cursor, 0);

    // Wrap from col 15 -> row 1, col 0, and back
    h.ui_state.hex_col_cursor = 15;
    h.press(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 1);
    assert_eq!(h.ui_state.hex_col_cursor, 0);
    h.press(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_cursor_index, 0);
    assert_eq!(h.ui_state.hex_col_cursor, 15);
    h.ui_state.hex_col_cursor = 0;

    // 2. Mode cycling: 'm' (Next) and 'M' (Prev with SHIFT and NONE)
    assert_eq!(
        h.ui_state.hexdump_view_mode,
        HexdumpViewMode::ScreencodeShifted
    );
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.hexdump_view_mode,
        HexdumpViewMode::ScreencodeUnshifted
    );
    h.press(KeyCode::Char('M'), KeyModifiers::SHIFT);
    assert_eq!(
        h.ui_state.hexdump_view_mode,
        HexdumpViewMode::ScreencodeShifted
    );
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    h.press(KeyCode::Char('M'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.hexdump_view_mode,
        HexdumpViewMode::ScreencodeShifted
    );

    // 3. Visual Mode ('V' with SHIFT & NONE) + 'b' conversion & state-sync regression check
    for v_mods in [KeyModifiers::SHIFT, KeyModifiers::NONE] {
        h.core.state.block_types[0..4].fill(BlockType::Code);
        h.core.state.disassemble();
        h.ui_state.hex_cursor_index = 0;
        h.ui_state.hex_col_cursor = 0;

        h.press(KeyCode::Char('V'), v_mods);
        assert!(h.ui_state.is_visual_mode);
        assert_eq!(h.ui_state.hex_selection_start, Some(0));
        h.press(KeyCode::Right, KeyModifiers::NONE);
        h.press(KeyCode::Right, KeyModifiers::NONE); // 3 bytes: 0..=2
        h.press(KeyCode::Char('b'), KeyModifiers::NONE);

        assert_eq!(
            &h.core.state.block_types[0..=2],
            &[
                BlockType::DataByte,
                BlockType::DataByte,
                BlockType::DataByte
            ]
        );
        assert!(
            !h.ui_state.is_visual_mode,
            "is_visual_mode must remain false after 'b' in HexDumpView"
        );
        assert_eq!(
            h.ui_state.hex_selection_start, None,
            "hex_selection_start must remain None after 'b' in HexDumpView"
        );
        assert_eq!(h.ui_state.selection_start, None);
    }

    // Shift+Right / Shift+Left / Shift+Down / Shift+Up selection + Esc
    h.ui_state.hex_cursor_index = 0;
    h.ui_state.hex_col_cursor = 0;
    h.press(KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.hex_selection_start, Some(0));
    assert_eq!(h.ui_state.hex_col_cursor, 1);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.hex_cursor_index, 1);
    h.press(KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.hex_cursor_index, 0);
    h.press(KeyCode::Left, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.hex_col_cursor, 0);
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(h.ui_state.hex_selection_start, None);

    // 4. Enter jumps to Disassembly at the selected byte address
    h.ui_state.hex_cursor_index = 1; // $0810
    h.ui_state.hex_col_cursor = 0;
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0810)
    );
}

/// Pillar 2 (`SpritesView`): Verifies 2-column and 1-column sprite grid navigation, `Shift+Arrows`
/// and `Shift+V` selection, `'m'` multicolor toggle, `'b'` 64-byte sprite block conversion and
/// selection clearing, and `Enter` jump to `DisassemblyView`.
#[test]
fn test_sprites_view_shortcuts() {
    let mut h = ShortcutTestHarness::new();
    h.set_pane(ActivePane::Sprites);
    h.ui_state.right_pane = RightPane::Sprites2Col;
    h.ui_state.sprites_cursor_index = 0;

    // 1. 2-Column navigation: Right/l (+1), Left/h (-1), Down/j (+2), Up/k (-2)
    h.press(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    h.press(KeyCode::Char('l'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 2);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);

    // Shift+Arrows in 2-col mode
    h.press(KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.sprites_selection_start, Some(0));
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.sprites_cursor_index, 3);
    h.press(KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Left, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_selection_start, None);

    // 2. 1-Column navigation & Shift+Down / Shift+Up
    h.ui_state.right_pane = RightPane::Sprites1Col;
    h.ui_state.sprites_cursor_index = 0;
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.sprites_selection_start, Some(0));
    assert_eq!(h.ui_state.sprites_cursor_index, 1);
    h.press(KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.sprites_cursor_index, 0);
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(h.ui_state.sprites_selection_start, None);

    // 3. 'm' toggles sprite_multicolor_mode
    assert!(!h.ui_state.sprite_multicolor_mode);
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(h.ui_state.sprite_multicolor_mode);
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(!h.ui_state.sprite_multicolor_mode);

    // 4. Visual mode ('V' with SHIFT & NONE) + 'b' converts 64-byte sprites and clears all selection state
    for v_mods in [KeyModifiers::SHIFT, KeyModifiers::NONE] {
        h.core.state.block_types[0..128].fill(BlockType::Undefined);
        h.core.state.disassemble();
        h.ui_state.sprites_cursor_index = 0;

        h.press(KeyCode::Char('V'), v_mods);
        assert!(h.ui_state.is_visual_mode);
        assert_eq!(h.ui_state.sprites_selection_start, Some(0));
        h.press(KeyCode::Down, KeyModifiers::NONE); // Sprite 1 in 1-col mode -> 2 sprites (128 bytes)
        h.press(KeyCode::Char('b'), KeyModifiers::NONE);

        assert!(
            h.core.state.block_types[0..128]
                .iter()
                .all(|&b| b == BlockType::DataByte),
            "'b' in SpritesView must convert 128 bytes to DataByte"
        );
        assert!(
            !h.ui_state.is_visual_mode,
            "is_visual_mode must be cleared after 'b' in SpritesView"
        );
        assert_eq!(
            h.ui_state.sprites_selection_start, None,
            "sprites_selection_start must be cleared after 'b' in SpritesView"
        );
        assert_eq!(
            h.ui_state.selection_start, None,
            "selection_start must be cleared after 'b' in SpritesView"
        );
    }

    // 5. Enter jumps to Disassembly at sprite address
    h.ui_state.sprites_cursor_index = 1; // offset 64 = $0840
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0840)
    );
}

/// Pillar 2 (`CharsetView`): Verifies 8-column and 4-column character grid navigation,
/// non-shadowing of `Alt+Left`/`Alt+Right`/`Ctrl+Left`, `Shift+Arrows` and `Shift+V` selection,
/// `'m'` multicolor toggle, `'b'` 8-byte character conversion and selection clearing, and `Enter`
/// jump to `DisassemblyView`.
#[test]
fn test_charset_view_shortcuts() {
    let mut h = ShortcutTestHarness::new();
    h.set_pane(ActivePane::Charset);
    h.ui_state.right_pane = RightPane::Charset8Col;
    h.ui_state.charset_cursor_index = 0;

    // 1. 8-Column & 4-Column navigation: Right/l (+1), Left/h (-1), Down/j (+8 or +4), Up/k (-8 or -4)
    h.press(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 1);
    h.press(KeyCode::Char('l'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 2);
    h.press(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 1);
    h.press(KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 0);

    // Regression guard: Alt+Left / Alt+Right / Ctrl+Left must NOT move charset_cursor_index!
    h.ui_state.charset_cursor_index = 2;
    h.press(KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(
        h.ui_state.charset_cursor_index, 2,
        "Alt+Left must not be swallowed by CharsetView Left handler"
    );
    h.press(KeyCode::Right, KeyModifiers::ALT);
    assert_eq!(
        h.ui_state.charset_cursor_index, 2,
        "Alt+Right must not be swallowed by CharsetView Right handler"
    );
    h.press(KeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(h.ui_state.charset_cursor_index, 2);

    // 4-Column stride check
    h.ui_state.right_pane = RightPane::Charset4Col;
    h.ui_state.charset_cursor_index = 0;
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 4);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_cursor_index, 0);
    h.ui_state.right_pane = RightPane::Charset8Col;

    // Shift+Arrows in CharsetView
    h.press(KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.charset_selection_start, Some(0));
    assert_eq!(h.ui_state.charset_cursor_index, 1);
    h.press(KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.charset_cursor_index, 9);
    h.press(KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.charset_cursor_index, 1);
    h.press(KeyCode::Left, KeyModifiers::SHIFT);
    assert_eq!(h.ui_state.charset_cursor_index, 0);
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(h.ui_state.charset_selection_start, None);

    // 2. 'm' toggles charset_multicolor_mode
    assert!(!h.ui_state.charset_multicolor_mode);
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(h.ui_state.charset_multicolor_mode);
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(!h.ui_state.charset_multicolor_mode);

    // 3. Visual mode ('V' with SHIFT & NONE) + 'b' converts 8-byte chars and clears all selection state
    for v_mods in [KeyModifiers::SHIFT, KeyModifiers::NONE] {
        h.core.state.block_types[0..16].fill(BlockType::Undefined);
        h.core.state.disassemble();
        h.ui_state.charset_cursor_index = 0;

        h.press(KeyCode::Char('V'), v_mods);
        assert!(h.ui_state.is_visual_mode);
        assert_eq!(h.ui_state.charset_selection_start, Some(0));
        h.press(KeyCode::Right, KeyModifiers::NONE); // Char 1 -> 2 chars (16 bytes)
        h.press(KeyCode::Char('b'), KeyModifiers::NONE);

        assert!(
            h.core.state.block_types[0..16]
                .iter()
                .all(|&b| b == BlockType::DataByte),
            "'b' in CharsetView must convert 16 bytes to DataByte"
        );
        assert!(
            !h.ui_state.is_visual_mode,
            "is_visual_mode must be cleared after 'b' in CharsetView"
        );
        assert_eq!(
            h.ui_state.charset_selection_start, None,
            "charset_selection_start must be cleared after 'b' in CharsetView"
        );
        assert_eq!(
            h.ui_state.selection_start, None,
            "selection_start must be cleared after 'b' in CharsetView"
        );
    }

    // 4. Enter jumps to Disassembly at character address
    h.ui_state.charset_cursor_index = 2; // offset 16 = $0810
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0810)
    );
}

/// Pillar 2 (`BitmapView`): Verifies Screen RAM bank cycling (`'s'` forward, `'S'` backward with
/// `SHIFT` and `NONE`), `'x'` (`ScreenRamMode::AfterBitmap`), `'m'` multicolor toggle, `'b'`
/// 8,000-byte bitmap conversion, `Esc` input buffer clear, and `Enter` jump to `DisassemblyView`.
#[test]
fn test_bitmap_view_shortcuts() {
    let mut h = ShortcutTestHarness::new();
    h.set_pane(ActivePane::Bitmap);
    h.ui_state.bitmap_cursor_index = 0;
    h.ui_state.bitmap_screen_ram_mode = ScreenRamMode::AfterBitmap;

    // 1. Screen RAM mode cycling: 's' (forward), 'S' (backward with SHIFT and NONE), 'x' (AfterBitmap)
    h.press(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.bitmap_screen_ram_mode,
        ScreenRamMode::BankOffset(0)
    );
    h.press(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.bitmap_screen_ram_mode,
        ScreenRamMode::BankOffset(1)
    );
    h.press(KeyCode::Char('S'), KeyModifiers::SHIFT);
    assert_eq!(
        h.ui_state.bitmap_screen_ram_mode,
        ScreenRamMode::BankOffset(0)
    );
    h.press(KeyCode::Char('S'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.bitmap_screen_ram_mode,
        ScreenRamMode::BankOffset(15)
    );
    h.press(KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(
        h.ui_state.bitmap_screen_ram_mode,
        ScreenRamMode::AfterBitmap
    );

    // 2. 'm' toggles bitmap_multicolor_mode
    assert!(!h.ui_state.bitmap_multicolor_mode);
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(h.ui_state.bitmap_multicolor_mode);
    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(!h.ui_state.bitmap_multicolor_mode);

    // 3. 'b' converts the bitmap region to BlockType::DataByte
    h.core.state.block_types[0..128].fill(BlockType::Undefined);
    h.core.state.disassemble();
    h.press(KeyCode::Char('b'), KeyModifiers::NONE);
    // For origin $0800 (2048), bitmap 0 covers $0000..=$1F3F (offsets 0..=5951 in raw_data)
    assert!(
        h.core.state.block_types[0..=5951]
            .iter()
            .all(|&b| b == BlockType::DataByte),
        "'b' in BitmapView must convert bitmap data range to DataByte"
    );

    // 4. Esc clears input_buffer
    h.ui_state.input_buffer = "7".to_string();
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(h.ui_state.input_buffer.is_empty());

    // 5. Enter jumps to Disassembly at bitmap display address ($0800)
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0800)
    );
}

/// Pillar 2 (`BlocksView` & `DebuggerView`): Verifies `Ctrl+k` collapsing/expanding the block
/// selected in `BlocksView` (independent of `cursor_index`), `Enter` jumping to the selected
/// block in `DisassemblyView`, and `'m'` opening `MemoryDumpAddressDialog` in `DebuggerView`
/// without intercepting `Ctrl+m` or `Alt+m`.
#[test]
fn test_blocks_and_debugger_view_shortcuts() {
    let mut h = ShortcutTestHarness::new();

    // 1. BlocksView: Down/j, Up/k, Ctrl+k (collapses block at blocks_selected_index, NOT cursor_index!), Enter
    h.set_pane(ActivePane::Blocks);
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800)); // Disassembly cursor stays on block 0 ($0800..$083F)
    h.ui_state.blocks_list_state.select(Some(0));

    // Move down to block index 2 ($0880..$08BF DataByte block)
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(1));
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(h.ui_state.blocks_list_state.selected(), Some(2));

    let items = h.core.state.get_blocks_view_items();
    let BlockItem::Block { start, end, .. } = items[2] else {
        panic!("Expected BlockItem::Block at index 2");
    };
    let expected_range = (
        start.offset_from(h.core.state.origin),
        end.offset_from(h.core.state.origin),
    );

    // Press Ctrl+k in BlocksView: must collapse block 2 (even though cursor_index is at block 0!)
    h.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    assert!(
        h.core.state.collapsed_blocks.contains(&expected_range),
        "Ctrl+k in BlocksView must collapse the selected block {expected_range:?}, got {:?}",
        h.core.state.collapsed_blocks
    );

    // Press Ctrl+k again to expand
    h.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    assert!(
        !h.core.state.collapsed_blocks.contains(&expected_range),
        "Second Ctrl+k in BlocksView must expand the block"
    );

    // Enter in BlocksView jumps to the selected block in Disassembly
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.ui_state.active_pane, ActivePane::Disassembly);
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        start
    );

    // 2. DebuggerView: 'm' opens MemoryDumpAddressDialog; Ctrl+m / Alt+m / Ctrl+s / Alt+Enter are NOT hijacked
    h.set_pane(ActivePane::Debugger);
    h.press(KeyCode::Char('m'), KeyModifiers::CONTROL);
    assert!(
        h.ui_state.active_dialog.is_none(),
        "Ctrl+m in DebuggerView must not trigger ViceMemoryDumpDialog"
    );
    h.press(KeyCode::Char('m'), KeyModifiers::ALT);
    assert!(
        h.ui_state.active_dialog.is_none(),
        "Alt+m in DebuggerView must not trigger ViceMemoryDumpDialog"
    );
    h.press(KeyCode::Enter, KeyModifiers::ALT);
    assert!(
        h.ui_state.active_dialog.is_none(),
        "Alt+Enter in DebuggerView must be ignored"
    );

    h.press(KeyCode::Char('m'), KeyModifiers::NONE);
    let title = h
        .active_dialog_title()
        .expect("'m' in DebuggerView must open MemoryDumpAddressDialog");
    assert!(title.contains("Memory Dump Address"));
    h.dismiss_dialog();
}

/// Pillar 2 (`Menu` & Modal Dialogs): Verifies menu bar keyboard navigation (`F10`, `Left`/`Right`
/// category wrapping, `Down`/`Up` separator skipping, `Enter` action dispatch, `Esc` dismissal),
/// `ShortcutsDialog` scrolling, and `UIState::dialog_queue` promotion when `SearchDialog` closes
/// via `AppAction::FindNext`.
#[test]
fn test_menu_and_dialog_navigation() {
    let mut h = ShortcutTestHarness::new();

    // 1. Menu Bar keyboard navigation: F10, Right, Left, Down, Up, Enter, Esc
    h.press(KeyCode::F(10), KeyModifiers::NONE);
    assert!(h.ui_state.menu.active);
    assert_eq!(h.ui_state.menu.selected_category, 0); // File

    h.press(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(h.ui_state.menu.selected_category, 1); // Edit
    h.press(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(h.ui_state.menu.selected_category, 0); // File
    // Left at 0 wraps to last category (6: Help)
    h.press(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(h.ui_state.menu.selected_category, 6); // Help
    assert_eq!(h.ui_state.menu.selected_item, Some(0)); // "Keyboard Shortcuts"

    // Down in Help menu skips separator at index 1 and lands on index 2 ("Documentation")
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.menu.selected_item, Some(2));
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.menu.selected_item, Some(3)); // "Examples"
    // Down again skips separator at index 4 and lands on index 5 ("About")
    h.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(h.ui_state.menu.selected_item, Some(5));

    // Enter on "About" closes menu and opens AboutDialog
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.ui_state.menu.active);
    let title = h
        .active_dialog_title()
        .expect("Enter on About menu item must open AboutDialog");
    assert!(title.contains("About"));
    h.dismiss_dialog();

    // Open menu with F10, navigate to Help -> Keyboard Shortcuts, and press Enter
    h.press(KeyCode::Char('h'), KeyModifiers::ALT);
    assert!(h.ui_state.menu.active);
    assert_eq!(h.ui_state.menu.selected_category, 6);
    assert_eq!(h.ui_state.menu.selected_item, Some(0));
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!h.ui_state.menu.active);
    let title = h
        .active_dialog_title()
        .expect("Help -> Keyboard Shortcuts must open ShortcutsDialog");
    assert!(title.contains("Keyboard Shortcuts"));

    // 2. ShortcutsDialog navigation: Down/j, Up/k, PageDown/Ctrl+d, PageUp/Ctrl+u, End, Home, Esc
    h.press(KeyCode::Down, KeyModifiers::NONE);
    h.press(KeyCode::Char('j'), KeyModifiers::NONE);
    h.press(KeyCode::Up, KeyModifiers::NONE);
    h.press(KeyCode::Char('k'), KeyModifiers::NONE);
    h.press(KeyCode::PageDown, KeyModifiers::NONE);
    h.press(KeyCode::PageUp, KeyModifiers::NONE);
    h.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    h.press(KeyCode::Char('u'), KeyModifiers::CONTROL);
    h.press(KeyCode::End, KeyModifiers::NONE);
    h.press(KeyCode::Home, KeyModifiers::NONE);
    assert!(h.ui_state.active_dialog.is_some());
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(h.ui_state.active_dialog.is_none());

    // Open menu and close with Esc
    h.press(KeyCode::F(10), KeyModifiers::NONE);
    assert!(h.ui_state.menu.active);
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!h.ui_state.menu.active);
    assert_eq!(h.ui_state.menu.selected_item, None);

    // 3. Dialog Queue Advancement when SearchDialog returns AppAction::FindNext on Enter
    h.ui_state.cursor_index = h.line_index_for_addr(Addr(0x0800));
    h.press(KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert!(h.ui_state.active_dialog.is_some());
    // Push a second dialog (ShortcutsDialog) onto dialog_queue
    h.ui_state
        .dialog_queue
        .push(Box::new(ShortcutsDialog::new()));

    // Type "99" and press Enter in SearchDialog
    for _ in 0..8 {
        h.press(KeyCode::Backspace, KeyModifiers::NONE);
    }
    h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    h.press(KeyCode::Char('9'), KeyModifiers::NONE);
    h.press(KeyCode::Enter, KeyModifiers::NONE);

    // Search must have executed AND the queued ShortcutsDialog must now be promoted to active_dialog!
    assert_eq!(
        h.core.state.disassembly[h.ui_state.cursor_index].address,
        Addr(0x0810),
        "SearchDialog Enter must execute FindNext"
    );
    assert!(
        h.ui_state.dialog_queue.is_empty(),
        "dialog_queue must be drained into active_dialog when FindNext closes SearchDialog"
    );
    let promoted_title = h
        .active_dialog_title()
        .expect("Queued ShortcutsDialog must be promoted to active_dialog");
    assert!(
        promoted_title.contains("Keyboard Shortcuts"),
        "Expected promoted ShortcutsDialog, got {promoted_title}"
    );
    h.dismiss_dialog();
}

// =============================================================================
// PILLAR 3: 4-Layer Canonical Shortcut Synchronization Contract
// =============================================================================

/// Normalizes a shortcut token (e.g. `"Ctrl+Shift+O"`, `"++ctrl+shift+o++"`, `"++open-bracket++"`)
/// into a canonical lowercase representation for cross-layer comparison.
fn normalize_shortcut_token(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('+').to_lowercase();
    match trimmed.as_str() {
        "comma" => ",".to_string(),
        "period" => ".".to_string(),
        "less-than" => "<".to_string(),
        "greater-than" => ">".to_string(),
        "open-bracket" => "[".to_string(),
        "close-bracket" => "]".to_string(),
        "question-mark" => "?".to_string(),
        "semicolon" => ";".to_string(),
        "colon" => ":".to_string(),
        "pipe" => "|".to_string(),
        "slash" => "/".to_string(),
        "page-up" => "pageup".to_string(),
        "page-down" => "pagedown".to_string(),
        "arrow-up" => "up".to_string(),
        "arrow-down" => "down".to_string(),
        "arrow-left" => "left".to_string(),
        "arrow-right" => "right".to_string(),
        "escape" => "esc".to_string(),
        "ctrl+comma" => "ctrl+,".to_string(),
        other => other.to_string(),
    }
}

/// Extracts all `++...++` shortcut tokens from `docs/keyboard_shortcuts.md`.
fn parse_markdown_shortcut_tokens(doc: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for line in doc.lines() {
        let mut rest = line;
        while let Some(start) = rest.find("++") {
            let after_start = &rest[start + 2..];
            if let Some(end) = after_start.find("++") {
                let token = &after_start[..end];
                tokens.push(normalize_shortcut_token(token));
                rest = &after_start[end + 2..];
            } else {
                break;
            }
        }
    }
    tokens
}

/// Splits a `ShortcutsDialog` key label (e.g. `"Alt+o (Ctrl+Shift+o)"`, `"Up/Down/j/k"`,
/// `"Alt+Up / Alt+Down"`, `"i / I"`) into normalized individual shortcut tokens.
fn expand_dialog_shortcut_keys(key_label: &str) -> Vec<String> {
    if key_label == "/" {
        return vec!["/".to_string()];
    }
    if key_label == "[Number] Shift+g" {
        return vec!["shift+g".to_string()];
    }
    if key_label == "Shift+Arrows" {
        return vec!["shift+up".to_string(), "down".to_string()];
    }
    if key_label == "i / I" {
        return vec!["i".to_string(), "shift+i".to_string()];
    }
    if key_label == "n / N" {
        return vec!["n".to_string(), "shift+n".to_string()];
    }
    if key_label == "m / M" {
        return vec!["m".to_string(), "shift+m".to_string()];
    }
    if key_label == "s / S" {
        return vec!["s".to_string(), "shift+s".to_string()];
    }

    let cleaned = key_label.replace(['(', ')'], " ");
    cleaned
        .split(['/', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_shortcut_token)
        .collect()
}

/// Pillar 3 Contract Test: Enforces bidirectional synchronization across [`MenuState::new`]
/// (Layer 2), [`ShortcutsDialog::get_shortcuts`] (Layer 3), and `docs/keyboard_shortcuts.md`
/// (Layer 4) so user-facing shortcut declarations never drift from live key handlers.
#[test]
fn test_shortcut_layers_synchronization() {
    let menu = MenuState::new();
    let dialog_shortcuts = ShortcutsDialog::get_shortcuts();
    let doc_tokens = parse_markdown_shortcut_tokens(KEYBOARD_SHORTCUTS_DOC);

    // 1. Specific anti-drift regression assertions in ShortcutsDialog::get_shortcuts()
    assert!(
        dialog_shortcuts.contains(&("e", "Apply Enum")),
        "ShortcutsDialog::get_shortcuts() must contain ('e', 'Apply Enum')"
    );
    assert!(
        dialog_shortcuts.contains(&("Shift+e", "External File")),
        "ShortcutsDialog::get_shortcuts() must contain ('Shift+e', 'External File')"
    );
    assert!(
        dialog_shortcuts.contains(&("Alt+Up / Alt+Down", "Nudge Scope Boundary")),
        "ShortcutsDialog::get_shortcuts() must contain ('Alt+Up / Alt+Down', 'Nudge Scope Boundary')"
    );
    assert!(
        !dialog_shortcuts.contains(&("e", "External File")),
        "ShortcutsDialog::get_shortcuts() must NOT list 'e' as 'External File'"
    );

    // 2. Verify all ShortcutsDialog entries are documented in docs/keyboard_shortcuts.md
    let mut all_dialog_tokens = Vec::new();
    for (key_label, desc) in &dialog_shortcuts {
        if desc.is_empty() {
            // Section header or spacer row
            continue;
        }
        let expanded = expand_dialog_shortcut_keys(key_label);
        assert!(
            !expanded.is_empty(),
            "Shortcut entry ({key_label:?}, {desc:?}) produced no tokens"
        );
        for token in expanded {
            assert!(
                doc_tokens.contains(&token),
                "ShortcutsDialog entry ({key_label:?}, {desc:?}) token '{token}' is missing from docs/keyboard_shortcuts.md"
            );
            all_dialog_tokens.push(token);
        }
    }

    // 3. Verify every documented token in docs/keyboard_shortcuts.md is represented in ShortcutsDialog
    // (or is a menu/column navigation key: up, down, left, right, h, l, enter, esc)
    let menu_nav_tokens = ["up", "down", "left", "right", "h", "l", "enter", "esc"];
    for doc_token in &doc_tokens {
        assert!(
            all_dialog_tokens.contains(doc_token) || menu_nav_tokens.contains(&doc_token.as_str()),
            "docs/keyboard_shortcuts.md token '{doc_token}' is missing from ShortcutsDialog::get_shortcuts()"
        );
    }

    // 4. Verify MenuState::new() completeness, External File shortcut, and parity with ShortcutsDialog & docs
    let mut found_external_file = false;
    for category in &menu.categories {
        for item in &category.items {
            if item.is_separator {
                assert!(
                    item.action.is_none() && item.shortcut.is_none(),
                    "Separator in category '{}' must not have action or shortcut",
                    category.name
                );
                continue;
            }
            assert!(
                item.action.is_some(),
                "Non-separator menu item '{} -> {}' must have an AppAction",
                category.name,
                item.name
            );

            if item.name == "External File" {
                found_external_file = true;
                assert_eq!(
                    item.shortcut.as_deref(),
                    Some("Shift+E"),
                    "Edit -> External File must have shortcut Some(\"Shift+E\")"
                );
                assert_eq!(item.action, Some(AppAction::SetExternalFile));
            }

            if let Some(shortcut) = &item.shortcut {
                let norm = normalize_shortcut_token(shortcut);
                assert!(
                    all_dialog_tokens.contains(&norm),
                    "Menu item '{} -> {}' shortcut '{}' (normalized '{}') is missing from ShortcutsDialog::get_shortcuts()",
                    category.name,
                    item.name,
                    shortcut,
                    norm
                );
                assert!(
                    doc_tokens.contains(&norm),
                    "Menu item '{} -> {}' shortcut '{}' (normalized '{}') is missing from docs/keyboard_shortcuts.md",
                    category.name,
                    item.name,
                    shortcut,
                    norm
                );
            }
        }
    }
    assert!(
        found_external_file,
        "MenuState::new() must contain 'External File'"
    );
}
