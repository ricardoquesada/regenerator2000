#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End-to-end parity tests between the TUI unpack completion + `ImportContextDialog`
//! confirmation flow and the MCP `r2000_unpack_binary` flow (`T4.4.1`, `T4.4.2`).

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

// T4.4.1
#[test]
fn tui_and_mcp_produce_identical_project_state() {
    let fixture = "c64_FppScroller.byte_boozer2.prg";
    let path = fixture_path(fixture);
    let (tui_state, tui_view, _) = run_tui_unpack_flow(&path);
    let (mcp_state, mcp_view) = run_mcp_unpack_flow(&path);

    assert_project_state_parity(fixture, &tui_state, &tui_view, &mcp_state, &mcp_view);
}

// T4.4.2
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
