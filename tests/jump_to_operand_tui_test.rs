//! End-to-end TUI integration test for multi-address Jump to Operand workflows.
//!
//! Exercises `tests/6502/test_dispatch_table.regen2000proj` through the headless key-event loop
//! and Ratatui [`TestBackend`] across LO/HI split dispatch tables (`$082F` / `$0832`), numeric
//! quick-jump hotkeys, cursor navigation, `Esc` dismissal, single-target jumps, and `Backspace`
//! history restoration.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use regenerator2000_core::Core;
use regenerator2000_core::state::Addr;
use regenerator2000_tui::events::process_key_event;
use regenerator2000_tui::theme::Theme;
use regenerator2000_tui::ui::ui;
use regenerator2000_tui::ui_state::UIState;
use std::path::Path;

fn load_dispatch_table_fixture() -> (Core, UIState) {
    let mut core = Core::new();
    let proj_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/6502/test_dispatch_table.regen2000proj");
    core.state
        .load_project(&proj_path)
        .expect("failed to load test_dispatch_table.regen2000proj");

    let mut ui_state = UIState::new(Theme::default());
    ui_state.dismiss_logo = true;
    (core, ui_state)
}

fn find_body_line_index(core: &Core, addr: Addr) -> usize {
    core.state
        .disassembly
        .iter()
        .position(|l| {
            l.external_label_address.is_none() && l.address == addr && !l.bytes.is_empty()
        })
        .unwrap_or_else(|| panic!("expected non-empty body line at ${:04X}", addr.0))
}

fn sync_cursor_to_addr(core: &mut Core, ui_state: &mut UIState, addr: Addr) {
    let idx = find_body_line_index(core, addr);
    ui_state.cursor_index = idx;
    ui_state.sub_cursor_index = 0;
    ui_state.sync_tui_to_core();
    core.view = ui_state.core.clone();
}

fn press_key(core: &mut Core, ui_state: &mut UIState, code: KeyCode) {
    process_key_event(core, ui_state, KeyEvent::new(code, KeyModifiers::NONE));
}

