/// Common graphics utilities for C64 rendering (sprites, charset, bitmaps).
///
/// Provides the shared VIC-II 16-color RGB palette used by bitmap rendering.
///
/// Each color is represented as `[R, G, B]` in the `0..=255` range.
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
