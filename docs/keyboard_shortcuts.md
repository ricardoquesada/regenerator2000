---
icon: material/keyboard-outline
description: Complete keyboard shortcut reference for Regenerator 2000 organized by pane, editing action, navigation, search, debugger, and menus.
---

# Keyboard Shortcuts

Regenerator 2000 is keyboard-first: mouse drags and clicks select or scroll, while single keys and chords drive every edit and jump.

- **Block types:** Single letters or `Shift` + letter convert blocks (such as ++c++ for Code or ++b++ for Byte).
- **Dialogs & global actions:** `Ctrl` + letter or `Alt` + letter chords open dialogs and run commands. Every `Ctrl` shortcut that commonly collides with terminals or `tmux` (like ++ctrl+2++) has an `Alt` alternative (++alt+2++).

---

## General & File Operations

| Action | Shortcut |
| :--- | :--- |
| **Activate Main Menu** | ++f10++ |
| **Quit** | ++ctrl+q++ |
| **Open File** | ++ctrl+o++ |
| **Open Recent Projects** | ++alt+o++ or ++ctrl+shift+o++ |
| **Save Project** | ++ctrl+s++ |
| **Save Project As** | ++alt+s++ or ++ctrl+shift+s++ |
| **Export to .asm** | ++ctrl+e++ |
| **Export to .asm as...** | ++alt+e++ or ++ctrl+shift+e++ |
| **Import / Export VICE Labels** | *(menu only)* |
| **Document Settings** | ++alt+d++ or ++ctrl+shift+d++ |
| **Settings** | ++alt+p++ or ++ctrl+comma++ |
| **Undo** | ++u++ |
| **Redo** | ++ctrl+r++ |
| **Switch Pane** *(Disassembly ↔ right pane)* | ++tab++ |

---

## Companion Views

| Action | Shortcut |
| :--- | :--- |
| **Toggle Blocks View** | ++alt+1++ or ++ctrl+1++ |
| **Cycle Hex Dump View** *(16 cols → 8 cols → Off)* | ++alt+2++ or ++ctrl+2++ |
| **Cycle Sprites View** *(2 cols → 1 col → Off)* | ++alt+3++ or ++ctrl+3++ |
| **Cycle Charset View** *(8 cols → 4 cols → Off)* | ++alt+4++ or ++ctrl+4++ |
| **Toggle Bitmap View** | ++alt+5++ or ++ctrl+5++ |
| **Toggle Debugger Panel** | ++alt+6++ or ++ctrl+6++ |

---

## Disassembly — Editing

| Action | Shortcut |
| :--- | :--- |
| **Disassemble Address (flow analysis)** | ++d++ |
| **Convert to Code** | ++c++ |
| **Convert to Byte** | ++b++ |
| **Convert to Word** | ++w++ |
| **Convert to Address** | ++a++ |
| **Convert to PETSCII Text** | ++p++ |
| **Convert to Screencode Text** | ++s++ |
| **Convert to External File** | ++shift+e++ |
| **Convert to Undefined** | ++question-mark++ |
| **Set Lo/Hi Address Table** | ++less-than++ |
| **Set Hi/Lo Address Table** | ++greater-than++ |
| **Set Lo/Hi Word Table** | ++comma++ |
| **Set Hi/Lo Word Table** | ++period++ |
| **Pack Lo/Hi Address (Immediate Mode)** | ++open-bracket++ |
| **Pack Hi/Lo Address (Immediate Mode)** | ++close-bracket++ |
| **Next/Prev Immediate Mode Format** *(Hex, Dec, Bin)* | ++i++ / ++shift+i++ |
| **Apply Enum** *(Immediate, Byte, Word)* | ++e++ |
| **Manage Enums...** | *(menu only)* |
| **Set Label** | ++l++ |
| **Set Symbolic Offset** | ++o++ |
| **Increment/Decrement Symbolic Offset** | ++plus++ / ++minus++ |
| **Add Side Comment** | ++semicolon++ |
| **Add Line Comment** | ++colon++ |
| **Create Scope** | ++r++ |
| **Nudge Scope Boundary** | ++alt+up++ / ++alt+down++ |
| **Remove Scope / Exclude External Address** | ++delete++ |
| **Toggle Splitter** | ++pipe++ |
| **Toggle Collapsed Block** | ++ctrl+k++ |
| **Toggle Bookmark** | ++ctrl+b++ |
| **List Bookmarks** | ++ctrl+shift+b++ or ++alt+b++ |
| **Analyze** | ++ctrl+a++ |

---

## Disassembly — Navigation

