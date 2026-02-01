/// Common graphics utilities for C64 rendering (sprites, charset, bitmaps)
///
/// This module provides shared functionality for rendering C64 graphics data including:
/// - VIC-II color palette
/// - Color lookup helpers
/// - Pixel decoding for multicolor and single-color modes
use image::{DynamicImage, Rgb, RgbImage};

/// VIC-II RGB Palette (16 colors)
///
/// This is the standard C64 color palette used for generating images from bitmap,
/// sprite, and charset data. Each color is represented as [R, G, B] in 0-255 range.
pub const VIC_II_RGB: [[u8; 3]; 16] = [
    [0, 0, 0],       // 0: Black
    [255, 255, 255], // 1: White
    [136, 0, 0],     // 2: Red
    [170, 255, 238], // 3: Cyan
    [204, 68, 204],  // 4: Purple
    [0, 204, 85],    // 5: Green
    [0, 0, 170],     // 6: Blue
    [238, 238, 119], // 7: Yellow
    [221, 136, 85],  // 8: Orange
    [102, 68, 0],    // 9: Brown
    [255, 119, 119], // 10: Light Red
    [51, 51, 51],    // 11: Dark Grey
    [119, 119, 119], // 12: Grey
    [170, 255, 102], // 13: Light Green
    [0, 136, 255],   // 14: Light Blue
    [187, 187, 187], // 15: Light Grey
];

/// Get RGB color from VIC-II palette by index
///
/// # Arguments
/// * `index` - Color index (0-15)
///
/// # Returns
/// RGB color as [u8; 3], or black if index is out of range
#[inline]
pub fn get_vic_color(index: u8) -> [u8; 3] {
    VIC_II_RGB.get(index as usize).copied().unwrap_or([0, 0, 0])
}

/// Get RGB color from VIC-II palette as image::Rgb
///
/// # Arguments
/// * `index` - Color index (0-15)
///
/// # Returns
/// RGB color as image::Rgb<u8>
#[inline]
pub fn get_vic_color_rgb(index: u8) -> Rgb<u8> {
    Rgb(get_vic_color(index))
}

/// Decode a 2-bit multicolor pixel value to color index
///
/// In multicolor mode, each 2-bit value maps to one of 4 colors:
/// - 00: Background color (typically color 0)
/// - 01: Color 1 (upper nibble of color byte)
/// - 10: Color 2 (lower nibble of color byte)
/// - 11: Color 3 (typically color 1, white)
///
/// # Arguments
/// * `bits` - 2-bit pixel value (0-3)
/// * `color_byte` - Color byte containing upper/lower nibble colors
/// * `bg_color` - Background color index (for 00)
/// * `fg_color` - Foreground color index (for 11)
///
/// # Returns
/// Color index (0-15)
#[inline]
pub fn decode_multicolor_pixel(bits: u8, color_byte: u8, bg_color: u8, fg_color: u8) -> u8 {
    match bits & 0b11 {
        0b00 => bg_color,
        0b01 => (color_byte >> 4) & 0x0F,
        0b10 => color_byte & 0x0F,
        0b11 => fg_color,
        _ => unreachable!(),
    }
}

/// Extract 4 two-bit pixel values from a byte (for multicolor mode)
///
/// Returns pixel values in left-to-right order (MSB to LSB)
///
/// # Arguments
/// * `byte` - Input byte containing 4 two-bit pixels
///
/// # Returns
/// Array of 4 pixel values [leftmost, ..., rightmost]
#[inline]
pub fn extract_multicolor_pixels(byte: u8) -> [u8; 4] {
    [
        (byte >> 6) & 0b11,
        (byte >> 4) & 0b11,
        (byte >> 2) & 0b11,
        byte & 0b11,
    ]
}

/// Extract 8 single-bit pixel values from a byte
///
/// Returns pixel values in left-to-right order (MSB to LSB)
///
/// # Arguments
/// * `byte` - Input byte containing 8 single-bit pixels
///
/// # Returns
/// Array of 8 pixel values (0 or 1)
#[inline]
pub fn extract_hires_pixels(byte: u8) -> [u8; 8] {
    [
        (byte >> 7) & 1,
        (byte >> 6) & 1,
        (byte >> 5) & 1,
        (byte >> 4) & 1,
        (byte >> 3) & 1,
        (byte >> 2) & 1,
        (byte >> 1) & 1,
        byte & 1,
    ]
}

