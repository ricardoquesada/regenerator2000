---
name: update-keyboard-shortcuts
description: >-
  Audits and synchronizes keyboard shortcuts across live TUI input handlers,
  MenuState::new, ShortcutsDialog::get_shortcuts, docs/keyboard_shortcuts.md,
  docs/views.md, and keyboard_shortcuts_tests.rs. Use when adding, changing, or
  auditing TUI keyboard shortcuts. Don't use for non-shortcut TUI styling or
  MCP tool schemas.
---

# Update Keyboard Shortcuts Workflow

Keep all four shortcut layers and the 3-pillar contract test synchronized
whenever you add, modify, or audit a keyboard shortcut.

## Synchronized Files

| Layer | File | Role |
| :--- | :--- | :--- |
| **Layer 1 (Live Input)** | `crates/regenerator2000-tui/src/events.rs` | Top-level routing (`process_key_event` -> dialog -> menu -> Vim search -> active pane -> global fallback) |
| **Layer 1 (Global)** | `crates/regenerator2000-tui/src/events/input.rs` | Global fallback shortcuts (`handle_global_input`, `global_action_for_key`) across all 7 panes |
| **Layer 1 (Shared Nav)** | `crates/regenerator2000-tui/src/ui/navigable.rs` | Shared movement, paging, `[number]G`, visual selection, and search (`handle_nav_input`) |
| **Layer 1 (Views)** | `crates/regenerator2000-tui/src/ui/view_disassembly/mod.rs`, `view_hexdump.rs`, `view_sprites.rs`, `view_charset.rs`, `view_bitmap.rs`, `view_blocks.rs`, `view_debugger.rs` | Per-pane `Widget::handle_input()` bindings |
| **Layer 2 (Menu)** | `crates/regenerator2000-tui/src/ui/menu/menu_model.rs` (dispatch in `menu_action.rs`) | Top menu items and shortcut hints (`MenuState::new()`) |
| **Layer 3 (Help Dialog)** | `crates/regenerator2000-tui/src/ui/dialog_keyboard_shortcut.rs` | `const SHORTCUTS` returned by `ShortcutsDialog::get_shortcuts()` |
| **Layer 4 (Docs)** | `docs/keyboard_shortcuts.md` and `docs/views.md` | User-facing MkDocs shortcut tables (`++key++` notation) |
| **Contract Tests** | `crates/regenerator2000-tui/src/keyboard_shortcuts_tests.rs` | Automated 3-pillar verification across all 7 panes and all 4 layers |

## 1. Run the Automated 3-Pillar Contract Suite First

Run the shortcut test suite to detect drift across layers immediately:

```bash
cargo test -p regenerator2000-tui keyboard_shortcuts_tests
```

- **Pillar 1 (`test_global_shortcuts_across_all_panes`)**: Verifies every
  global shortcut across all 7 `ActivePane` variants so view-local keys never
  hijack global bindings.
- **Pillar 2 (`test_navigable_shortcuts_all_views`, `test_*_view_shortcuts`,
  `test_menu_and_dialog_navigation`)**: Exercises view-local, `Navigable`,
  selection, scope-nudging, and dialog keys through `process_key_event`.
- **Pillar 3 (`test_shortcut_layers_synchronization`)**: Checks bidirectional
  token parity across `MenuState::new()`, `ShortcutsDialog::get_shortcuts()`,
  `docs/keyboard_shortcuts.md`, and `docs/views.md`.

## 2. Update All Four Layers

### Layer 1: View `handle_input()` or `events/input.rs`

- **Plain keys in views**: Guard with `key.modifiers.is_empty()` so `Ctrl+*` and
  `Alt+*` return `WidgetResult::Ignored` and reach `handle_global_input`:

  ```rust
  KeyCode::Char('x') if key.modifiers.is_empty() => {
      WidgetResult::Action(AppAction::SomeAction)
  }
  KeyCode::Char('X') if key.modifiers == KeyModifiers::SHIFT => {
      WidgetResult::Action(AppAction::SomeShiftAction)
  }
  ```

- **Shifted punctuation (`?`, `<`, `>`, `|`, `;`, `:`, `+`)**: Accept
  `key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT` because
  some terminals include `SHIFT` on punctuation characters and others omit it.
- **`Ctrl+Shift+*` and `Alt+Shift+*` in `events/input.rs`**: Always match both
  cases (`KeyCode::Char('g' | 'G')`) because legacy ANSI terminals report
  lowercase + `SHIFT` while Kitty keyboard enhancement reports uppercase +
  `SHIFT`.

### Layer 2: `crates/regenerator2000-tui/src/ui/menu/menu_model.rs`

Update `MenuState::new()` (and `MenuState::update_availability()` if context-sensitive):

```rust
MenuItem::new("Action Label", Some("Ctrl+B"), Some(AppAction::ToggleBookmark))
```

Wire any new `AppAction` variant in `crates/regenerator2000-tui/src/ui/menu/menu_action.rs`
and `regenerator2000-core`.

### Layer 3: `crates/regenerator2000-tui/src/ui/dialog_keyboard_shortcut.rs`

Add or update the `(key, description)` tuple in `const SHORTCUTS`:

- `("Section Header", "")` renders a bold underlined section header.
- `("", "")` renders a blank spacer row.
- `("Alt+1 (Ctrl+1)", "Toggle Blocks View")` lists primary and alternate chords
  so Pillar 3 expands and checks both tokens.

### Layer 4: `docs/keyboard_shortcuts.md` and `docs/views.md`

Use MkDocs Material `++key++` notation:

- Modifiers: `++ctrl+b++`, `++alt+2++`, `++shift+e++`, `++ctrl+shift+g++`,
  `++ctrl+comma++`, `++alt+up++` / `++alt+down++`
- Symbols: `++open-bracket++` (`[`), `++close-bracket++` (`]`),
  `++less-than++` (`<`), `++greater-than++` (`>`), `++pipe++` (`|`),
  `++semicolon++` (`;`), `++colon++` (`:`), `++comma++` (`,`),
  `++period++` (`.`), `++plus++` (`+`), `++minus++` (`-`),
  `++slash++` (`/`), `++question-mark++` (`?`)

## 3. Add Regression Coverage and Verify

Add assertions for new bindings in
`crates/regenerator2000-tui/src/keyboard_shortcuts_tests.rs`, then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p regenerator2000-tui keyboard_shortcuts_tests
```