fn render_screen(core: &Core, ui_state: &mut UIState) -> String {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui(f, &core.state, ui_state)).unwrap();
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn jump_to_operand_tui_dispatch_table_e2e_workflow() {
    let (mut core, mut ui_state) = load_dispatch_table_fixture();

    // 1. LO split-table row at $082F opens picker with all 3 targets ($0835, $083F, $0849).
    sync_cursor_to_addr(&mut core, &mut ui_state, Addr(0x082F));
    press_key(&mut core, &mut ui_state, KeyCode::Enter);
    assert!(ui_state.active_dialog.is_some());
    assert!(ui_state.navigation_history.is_empty());

    let screen_lo = render_screen(&core, &mut ui_state);
    assert!(
        screen_lo.contains("Jump to Operand from $082F"),
        "{screen_lo}"
    );
    assert!(screen_lo.contains("[1]"), "{screen_lo}");
    assert!(screen_lo.contains("[2]"), "{screen_lo}");
    assert!(screen_lo.contains("[3]"), "{screen_lo}");
    assert!(screen_lo.contains("$0835"), "{screen_lo}");
    assert!(screen_lo.contains("$083F"), "{screen_lo}");
    assert!(screen_lo.contains("$0849"), "{screen_lo}");
    assert!(
        screen_lo.contains("<(state_0_border_red - 1)"),
        "{screen_lo}"
    );
    assert!(
        screen_lo.contains("<(state_1_border_green - 1)"),
        "{screen_lo}"
    );
    assert!(
        screen_lo.contains("<(state_2_border_blue - 1)"),
        "{screen_lo}"
    );
    assert!(screen_lo.contains("VicColor.RED"), "{screen_lo}");
    assert!(screen_lo.contains("VicColor.GREEN"), "{screen_lo}");
    assert!(screen_lo.contains("VicColor.BLUE"), "{screen_lo}");

    // Pressing hotkey '2' closes the dialog and lands on body line $083F (state_1_border_green).
    press_key(&mut core, &mut ui_state, KeyCode::Char('2'));
    assert!(ui_state.active_dialog.is_none());
    let landed_green = &core.state.disassembly[ui_state.cursor_index];
    assert!(landed_green.external_label_address.is_none());
    assert_eq!(landed_green.address, Addr(0x083F));
    assert!(!landed_green.bytes.is_empty());
    assert_eq!(ui_state.navigation_history.len(), 1);

    // Backspace returns to $082F and empties navigation history.
    press_key(&mut core, &mut ui_state, KeyCode::Backspace);
    assert_eq!(
        core.state.disassembly[ui_state.cursor_index].address,
        Addr(0x082F)
    );
    assert!(ui_state.navigation_history.is_empty());

    // 2. HI split-table row at $0832 opens picker; Down + Down + Enter lands on $0849.
    sync_cursor_to_addr(&mut core, &mut ui_state, Addr(0x0832));
    press_key(&mut core, &mut ui_state, KeyCode::Enter);
    assert!(ui_state.active_dialog.is_some());

    let screen_hi = render_screen(&core, &mut ui_state);
    assert!(
        screen_hi.contains("Jump to Operand from $0832"),
        "{screen_hi}"
    );
    assert!(
        screen_hi.contains(">(state_0_border_red - 1)"),
        "{screen_hi}"
    );
    assert!(
        screen_hi.contains(">(state_1_border_green - 1)"),
        "{screen_hi}"
    );
    assert!(
        screen_hi.contains(">(state_2_border_blue - 1)"),
        "{screen_hi}"
    );

    press_key(&mut core, &mut ui_state, KeyCode::Down);
    press_key(&mut core, &mut ui_state, KeyCode::Down);
    press_key(&mut core, &mut ui_state, KeyCode::Enter);
    assert!(ui_state.active_dialog.is_none());
    let landed_blue = &core.state.disassembly[ui_state.cursor_index];
    assert!(landed_blue.external_label_address.is_none());
    assert_eq!(landed_blue.address, Addr(0x0849));
    assert!(!landed_blue.bytes.is_empty());
    assert_eq!(ui_state.navigation_history.len(), 1);

    // Backspace returns to $0832.
    press_key(&mut core, &mut ui_state, KeyCode::Backspace);
    assert_eq!(
        core.state.disassembly[ui_state.cursor_index].address,
        Addr(0x0832)
    );
    assert!(ui_state.navigation_history.is_empty());

    // 3. Pressing Esc closes the picker without navigating or pushing history.
    press_key(&mut core, &mut ui_state, KeyCode::Enter);
    assert!(ui_state.active_dialog.is_some());
    press_key(&mut core, &mut ui_state, KeyCode::Esc);
    assert!(ui_state.active_dialog.is_none());
    assert_eq!(
        core.state.disassembly[ui_state.cursor_index].address,
        Addr(0x0832)
    );
    assert!(ui_state.navigation_history.is_empty());

    // 4. Single-target instruction at $081E (`jsr dispatch_border_state`) jumps directly with no picker.
    sync_cursor_to_addr(&mut core, &mut ui_state, Addr(0x081E));
    press_key(&mut core, &mut ui_state, KeyCode::Enter);
    assert!(ui_state.active_dialog.is_none());
    let landed_dispatch = &core.state.disassembly[ui_state.cursor_index];
    assert!(landed_dispatch.external_label_address.is_none());
    assert_eq!(landed_dispatch.address, Addr(0x0824));
    assert!(!landed_dispatch.bytes.is_empty());
    assert_eq!(ui_state.navigation_history.len(), 1);

    press_key(&mut core, &mut ui_state, KeyCode::Backspace);
    assert_eq!(
        core.state.disassembly[ui_state.cursor_index].address,
        Addr(0x081E)
    );
    assert!(ui_state.navigation_history.is_empty());
}
