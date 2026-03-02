# Keyboard Shortcuts

Regenerator 2000 is designed to be a keyboard-centric application. While it features mouse support—which is handy for scrolling through different views—you'll find that the keyboard shortcuts are often a more efficient way to navigate and interact with the program.

The shortcut scheme follows a simple logic:

- **Block Type Operations:** These are usually a single letter or `Shift` + letter (e.g., `c` for Code, `b` for Byte).
- **Dialogs & System Actions:** Operations that open a dialog or perform system-level tasks typically use `Ctrl` + letter or `Alt` + letter.

We've tested these shortcuts extensively across Windows, macOS, and Linux, and in various terminal environments (including inside `tmux`). Wherever a standard shortcut might conflict with a reserved key combination (like `Ctrl+2` in `tmux`), we've provided alternatives (e.g., `Alt+2`).

| Context                            | Action                                                                                  | Shortcut                                                      |
| :--------------------------------- | :-------------------------------------------------------------------------------------- | :------------------------------------------------------------ |
| **General**                        | **Activate Main Menu**                                                                  | ++f10++                                                       |
|                                    | **Quit**                                                                                | ++ctrl+q++                                                    |
| **General**                        | **Open File**                                                                           | ++ctrl+o++                                                    |
|                                    | **Save Project**                                                                        | ++ctrl+s++                                                    |
|                                    | **Save Project As**                                                                     | ++alt+s++ or ++ctrl+shift+s++                                 |
|                                    | **Export Project (ASM)**                                                                | ++ctrl+e++                                                    |
|                                    | **Export Project As (ASM)**                                                             | ++alt+e++ or ++ctrl+shift+e++                                 |
|                                    | **Document Settings**                                                                   | ++alt+d++ or ++ctrl+shift+d++                                 |
|                                    | **Settings**                                                                            | ++alt+p++ or ++ctrl+comma++                                   |
|                                    | **Open Recent Projects**                                                                | ++alt+o++ or ++ctrl+shift+o++                                 |
|                                    | **Undo**                                                                                | ++u++                                                         |
|                                    | **Redo**                                                                                | ++ctrl+r++                                                    |
|                                    | **Switch Pane (between Disasm and right pane)**                                         | ++tab++                                                       |
| **Views**                          | **Toggle Blocks View**                                                                  | ++alt+1++ or ++ctrl+1++                                       |
|                                    | **Toggle Hex Dump View**                                                                | ++alt+2++ or ++ctrl+2++                                       |
|                                    | **Toggle Sprites View**                                                                 | ++alt+3++ or ++ctrl+3++                                       |
|                                    | **Toggle Charset View**                                                                 | ++alt+4++ or ++ctrl+4++                                       |
|                                    | **Toggle Bitmap View**                                                                  | ++alt+5++ or ++ctrl+5++                                       |
|                                    | **Toggle Debugger Panel**                                                               | ++alt+6++ or ++ctrl+6++                                       |
| **View: Disassembly** (editing)    | **Convert to Code**                                                                     | ++c++                                                         |
|                                    | **Convert to Byte**                                                                     | ++b++                                                         |
|                                    | **Convert to Word**                                                                     | ++w++                                                         |
|                                    | **Convert to Address**                                                                  | ++a++                                                         |
|                                    | **Convert to PETSCII Text**                                                             | ++p++                                                         |
|                                    | **Convert to Screencode Text**                                                          | ++s++                                                         |
|                                    | **Next/Prev Immediate Mode Format** (Hex, Decimal, Binary)                              | ++d++ / ++shift+d++                                           |
|                                    | **Pack Lo/Hi Address (Immediate Mode)**                                                 | ++open-bracket++                                              |
|                                    | **Pack Hi/Lo Address (Immediate Mode)**                                                 | ++close-bracket++                                             |
|                                    | **Set Lo/Hi Word Table**                                                                | ++comma++                                                     |
|                                    | **Set Hi/Lo Word Table**                                                                | ++period++                                                    |
|                                    | **Set Lo/Hi Address Table**                                                             | ++less-than++                                                 |
|                                    | **Set Hi/Lo Address Table**                                                             | ++greater-than++                                              |
|                                    | **Convert to External File**                                                            | ++e++                                                         |
|                                    | **Convert to Undefined**                                                                | ++question-mark++                                             |
|                                    | **Set Label**                                                                           | ++l++                                                         |
|                                    | **Toggle Bookmark**                                                                     | ++ctrl+b++                                                    |
|                                    | **List Bookmarks**                                                                      | ++ctrl+shift+b++ or ++alt+b++                                 |
|                                    | **Add Side Comment**                                                                    | ++semicolon++                                                 |
|                                    | **Add Line Comment**                                                                    | ++colon++                                                     |
|                                    | **Toggle Collapsed Block**                                                              | ++ctrl+k++                                                    |
|                                    | **Toggle Splitter**                                                                     | ++pipe++                                                      |
|                                    | **Analyze**                                                                             | ++ctrl+a++                                                    |
| **View: Disassembly** (navigation) | **Move Cursor**                                                                         | ++up++ / ++down++ / ++j++ / ++k++                             |
|                                    | **Page Up/Down**                                                                        | ++page-up++ / ++page-down++                                   |
|                                    | **Home/End**                                                                            | ++home++ / ++end++                                            |
|                                    | **Jump to Address**                                                                     | ++ctrl+g++ or ++alt+g++                                       |
|                                    | **Jump to Line**                                                                        | ++ctrl+shift+g++ or ++alt+shift+g++                           |
|                                    | **Jump to Line / End of File**                                                          | number + ++shift+g++                                          |
|                                    | **Jump to Operand**                                                                     | ++enter++                                                     |
|                                    | **Jump Back (History)**                                                                 | ++backspace++                                                 |
|                                    | **Previous/Next 10 Lines**                                                              | ++ctrl+u++ / ++ctrl+d++                                       |
| **View: Hexdump**                  | **Convert to Byte**                                                                     | ++b++                                                         |
|                                    | **Next / Prev Hex Text Mode** (Screencode shifted/unshifted, PETSCII shifted/unshifted) | ++m++ / ++shift+m++                                           |
|                                    | **Jump to Disassembly + update cursor**                                                 | ++enter++                                                     |
| **View: Sprites**                  | **Convert to Byte**                                                                     | ++b++                                                         |
|                                    | **Toggle Multicolor Sprites**                                                           | ++m++                                                         |
|                                    | **Jump to Disassembly + update cursor**                                                 | ++enter++                                                     |
| **View: Charset**                  | **Convert to Byte**                                                                     | ++b++                                                         |
|                                    | **Toggle Multicolor Charset**                                                           | ++m++                                                         |
|                                    | **Jump to Disassembly + update cursor**                                                 | ++enter++                                                     |
| **View: Bitmap**                   | **Convert to Byte**                                                                     | ++b++                                                         |
|                                    | **Toggle Multicolor Bitmap**                                                            | ++m++                                                         |
|                                    | **Next / Prev Screen RAM** (0x0000,0x0400, ...)                                         | ++s++ / ++shift+s++                                           |
|                                    | **Screen RAM after Bitmap**                                                             | ++x++                                                         |
|                                    | **Jump to Disassembly + update cursor**                                                 | ++enter++                                                     |
| **View: Blocks**                   | **Toggle Collapsed Block**                                                              | ++ctrl+k++                                                    |
|                                    | **Jump to Disassembly + update cursor**                                                 | ++enter++                                                     |
| **Debugger**                       | **Toggle Breakpoint**                                                                   | ++f2++                                                        |
|                                    | **Toggle Breakpoint...**                                                                | ++shift+f2++                                                  |
|                                    | **Watchpoint**                                                                          | ++f6++                                                        |
|                                    | **Run to Cursor**                                                                       | ++f4++                                                        |
|                                    | **Step Instruction**                                                                    | ++f7++                                                        |
|                                    | **Step Over**                                                                           | ++f8++                                                        |
|                                    | **Step Out**                                                                            | ++shift+f8++                                                  |
|                                    | **Run / Pause (Continue)**                                                              | ++f9++                                                        |
| **Menus**                          | **Navigate Menu**                                                                       | ++arrow-up++, ++arrow-down++, ++arrow-left++, ++arrow-right++ |
|                                    | **Select Item**                                                                         | ++enter++                                                     |
|                                    | **Close Menu**                                                                          | ++escape++                                                    |
| **Search**                         | **Vim Search**                                                                          | ++slash++                                                     |
|                                    | **Next / Previous Match**                                                               | ++n++ / ++shift+n++                                           |
|                                    | **Search Dialog**                                                                       | ++ctrl+f++                                                    |
|                                    | **Find Next / Previous**                                                                | ++f3++ / ++shift+f3++                                         |
|                                    | **Go to symbol**                                                                        | ++ctrl+p++                                                    |
|                                    | **Go to Cross References**                                                              | ++ctrl+x++                                                    |
| **Selection**                      | **Toggle Visual Mode**                                                                  | ++shift+v++                                                   |
|                                    | **Select Text**                                                                         | ++shift+up++ / ++down++ / Visual Mode + ++j++ / ++k++         |
|                                    | **Clear Selection**                                                                     | ++escape++                                                    |
