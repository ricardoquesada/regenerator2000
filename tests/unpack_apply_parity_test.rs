#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End-to-end parity tests between the TUI unpack completion + `ImportContextDialog`
//! confirmation flow and the MCP `r2000_unpack_binary` flow.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regenerator2000_core::Core;
use regenerator2000_core::mcp::tools::session_tools;
use regenerator2000_core::packers::registry::REGISTRY;
use regenerator2000_core::state::AppState;
use regenerator2000_core::unpacker::{UnpackConfig, unpack};
use regenerator2000_core::view_state::CoreViewState;
use regenerator2000_tui::events::handle_unpack_complete;
use regenerator2000_tui::theme::Theme;
use regenerator2000_tui::ui::widget::WidgetResult;
use regenerator2000_tui::ui_state::UIState;
use serde_json::json;

fn fixture_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/6502")
        .join(rel)
}

fn run_tui_unpack_flow(prg_path: &Path) -> (AppState, CoreViewState, Option<&'static str>) {
    let mut core = Core::new();
    let mut ui_state = UIState::new(Theme::default());
    core.state
        .load_file(prg_path)
        .unwrap_or_else(|e| panic!("TUI load_file({}) failed: {e}", prg_path.display()));

    let result = unpack(
        &core.state.raw_data,
        core.state.origin.0,
        &UnpackConfig::default(),
        None,
    );
    let detected_packer = result.as_ref().ok().and_then(|r| r.packer);
    handle_unpack_complete(result, &mut core, &mut ui_state);

    let mut dialog = ui_state
        .active_dialog
        .take()
        .expect("ImportContextDialog should be pushed after handle_unpack_complete");
    let widget_res = dialog.handle_input(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut core.state,
        &mut ui_state,
    );
    assert_eq!(widget_res, WidgetResult::Close);

    (core.state, ui_state.core, detected_packer)
}

fn run_mcp_unpack_flow(prg_path: &Path) -> (AppState, CoreViewState) {
    let mut app_state = AppState::new();
    let mut view_state = CoreViewState::new();
    app_state
        .load_file(prg_path)
        .unwrap_or_else(|e| panic!("MCP load_file({}) failed: {e}", prg_path.display()));

    session_tools::handle_call(
        "r2000_unpack_binary",
        &json!({}),
        &mut app_state,
        &mut view_state,
    )
    .expect("r2000_unpack_binary must be handled")
    .unwrap_or_else(|e| panic!("MCP unpack({}) failed: {}", prg_path.display(), e.message));

    (app_state, view_state)
}

fn assert_project_state_parity(
    fixture: &str,
    tui_state: &AppState,
    tui_view: &CoreViewState,
    mcp_state: &AppState,
    mcp_view: &CoreViewState,
) {
    assert_eq!(
        tui_state.origin, mcp_state.origin,
        "{fixture}: origin mismatch"
    );
    assert_eq!(
        tui_state.raw_data, mcp_state.raw_data,
        "{fixture}: raw_data mismatch"
    );
    assert_eq!(
        tui_state.entry_point, mcp_state.entry_point,
        "{fixture}: entry_point mismatch"
    );
    assert_eq!(
        tui_state.block_types, mcp_state.block_types,
        "{fixture}: block_types mismatch"
    );
    assert_eq!(
        tui_state.labels, mcp_state.labels,
        "{fixture}: labels mismatch"
    );
    assert_eq!(
        tui_state.disassembly, mcp_state.disassembly,
        "{fixture}: disassembly mismatch"
    );
    assert_eq!(
        tui_state.cross_refs, mcp_state.cross_refs,
        "{fixture}: cross_refs mismatch"
    );
    assert_eq!(
        tui_state.undo_stack, mcp_state.undo_stack,
        "{fixture}: undo_stack mismatch"
    );
    assert_eq!(
        tui_view.cursor_index, mcp_view.cursor_index,
        "{fixture}: cursor_index mismatch"
    );
}

#[test]
fn tui_and_mcp_produce_identical_project_state() {
    let fixture = "c64_FppScroller.byte_boozer2.prg";
    let path = fixture_path(fixture);
    let (tui_state, tui_view, _) = run_tui_unpack_flow(&path);
    let (mcp_state, mcp_view) = run_mcp_unpack_flow(&path);

    assert_project_state_parity(fixture, &tui_state, &tui_view, &mcp_state, &mcp_view);
}

#[test]
fn parity_over_one_file_per_hooked_packer() {
    let cases: &[(&str, &str)] = &[
        ("byteboozer", "c64_FppScroller.byte_boozer2.prg"),
        ("tscrunch", "c64_moving_tubes_lxt.tscrunch_x.prg"),
        ("dali", "c64_moving_tubes_lxt.dali.prg"),
        ("exomizer", "c64_moving_tubes_lxt.exo3.prg"),
    ];

    let expected_hooked_ids: BTreeSet<&str> = REGISTRY
        .iter()
        .filter(|spec| spec.hooks.is_some())
        .map(|spec| spec.id)
        .collect();
    let covered_ids: BTreeSet<&str> = cases.iter().map(|&(id, _)| id).collect();
    assert_eq!(
        covered_ids, expected_hooked_ids,
        "cases table must cover every hooked packer in REGISTRY"
    );

    for &(expected_spec_id, fixture) in cases {
        let path = fixture_path(fixture);
        let (tui_state, tui_view, detected_packer) = run_tui_unpack_flow(&path);
        assert_eq!(
            detected_packer,
            Some(expected_spec_id),
            "fixture {fixture} did not match expected hooked spec {expected_spec_id}"
        );
        let (mcp_state, mcp_view) = run_mcp_unpack_flow(&path);

        assert_project_state_parity(fixture, &tui_state, &tui_view, &mcp_state, &mcp_view);
    }
}

