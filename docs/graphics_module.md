# Graphics Module ([`ui/graphics_common.rs`](https://github.com/ricardoquesada/regenerator2000/blob/main/src/ui/graphics_common.rs))

## Overview

The `graphics_common` module consolidates shared graphics functionality used across C64 rendering components (sprites, charset, and bitmaps). This reduces code duplication and provides a single source of truth for VIC-II color handling.

## Features

### VIC-II Color Palette

The module exports the standard C64 16-color palette as `VIC_II_RGB`, which can be used across all graphics views:

```rust
use crate::ui::graphics_common::VIC_II_RGB;

let white = VIC_II_RGB[1]; // [255, 255, 255]
let black = VIC_II_RGB[0]; // [0, 0, 0]
```

Helper functions are provided for safer access:
- `get_vic_color(index: u8) -> [u8; 3]` - Returns RGB array
- `get_vic_color_rgb(index: u8) -> Rgb<u8>` - Returns image::Rgb type

### Pixel Decoding Helpers

Common pixel extraction functions:

- `extract_multicolor_pixels(byte: u8) -> [u8; 4]` - Extract 4 two-bit pixels (multicolor mode)
- `extract_hires_pixels(byte: u8) -> [u8; 8]` - Extract 8 single-bit pixels (hi-res mode)
- `decode_multicolor_pixel(...)` - Decode 2-bit value to color index using color byte

### Image Generation

Functions to render C64 graphics data as RGB images (useful for export/preview):

- `render_sprite_to_image(...)` - Render a 24×21 sprite to PNG
- `render_char_to_image(...)` - Render an 8×8 character to PNG

Both support multicolor and single-color modes with configurable scaling.

## Current Usage

### Bitmap View
The bitmap view uses the VIC-II palette for generating full-screen images (320×200 or 160×200):

```rust
use crate::ui::graphics_common::VIC_II_RGB;

let color = VIC_II_RGB[(val >> 4) as usize];
rgb_img.put_pixel(x, y, Rgb(color));
```

### Future Usage

The sprite and charset views currently render using terminal characters (█, dots) with theme colors. The graphics_common module provides the foundation for future enhancements:

1. **PNG Export** - Export sprites/charsets as PNG files using `render_sprite_to_image()` and `render_char_to_image()`
2. **Image Preview** - Optionally show actual pixel-perfect previews in addition to ASCII art
3. **Color Palette Editing** - Support for custom color palettes
4. **SID View** - Common infrastructure for new visualization types

## Benefits

1. **Single Source of Truth** - VIC-II palette defined once
2. **Type Safety** - Helper functions prevent out-of-bounds access
3. **Testability** - Pixel extraction logic has unit tests
4. **Extensibility** - Easy to add new rendering functions
5. **Consistency** - All views use the same color values

## Testing

The module includes unit tests for all helper functions:

```bash
cargo test ui::graphics_common
```

Tests cover:
- Color palette access (including out-of-bounds)
- Multicolor pixel extraction
- Hi-res pixel extraction
- Multicolor pixel decoding

## Code Organization

Before refactoring:
- [`view_bitmap.rs`](https://github.com/ricardoquesada/regenerator2000/blob/main/src/ui/view_bitmap.rs): 457 lines (including VIC-II palette definition)

After refactoring:
- [`view_bitmap.rs`](https://github.com/ricardoquesada/regenerator2000/blob/main/src/ui/view_bitmap.rs): 437 lines (uses shared palette)
- [`graphics_common.rs`](https://github.com/ricardoquesada/regenerator2000/blob/main/src/ui/graphics_common.rs): 342 lines (shared utilities + tests)
- **Net benefit**: Reusable code for sprites/charset + foundation for PNG export

## Future Enhancements

Potential additions to this module:

1. **Color RAM Support** - Helpers for C64 color RAM ($D800-$DBFF)
2. **Color Palette Variations** - Support for different palette interpretations
3. **Bitmap Rendering Helper** - Consolidate bitmap rendering logic
4. **Anti-aliasing** - Optional smoothing for scaled exports
5. **Format Converters** - C64 format ↔ modern image formats