| Action | Shortcut |
| :--- | :--- |
| **Move Cursor** | ++up++ / ++down++ / ++j++ / ++k++ |
| **Previous/Next 10 Lines** | ++ctrl+u++ / ++ctrl+d++ |
| **Page Up/Down** | ++page-up++ / ++page-down++ |
| **Home/End** | ++home++ / ++end++ |
| **Jump to Address** | ++ctrl+g++ or ++alt+g++ |
| **Jump to Line** | ++ctrl+shift+g++ or ++alt+shift+g++ |
| **Jump to Line / End of File** | number + ++shift+g++ |
| **Jump to Operand** *(1 target immediate; 2+ picker)* | ++enter++ |
| **Jump Back (History)** | ++backspace++ |
| **Next Unexplored Block** | ++ctrl+n++ |
| **Prev Unexplored Block** | ++ctrl+shift+n++ |

---

## Companion View Controls

### Hexdump

| Action | Shortcut |
| :--- | :--- |
| **Column Navigation** | ++left++ / ++right++ / ++h++ / ++l++ |
| **Convert to Byte** | ++b++ |
| **Next / Prev Hex Text Mode** *(Screencode / PETSCII shifted & unshifted)* | ++m++ / ++shift+m++ |
| **Jump to Disassembly + update cursor** | ++enter++ |

### Sprites

| Action | Shortcut |
| :--- | :--- |
| **Convert to Byte** | ++b++ |
| **Toggle Multicolor Sprites** | ++m++ |
| **Jump to Disassembly + update cursor** | ++enter++ |

### Charset

| Action | Shortcut |
| :--- | :--- |
| **Convert to Byte** | ++b++ |
| **Toggle Multicolor Charset** | ++m++ |
| **Jump to Disassembly + update cursor** | ++enter++ |

### Bitmap

| Action | Shortcut |
| :--- | :--- |
| **Convert to Byte** | ++b++ |
| **Toggle Multicolor Bitmap** | ++m++ |
| **Next / Prev Screen RAM** *(0x0000, 0x0400, ...)* | ++s++ / ++shift+s++ |
| **Screen RAM after Bitmap** | ++x++ |
| **Jump to Disassembly + update cursor** | ++enter++ |

### Blocks

| Action | Shortcut |
| :--- | :--- |
| **Toggle Collapsed Block** | ++ctrl+k++ |
| **Jump to Disassembly + update cursor** | ++enter++ |

---

## Live Debugger (VICE)

| Action | Shortcut |
| :--- | :--- |
| **Toggle Breakpoint** | ++f2++ |
| **Toggle Breakpoint...** | ++shift+f2++ |
| **Watchpoint** | ++f6++ |
| **Memory Dump...** | ++m++ |
| **Run to Cursor** | ++f4++ |
| **Step Instruction** | ++f7++ |
| **Step Over** | ++f8++ |
| **Step Out** | ++shift+f8++ |
| **Run / Pause (Continue)** | ++f9++ |

---

## Search & Selection

| Action | Shortcut |
| :--- | :--- |
| **Vim Search** | ++slash++ |
| **Next / Previous Match** | ++n++ / ++shift+n++ |
| **Search Dialog** | ++ctrl+f++ |
| **Find Next / Previous** | ++f3++ / ++shift+f3++ |
| **Go to symbol** | ++ctrl+p++ |
| **Go to Cross References** | ++ctrl+x++ |
| **Toggle Visual Mode** | ++shift+v++ |
| **Select Text** | ++shift+up++ / ++down++ / Visual Mode + ++j++ / ++k++ |
| **Clear Selection** | ++escape++ |
| **Drag to Select** | Mouse Drag |

---

## Menus

| Action | Shortcut |
| :--- | :--- |
| **Navigate Menu** | ++arrow-up++, ++arrow-down++, ++arrow-left++, ++arrow-right++ |
| **Select Item** | ++enter++ |
| **Close Menu** | ++escape++ |
| **File Menu** | ++alt+f++ |
| **Edit Menu** | ++alt+t++ |
| **Jump Menu** | ++alt+j++ |
| **Search Menu** | ++alt+r++ |
| **View Menu** | ++alt+v++ |
| **Debugger Menu** | ++alt+u++ |
| **Help Menu** | ++alt+h++ |

---

## Jump to Operand Picker

Press ++enter++ on an **Address**, **Lo/Hi Address**, or **Hi/Lo Address** line with two or more unique targets to open the **Jump to Operand** picker (single-target lines jump immediately). See [Views — Jump to Operand](views.md#jump-to-operand) for details.

| Action | Control |
| :--- | :--- |
| **Move Selection** | ++up++ / ++down++ / ++j++ / ++k++, or Mouse Scroll |
| **Page Up / Page Down** | ++page-up++ / ++page-down++ |
| **First / Last Target** | ++home++ / ++end++ |
| **Quick Jump (Targets 1–9)** | `1`–`9` |
| **Jump to Selected Target** | ++enter++ or Left-Click Row |
| **Cancel / Close** | ++escape++, `[x]` Button, or Left-Click Outside Dialog |