#[test]
fn tui_unpack_poke4k_respects_unchecked_entry_disassembly_toggle() {
    use regenerator2000_core::state::actions::AppAction;
    use regenerator2000_core::state::{Addr, BlockType, EntryAnalysisMode};

    let path = fixture_path("c64_poke4k_1_1.alz64.prg");
    let mut core = Core::new();
    let mut ui_state = UIState::new(Theme::default());
    let loaded = core
        .state
        .load_file(&path)
        .unwrap_or_else(|error| panic!("load_file({}) failed: {error}", path.display()));
    ui_state.restore_session(&loaded, &core.state);

    // 1st ImportContextDialog is open; navigate to checkbox (field 3) and uncheck it, then click < Unpack > (field 5).
    let mut first_dialog = ui_state
        .active_dialog
        .take()
        .expect("1st ImportContextDialog must be open after loading c64_poke4k_1_1.alz64.prg");
    // Initial focus on packed binary is field 5 (< Unpack >); Shift+Tab twice reaches field 3 (checkbox).
    for _ in 0..2 {
        let step_result = first_dialog.handle_input(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &mut core.state,
            &mut ui_state,
        );
        assert_eq!(step_result, WidgetResult::Handled);
    }
    let toggle_result = first_dialog.handle_input(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &mut core.state,
        &mut ui_state,
    );
    assert_eq!(toggle_result, WidgetResult::Handled);

    // Tab twice back to field 5 (< Unpack >) and press Enter.
    for _ in 0..2 {
        let step_result = first_dialog.handle_input(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut core.state,
            &mut ui_state,
        );
        assert_eq!(step_result, WidgetResult::Handled);
    }
    let unpack_action = first_dialog.handle_input(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut core.state,
        &mut ui_state,
    );
    assert_eq!(unpack_action, WidgetResult::Action(AppAction::UnpackBinary));
    assert_eq!(
        ui_state.pending_unpack_entry_mode,
        Some(EntryAnalysisMode::SkipEntryAnalysis)
    );

    let result = unpack(
        &core.state.raw_data,
        core.state.origin.0,
        &UnpackConfig::default(),
        None,
    );
    handle_unpack_complete(result, &mut core, &mut ui_state);

    // While the 2nd dialog is open, unpacked bytes must remain Undefined with no "start" label or disassembled instructions.
    assert!(
        core.state
            .block_types
            .iter()
            .all(|&block_type| block_type == BlockType::Undefined),
        "unpacked bytes must remain Undefined while 2nd ImportContextDialog is open"
    );
    assert!(
        !core
            .state
            .labels
            .values()
            .any(|labels| labels.iter().any(|label| label.name == "start")),
        "unpacked binary must not have 'start' label while 2nd ImportContextDialog is open"
    );

    // Confirming the 2nd dialog without touching its checkbox preserves SkipEntryAnalysis.
    let mut second_dialog = ui_state
        .active_dialog
        .take()
        .expect("2nd ImportContextDialog must be open after handle_unpack_complete");
    let confirm_result = second_dialog.handle_input(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut core.state,
        &mut ui_state,
    );
    assert_eq!(confirm_result, WidgetResult::Close);
    assert!(
        core.state
            .block_types
            .iter()
            .all(|&block_type| block_type == BlockType::Undefined),
        "confirming 2nd dialog with preserved unchecked toggle must keep bytes Undefined"
    );
    assert!(
        !core
            .state
            .labels
            .values()
            .any(|labels| labels.iter().any(|label| label.name == "start")),
        "confirming 2nd dialog with preserved unchecked toggle must create no 'start' label"
    );
    assert!(
        core.state.undo_stack.active_commands().is_empty(),
        "confirming SkipEntryAnalysis must leave undo_stack empty"
    );

    // Confirming with AnalyzeAndLabel disassembles entry point $080D (`stx $d011`).
    let entry_address = core.state.entry_point.expect("entry_point must be set");
    core.state.apply_import_context(
        core.state.settings.system.clone(),
        core.state.origin,
        entry_address,
        EntryAnalysisMode::AnalyzeAndLabel,
    );
    assert!(
        core.state
            .labels
            .get(&Addr(0x080D))
            .is_some_and(|labels| labels.iter().any(|label| label.name == "start")),
        "confirming with AnalyzeAndLabel must label entry $080D as start"
    );
    assert!(
        core.state
            .disassembly
            .iter()
            .any(|line| line.address == Addr(0x080D) && line.mnemonic == "stx"),
        "entry $080D must disassemble to stx when AnalyzeAndLabel is confirmed"
    );
}
