# Views

Regenerator 2000 offers several specialized views to efficiently analyze and reverse engineer your C64 binaries.

## Disassembly View

The Disassembly View is the central workspace of Regenerator 2000. It shows the disassembled code, data, and text. Here you can:

- Navigate through the memory.
- Define block types (Code, Byte, Word, etc.).
- Add comments and rename labels.
- Follow code execution with jump/branch arrows.

## Hexdump View

The Hexdump View provides a raw hexadecimal representation of the memory. It is useful for inspecting data that hasn't been formatted yet or for verifying the exact byte values in a region.

![Hexdump View](regenerator2000_hexdump_screenshot.png)

## Blocks View

The Blocks View visualizes the memory layout of your project. It helps you quickly identify:

- **Code regions** (Blue)
- **Data regions** (Green/Yellow)
- **Undefined/Unknown regions** (Grey)

This bird's-eye view is essential for understanding the overall structure of the binary and spotting large chunks of unanalyzed data.

![Blocks View](regenerator2000_blocks_screenshot.png)

## Charset View

The Charset View allows you to inspect memory as if it were a C64 character set (font). This is crucial for verifying if a memory region contains custom fonts.

- Supports Standard (Multi-color) and Hi-Res modes.
- Useful for spotting graphical data masquerading as code.

![Charset View](regenerator2000_charset_screenshot.png)

## Sprites View

The Sprites View helps you find and analyze sprite data.

- Displays memory in 64-byte chunks formatted as C64 sprites (24x21 pixels).
- Helps identifying player characters, enemies, and other game objects.

![Sprites View](regenerator2000_sprites_screenshot.png)

## Bitmap View

The Bitmap View renders memory as a bitmap image.

- Useful for finding splash screens, background graphics, or other large graphical assets.
- Can help identify the format of unknown large data blocks.

![Bitmap View](regenerator2000_bitmap_screenshot.png)
