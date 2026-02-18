---
name: update-keyboard-shortcuts
description: Audits and synchronizes keyboard shortcuts across docs/keyboard_shortcuts.md, src/ui/dialog_keyboard_shortcut.rs, src/ui/menu.rs, and the view files (view_disassembly.rs, view_hexdump.rs, view_charset.rs, view_bitmap.rs, view_blocks.rs, view_sprites.rs).
---

# Update Keyboard Shortcuts Workflow

Use this skill when the user asks to:

- "check keyboard shortcuts are in sync"
- "add a new keyboard shortcut"
- "update the keyboard shortcuts documentation"
- "verify shortcuts are consistent"

## Files to Keep in Sync

The following files all define or document keyboard shortcuts and must be consistent with each other:

| File                                 | Role                                                |
| ------------------------------------ | --------------------------------------------------- |
| `docs/keyboard_shortcuts.md`         | User-facing documentation (MkDocs table format)     |
| `src/ui/dialog_keyboard_shortcut.rs` | In-app shortcuts dialog (`get_shortcuts()` fn)      |
| `src/ui/menu.rs`                     | Menu items with shortcut hints (`MenuState::new()`) |
| `src/ui/view_disassembly.rs`         | Disassembly view `handle_input()`                   |
| `src/ui/view_hexdump.rs`             | Hex dump view `handle_input()`                      |
| `src/ui/view_charset.rs`             | Charset view `handle_input()`                       |
| `src/ui/view_bitmap.rs`              | Bitmap view `handle_input()`                        |
| `src/ui/view_blocks.rs`              | Blocks view `handle_input()`                        |
| `src/ui/view_sprites.rs`             | Sprites view `handle_input()`                       |

## 1. Read All Relevant Files

Read all files listed above to get the current state of shortcuts in each location.

## 2. Audit for Discrepancies

Compare shortcuts across the three "declaration" layers:

### Layer 1: `handle_input()` in view files (ground truth)

These are the **actual** key bindings. What the code does is what the user experiences.

### Layer 2: `menu.rs` shortcut hints

These are displayed in the menu UI next to menu items. They should match what `handle_input()` actually handles.

### Layer 3: `dialog_keyboard_shortcut.rs` → `get_shortcuts()`

These are shown in the in-app help dialog. They should match the actual behavior.

### Layer 4: `docs/keyboard_shortcuts.md`

User-facing documentation. Should match all of the above.

### Common Discrepancies to Look For

- A shortcut exists in the code but is **missing** from the docs or dialog.
- A shortcut is documented but **not implemented** in any view's `handle_input()`.
- The shortcut key string is **formatted differently** across files (e.g., `"Ctrl+B"` vs `"Ctrl+b"`).
- A shortcut is listed under the **wrong context/view** in the docs.
- A shortcut exists in `menu.rs` but the corresponding `MenuAction` is never triggered by any key.
- The `ToggleBlocksView` menu shows `Alt+6` but the docs say `Alt+5`.

## 3. Report Discrepancies

List all discrepancies found, grouped by type:

```
MISSING FROM DOCS:
  - [key] [action] (found in: view_X.rs handle_input)

MISSING FROM DIALOG:
  - [key] [action] (found in: view_X.rs handle_input)

WRONG KEY IN MENU:
  - Menu shows "[key]" but handle_input uses "[key]" for [action]

WRONG KEY IN DOCS:
  - Docs say "[key]" but handle_input uses "[key]" for [action]

UNDOCUMENTED SHORTCUTS:
  - [key] in [file] has no corresponding entry in docs or dialog
```

## 4. Fix Discrepancies

For each discrepancy, update the appropriate file(s):

### Updating `docs/keyboard_shortcuts.md`

The file uses MkDocs Material `++key++` notation. Format examples:

- `++ctrl+b++` for Ctrl+B
- `++alt+2++` for Alt+2
- `++shift+v++` for Shift+V
- `++open-bracket++` for `[`
- `++close-bracket++` for `]`
- `++less-than++` for `<`
- `++greater-than++` for `>`
- `++pipe++` for `|`
- `++semicolon++` for `;`
- `++colon++` for `:`
- `++comma++` for `,`
- `++period++` for `.`
- `++question-mark++` for `?`

### Updating `src/ui/dialog_keyboard_shortcut.rs`

Edit the `get_shortcuts()` function. Each entry is a `(&str, &str)` tuple:

- First element: key string (human readable, e.g. `"Ctrl+b"`, `"Alt+2 (Ctrl+2)"`)
- Second element: action description
- Empty `("", "")` = blank separator line
- `("Section Header", "")` = section header (rendered bold+underlined)

### Updating `src/ui/menu.rs`

Edit `MenuState::new()`. Each `MenuItem::new()` call has:

- `name`: menu item label
- `shortcut`: `Some("Ctrl+B")` or `None`
- `action`: the `MenuAction` variant

### Updating view `handle_input()` functions

Add or modify `KeyCode::Char(...)` match arms. Follow the existing patterns:

```rust
KeyCode::Char('x') if key.modifiers.is_empty() => {
    WidgetResult::Action(MenuAction::SomeAction)
}
KeyCode::Char('X') if key.modifiers == KeyModifiers::SHIFT => {
    WidgetResult::Action(MenuAction::SomeOtherAction)
}
KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
    WidgetResult::Action(MenuAction::SomeCtrlAction)
}
```

## 5. Verify

After making changes, do a final cross-check:

1. Confirm every shortcut in `handle_input()` appears in `get_shortcuts()` and `docs/keyboard_shortcuts.md`.
2. Confirm every shortcut in the docs is actually implemented in some `handle_input()`.
3. Confirm menu shortcut hints match the actual key bindings.

## Known Quirks

- **`ToggleBlocksView`**: The menu shows `Alt+6` but the docs say `Alt+5`. The actual key handling is in the global event handler (not in a view's `handle_input()`), so check `src/ui/menu.rs` and the main event loop for this one.
- **Shifted characters**: Some keys like `?`, `<`, `>`, `|`, `;`, `:` are handled with `key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT` because on some terminals/OSes, pressing Shift to get these characters sends a SHIFT modifier. This is intentional.
- **Global shortcuts** (Ctrl+O, Ctrl+S, etc.) are handled in the main event loop, not in individual view `handle_input()` functions.
- **Navigation shortcuts** (Up/Down/j/k, PageUp/PageDown, Home/End, G) are handled by the `Navigable` trait via `handle_nav_input()`, not directly in `handle_input()`.
