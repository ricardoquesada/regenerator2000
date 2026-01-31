# Keyboard Shortcuts

Some actions can be triggered with more than one keyboard combination. This is intentional to ensure compatibility
across Windows, macOS, and Linux, and different terminal emulators.

| Context                            | Action                                                                                  | Shortcut                                                      |
| :--------------------------------- | :-------------------------------------------------------------------------------------- | :------------------------------------------------------------ |
| **Global**                         | **Activate Menu**                                                                       | ++f10++                                                       |
|                                    | **Exit**                                                                                | ++ctrl+q++                                                    |
|                                    | **Open File**                                                                           | ++ctrl+o++                                                    |
|                                    | **Save Project**                                                                        | ++ctrl+s++                                                    |
|                                    | **Save Project As**                                                                     | ++alt+s++ or ++ctrl+shift+s++                                 |
|                                    | **Export Project (ASM)**                                                                | ++ctrl+e++                                                    |
|                                    | **Export Project As (ASM)**                                                             | ++alt+e++ or ++ctrl+shift+e++                                 |
|                                    | **Document Settings**                                                                   | ++alt+d++ or ++ctrl+shift+d++                                 |
|                                    | **Settings**                                                                            | ++alt+o++ or ++ctrl+comma++                                   |
|                                    | **Undo**                                                                                | ++u++                                                         |
|                                    | **Redo**                                                                                | ++ctrl+r++                                                    |
|                                    | **Switch Pane (betweeen Disasm and right pane)**                                        | ++tab++                                                       |
| **Views**                          | **Toggle Hex Dump View**                                                                | ++alt+2++ or ++ctrl+2++                                       |
|                                    | **Toggle Sprites View**                                                                 | ++alt+3++ or ++ctrl+3++                                       |
|                                    | **Toggle Charset View**                                                                 | ++alt+4++ or ++ctrl+4++                                       |
|                                    | **Toggle Blocks View**                                                                  | ++alt+5++ or ++ctrl+5++                                       |
| **View: Disassembly** (editing)    | **Convert to Code**                                                                     | ++c++                                                         |
|                                    | **Convert to Byte**                                                                     | ++b++                                                         |
|                                    | **Convert to Word**                                                                     | ++w++                                                         |
|                                    | **Convert to Address**                                                                  | ++a++                                                         |
|                                    | **Set Lo/Hi Word Table**                                                                | ++t++                                                         |
|                                    | **Set Hi/Lo Word Table**                                                                | ++shift+t++                                                   |
|                                    | **Set Lo/Hi Address Table**                                                             | ++less-than++                                                 |
|                                    | **Set Hi/Lo Address Table**                                                             | ++greater-than++                                              |
|                                    | **Convert to PETSCII Text**                                                             | ++p++                                                         |
|                                    | **Convert to Screencode Text**                                                          | ++s++                                                         |
|                                    | **Convert to Undefined**                                                                | ++question-mark++                                             |
|                                    | **Next/Prev Immediate Mode Format** (Hex, Decimal, Binary)                              | ++d++ / ++shift+d++                                           |
|                                    | **Set Label**                                                                           | ++l++                                                         |
|                                    | **Add Side Comment**                                                                    | ++semicolon++                                                 |
|                                    | **Add Line Comment**                                                                    | ++colon++                                                     |
|                                    | **Toggle Collapsed Block**                                                              | ++ctrl+k++                                                    |
|                                    | **Toggle Splitter**                                                                     | ++pipe++                                                      |
|                                    | **Analyze**                                                                             | ++ctrl+a++                                                    |
| **View: Disassembly** (navigation) | **Move Cursor**                                                                         | ++up++ / ++down++ / ++j++ / ++k++                             |
|                                    | **Page Up/Down**                                                                        | ++page-up++ / ++page-down++                                   |
|                                    | **Home/End**                                                                            | ++home++ / ++end++                                            |
|                                    | **Jump to Address (Dialog)**                                                            | ++g++                                                         |
|                                    | **Jump to Line (Dialog)**                                                               | ++alt+g++, ++ctrl+shift+g++                                   |
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
| **Menus**                          | **Navigate Menu**                                                                       | ++arrow-up++, ++arrow-down++, ++arrow-left++, ++arrow-right++ |
|                                    | **Select Item**                                                                         | ++enter++                                                     |
|                                    | **Close Menu**                                                                          | ++escape++                                                    |
| **Search**                         | **Vim Search**                                                                          | ++slash++                                                     |
|                                    | **Next / Previous Match**                                                               | ++n++ / ++shift+n++                                           |
|                                    | **Search Dialog**                                                                       | ++ctrl+f++                                                    |
|                                    | **Find Next / Previous**                                                                | ++f3++ / ++shift+f3++                                         |
|                                    | **Go to symbol**                                                                        | ++ctrl+p++                                                    |
|                                    | **Find Cross References**                                                               | ++ctrl+x++                                                    |
| **Selection**                      | **Toggle Visual Mode**                                                                  | ++shift+v++                                                   |
|                                    | **Select Text**                                                                         | ++shift+up++ / ++down++ / Visual Mode + ++j++ / ++k++         |
|                                    | **Clear Selection**                                                                     | ++escape++                                                    |