/// Render a C64 sprite (24x21 pixels) to an RGB image
///
/// # Arguments
/// * `sprite_data` - 63 bytes of sprite data
/// * `multicolor` - Whether to use multicolor mode
/// * `color1` - Sprite color (or upper nibble color in MC mode)
/// * `color2` - Multicolor 1 (only used in MC mode)
/// * `color3` - Multicolor 2 (only used in MC mode)
/// * `scale` - Scaling factor (1 = 24x21, 2 = 48x42, etc.)
///
/// # Returns
/// DynamicImage containing the rendered sprite
pub fn render_sprite_to_image(
    sprite_data: &[u8],
    multicolor: bool,
    color1: u8,
    color2: u8,
    color3: u8,
    scale: u32,
) -> DynamicImage {
    let (width, height) = (24, 21);

    let mut img = RgbImage::new(width * scale, height * scale);

    for row in 0..21 {
        let row_start = row * 3;
        if row_start + 2 >= sprite_data.len() {
            break;
        }

        let bytes = &sprite_data[row_start..row_start + 3];

        if multicolor {
            // Multicolor: 12 fat pixels (2x1 C64 pixels each)
            let mut x = 0;
            for &byte in bytes {
                for bits in extract_multicolor_pixels(byte) {
                    let color = match bits {
                        0b00 => [0, 0, 0], // Transparent (black)
                        0b01 => get_vic_color(color1),
                        0b10 => get_vic_color(color2),
                        0b11 => get_vic_color(color3),
                        _ => unreachable!(),
                    };

                    // Draw fat pixel (2 pixels wide)
                    if bits != 0b00 {
                        for dy in 0..scale {
                            for dx in 0..scale * 2 {
                                img.put_pixel(
                                    x * scale * 2 + dx,
                                    (row as u32) * scale + dy,
                                    Rgb(color),
                                );
                            }
                        }
                    }
                    x += 1;
                }
            }
        } else {
            // Single color: 24 pixels
            let mut x = 0;
            for &byte in bytes {
                for bit in extract_hires_pixels(byte) {
                    if bit == 1 {
                        let color = get_vic_color(color1);
                        for dy in 0..scale {
                            for dx in 0..scale {
                                img.put_pixel(
                                    x * scale + dx,
                                    (row as u32) * scale + dy,
                                    Rgb(color),
                                );
                            }
                        }
                    }
                    x += 1;
                }
            }
        }
    }

    DynamicImage::ImageRgb8(img)
}

/// Render a C64 character (8x8 pixels) to an RGB image
///
/// # Arguments
/// * `char_data` - 8 bytes of character data
/// * `multicolor` - Whether to use multicolor mode
/// * `fg_color` - Foreground color
/// * `bg_color` - Background color
/// * `mc1_color` - Multicolor 1 (only used in MC mode)
/// * `mc2_color` - Multicolor 2 (only used in MC mode)
/// * `scale` - Scaling factor (1 = 8x8, 2 = 16x16, etc.)
///
/// # Returns
/// DynamicImage containing the rendered character
pub fn render_char_to_image(
    char_data: &[u8],
    multicolor: bool,
    fg_color: u8,
    bg_color: u8,
    mc1_color: u8,
    mc2_color: u8,
    scale: u32,
) -> DynamicImage {
    let (width, height) = (8, 8);

    let mut img = RgbImage::new(width * scale, height * scale);

    for (row, &byte) in char_data.iter().enumerate().take(8) {
        if multicolor {
            // Multicolor: 4 fat pixels
            for (x, bits) in extract_multicolor_pixels(byte).iter().enumerate() {
                let color_idx = match bits {
                    0b00 => bg_color,
                    0b01 => fg_color,
                    0b10 => mc1_color,
                    0b11 => mc2_color,
                    _ => unreachable!(),
                };
                let color = get_vic_color(color_idx);

                // Draw fat pixel (2 pixels wide)
                for dy in 0..scale {
                    for dx in 0..scale * 2 {
                        img.put_pixel(
                            x as u32 * scale * 2 + dx,
                            row as u32 * scale + dy,
                            Rgb(color),
                        );
                    }
                }
            }
        } else {
            // Single color: 8 pixels
            for (x, bit) in extract_hires_pixels(byte).iter().enumerate() {
                let color_idx = if *bit == 1 { fg_color } else { bg_color };
                let color = get_vic_color(color_idx);

                for dy in 0..scale {
                    for dx in 0..scale {
                        img.put_pixel(x as u32 * scale + dx, row as u32 * scale + dy, Rgb(color));
                    }
                }
            }
        }
    }

    DynamicImage::ImageRgb8(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_vic_color() {
        assert_eq!(get_vic_color(0), [0, 0, 0]); // Black
        assert_eq!(get_vic_color(1), [255, 255, 255]); // White
        assert_eq!(get_vic_color(16), [0, 0, 0]); // Out of range -> black
    }

    #[test]
    fn test_extract_multicolor_pixels() {
        assert_eq!(extract_multicolor_pixels(0b11_10_01_00), [3, 2, 1, 0]);
        assert_eq!(extract_multicolor_pixels(0b00_00_00_00), [0, 0, 0, 0]);
        assert_eq!(extract_multicolor_pixels(0b11_11_11_11), [3, 3, 3, 3]);
    }

    #[test]
    fn test_extract_hires_pixels() {
        assert_eq!(extract_hires_pixels(0b10101010), [1, 0, 1, 0, 1, 0, 1, 0]);
        assert_eq!(extract_hires_pixels(0b11110000), [1, 1, 1, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn test_decode_multicolor_pixel() {
        assert_eq!(decode_multicolor_pixel(0b00, 0x12, 0, 1), 0); // bg
        assert_eq!(decode_multicolor_pixel(0b01, 0x12, 0, 1), 1); // upper nibble
        assert_eq!(decode_multicolor_pixel(0b10, 0x12, 0, 1), 2); // lower nibble
        assert_eq!(decode_multicolor_pixel(0b11, 0x12, 0, 1), 1); // fg
    }
}
