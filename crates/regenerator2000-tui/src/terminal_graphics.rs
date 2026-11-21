//! Terminal graphics protocol probing, theme background compositing, and layout helpers.
//!
//! This module separates deterministic [`Halfblocks`](ProtocolType::Halfblocks) picker
//! creation from interactive stdio probing. Headless [`UIState::new`](crate::ui_state::UIState::new)
//! callers and tests use [`create_headless_picker`] with zero stdio I/O. The binary startup
//! path calls [`probe_terminal_picker`] once to detect `Kitty`, `Iterm2`, `Sixel`, or
//! `Halfblocks` capabilities and true cell pixel metrics.

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba, imageops};
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;
use ratatui_image::picker::Capability;
use ratatui_image::picker::cap_parser::{Parser, QueryStdioOptions};
pub use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::kitty::StatefulKitty;
use ratatui_image::protocol::{StatefulProtocol, StatefulProtocolType};
use ratatui_image::{FilterType, FontSize, Resize, ResizeEncodeRender};
use std::cmp::min;
use std::env;
use std::fmt::{self, Write as _};
use std::io::{self, Cursor, IsTerminal};
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Timeout for terminal stdio capability probing (`_Gi=31`, `CSI c`, `CSI 16 t`, `CSI 5 n`).
///
/// Interactive graphics terminals respond within 1 to 3 ms. A 500 ms timeout caps
/// worst-case startup delay on unresponsive PTYs or slow remote links.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// Creates a deterministic [`Picker::halfblocks`] instance with zero stdio I/O.
///
/// [`UIState::new`](crate::ui_state::UIState::new) and headless `TestBackend` tests use this
/// picker so tests never emit escape sequences or block on `stdin`.
#[must_use]
pub fn create_headless_picker() -> Picker {
    Picker::halfblocks()
}

/// Terminal `TERM_PROGRAM` substrings accepted as native iTerm2-protocol terminals.
///
/// Excludes `WarpTerminal` (broken clearing) and `vscode` (inline images disabled by default).
const ITERM2_TERM_PROGRAMS: [&str; 5] = ["iTerm", "WezTerm", "mintty", "rio", "Bobcat"];

/// Terminal `TERM_PROGRAM` substrings explicitly excluded from upstream iTerm2 env fallbacks.
const BLOCKED_ITERM2_TERM_PROGRAMS: [&str; 4] = ["vscode", "WarpTerminal", "Tabby", "Hyper"];

/// Returns `true` if session hints positively identify an approved iTerm2-protocol terminal.
///
/// Rejects `vscode`, `WarpTerminal`, `Tabby`, and `Hyper` even when inherited `ITERM_SESSION_ID`
/// or `WEZTERM_EXECUTABLE` env vars are present, while permitting approved terminals inside
/// `tmux`.
#[must_use]
fn is_approved_iterm2_env(
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
    has_wezterm_executable: bool,
    has_iterm_session_id: bool,
) -> bool {
    if let Some(tp) = term_program
        && BLOCKED_ITERM2_TERM_PROGRAMS
            .iter()
            .any(|blocked| tp.contains(blocked))
    {
        return false;
    }

    if has_wezterm_executable || has_iterm_session_id {
        return true;
    }

    if let Some(tp) = term_program
        && ITERM2_TERM_PROGRAMS
            .iter()
            .any(|needle| tp.contains(needle))
    {
        return true;
    }

    lc_terminal.is_some_and(|lc| lc.contains("iTerm"))
}

/// Returns `true` if environment hints outside `tmux` identify a native iTerm2-protocol terminal.
///
/// Terminals such as iTerm2 and Rio advertise Sixel in `DA1`, which otherwise shadows
/// 24-bit `OSC 1337` graphics during stdio probing.
#[must_use]
fn prefers_iterm2_over_sixel(
    term: Option<&str>,
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
) -> bool {
    // Inside tmux, outer `TERM_PROGRAM` and `LC_TERMINAL` values can be stale after detach
    // and reattach across different terminals. Let active stdio probing decide inside tmux.
    let is_tmux = term.is_some_and(|t| t.starts_with("tmux")) || term_program == Some("tmux");
    !is_tmux && is_approved_iterm2_env(term_program, lc_terminal, false, false)
}

/// Downgrades `Iterm2` detected solely via unapproved upstream env fallbacks to `Halfblocks`.
///
/// `ratatui-image` stops `Kitty` and `Sixel` probing when blacklisted, then falls back on
/// `TERM_PROGRAM` (`vscode`, `WarpTerminal`, `Tabby`, `Hyper`), `ITERM_SESSION_ID`,
/// `WEZTERM_EXECUTABLE`, or `LC_TERMINAL`. Blocking only unapproved terminals preserves `Iterm2`
/// for approved terminals such as `WezTerm` or `iTerm2` running inside `tmux`.
#[must_use]
fn sanitize_probed_protocol(
    probed: ProtocolType,
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
    has_wezterm_executable: bool,
    has_iterm_session_id: bool,
) -> ProtocolType {
    if probed == ProtocolType::Iterm2
        && !is_approved_iterm2_env(
            term_program,
            lc_terminal,
            has_wezterm_executable,
            has_iterm_session_id,
        )
    {
        ProtocolType::Halfblocks
    } else {
        probed
    }
}

/// Builds [`QueryStdioOptions`] for the current terminal emulator environment.
///
/// For native iTerm2 terminals (`iTerm2`, `WezTerm`, `Rio`, `mintty`, `Bobcat`) outside
/// `tmux`, this function blacklists [`ProtocolType::Kitty`] and [`ProtocolType::Sixel`].
/// Blacklisting prevents `DA1` Sixel replies from shadowing 24-bit `OSC 1337` while
/// keeping `CSI 16 t` enabled for Retina cell pixel metrics.
#[must_use]
pub fn build_query_options(
    term: Option<&str>,
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
) -> QueryStdioOptions {
    let blacklist_protocols = if prefers_iterm2_over_sixel(term, term_program, lc_terminal) {
        vec![ProtocolType::Kitty, ProtocolType::Sixel]
    } else {
        Vec::new()
    };

    QueryStdioOptions {
        timeout: PROBE_TIMEOUT,
        text_sizing_protocol: false,
        terminal_background_color_osc: false,
        blacklist_protocols,
        kitty_compression: false,
    }
}

/// Returns `true` if both stdio streams are TTYs and `TERM` is not `"dumb"`.
///
/// `ratatui-image` spawns an uncancelled `stdin` reader thread that blocks until a probe
/// reply arrives. This guard prevents that thread from blocking or racing the TUI input thread.
#[must_use]
fn should_probe_stdio(stdin_is_tty: bool, stdout_is_tty: bool, term: Option<&str>) -> bool {
    stdin_is_tty && stdout_is_tty && term != Some("dumb")
}

/// Returns `true` if `font` has non-zero cell width and height in pixels.
///
/// `ratatui-image`'s `font_size_fallback()` uses integer division that can return `0` on
/// small PTYs. Rejecting zero dimensions prevents division by zero in `Resize::round_pixel_size_to_cells`.
#[must_use]
fn has_valid_font_size(font: FontSize) -> bool {
    font.width > 0 && font.height > 0
}

/// Probes the interactive terminal over stdio for graphics protocol and pixel cell size.
///
/// Call this function after you enable raw mode and enter the alternate screen. Call it
/// before you enable mouse capture, push keyboard enhancement flags, or spawn the input
/// thread. That order prevents SGR mouse escapes or key events from interrupting
/// `ratatui-image`'s capability response parser.
///
/// Returns [`create_headless_picker`] if `stdin` or `stdout` is not a TTY, if `TERM` is
/// `"dumb"`, if probing fails, or if the terminal reports zero-pixel cells.
#[must_use]
pub fn probe_terminal_picker() -> Picker {
    let term = env::var("TERM").ok();
    if !should_probe_stdio(
        io::stdin().is_terminal(),
        io::stdout().is_terminal(),
        term.as_deref(),
    ) {
        return create_headless_picker();
    }

    let term_program = env::var("TERM_PROGRAM").ok();
    let lc_terminal = env::var("LC_TERMINAL").ok();
    let has_wezterm_executable = env::var("WEZTERM_EXECUTABLE").is_ok_and(|s| !s.is_empty());
    let has_iterm_session_id = env::var("ITERM_SESSION_ID").is_ok_and(|s| !s.is_empty());
    let options = build_query_options(
        term.as_deref(),
        term_program.as_deref(),
        lc_terminal.as_deref(),
    );

    match Picker::from_query_stdio_with_options(options) {
        Ok(mut picker) => {
            let font = picker.font_size();
            if !has_valid_font_size(font) {
                log::warn!(
                    "Terminal reported invalid {}x{} px cell size; falling back to Halfblocks",
                    font.width,
                    font.height
                );
                return create_headless_picker();
            }
            let sanitized = sanitize_probed_protocol(
                picker.protocol_type(),
                term_program.as_deref(),
                lc_terminal.as_deref(),
                has_wezterm_executable,
                has_iterm_session_id,
            );
            picker.set_protocol_type(sanitized);
            log::info!(
                "Detected terminal graphics protocol {:?} (cell {}x{} px, tmux={})",
                picker.protocol_type(),
                font.width,
                font.height,
                picker.tmux_detected()
            );
            picker
        }
        Err(err) => {
            log::warn!("Terminal graphics probe failed ({err}); falling back to Halfblocks");
            create_headless_picker()
        }
    }
}

/// Converts a Ratatui [`Color`] to an opaque [`Rgba<u8>`](image::Rgba) background color.
///
/// Passing this color to `ratatui-image` composites transparent pixels before scaling. That
/// prevents `#000000` boxes in `Halfblocks` and dark edge halos in `Kitty`, `Iterm2`, and
/// `Sixel`. Returns `None` for [`Color::Reset`] because the terminal background RGB is
/// unknown.
#[must_use]
pub fn theme_color_to_rgba(color: Color) -> Option<Rgba<u8>> {
    let [r, g, b] = match color {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black => [0, 0, 0],
        Color::Red => [205, 0, 0],
        Color::Green => [0, 205, 0],
        Color::Yellow => [205, 205, 0],
        Color::Blue => [0, 0, 238],
        Color::Magenta => [205, 0, 205],
        Color::Cyan => [0, 205, 205],
        Color::Gray => [229, 229, 229],
        Color::DarkGray => [127, 127, 127],
        Color::LightRed => [255, 0, 0],
        Color::LightGreen => [0, 255, 0],
        Color::LightYellow => [255, 255, 0],
        Color::LightBlue => [92, 92, 255],
        Color::LightMagenta => [255, 0, 255],
        Color::LightCyan => [0, 255, 255],
        Color::White => [255, 255, 255],
        Color::Indexed(idx) => indexed_color_to_rgb(idx),
        Color::Reset => return None,
    };
    Some(Rgba([r, g, b, 255]))
}

/// Maps an xterm-256 palette index (`0..=255`) to its canonical sRGB triplet.
#[must_use]
fn indexed_color_to_rgb(idx: u8) -> [u8; 3] {
    match idx {
        0 => [0, 0, 0],
        1 => [128, 0, 0],
        2 => [0, 128, 0],
        3 => [128, 128, 0],
        4 => [0, 0, 128],
        5 => [128, 0, 128],
        6 => [0, 128, 128],
        7 => [192, 192, 192],
        8 => [128, 128, 128],
        9 => [255, 0, 0],
        10 => [0, 255, 0],
        11 => [255, 255, 0],
        12 => [0, 0, 255],
        13 => [255, 0, 255],
        14 => [0, 255, 255],
        15 => [255, 255, 255],
        16..=231 => {
            let cube = idx - 16;
            let to_level = |c: u8| if c == 0 { 0 } else { 55 + c * 40 };
            [
                to_level((cube / 36) % 6),
                to_level((cube / 6) % 6),
                to_level(cube % 6),
            ]
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            [gray, gray, gray]
        }
    }
}

/// Default padding fill when no opaque background color is specified.
const TRANSPARENT_RGBA: Rgba<u8> = Rgba([0, 0, 0, 0]);

/// Maximum pixel width or height for `2x` `HiDPI` oversampled `OSC 1337` canvases.
///
/// Always oversampling `2x` up to this ceiling keeps zoomed-in Retina iTerm2 fonts sharp
/// without blowing up PNG size on ultra-wide terminals.
const MAX_ITERM2_CANVAS_PX: u32 = 3840;

/// RFC 4648 standard base64 alphabet for `OSC 1337` payloads.
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Next 1-based Kitty image ID pair index (`k` -> even `2*k` and odd `2*k + 1`).
static NEXT_KITTY_ID_PAIR: AtomicU32 = AtomicU32::new(1);

/// Allocates the even base ID of a non-zero 24-bit Kitty double-buffer pair `(id, id ^ 1)`.
#[must_use]
fn allocate_kitty_base_id() -> u32 {
    let pair = NEXT_KITTY_ID_PAIR.fetch_add(1, Ordering::Relaxed);
    let wrapped = (pair % 0x007F_FFFF).saturating_add(1);
    wrapped * 2
}

/// Stateful rendering backend supporting cell-sized `OSC 1337` for `Iterm2`, double-buffered
/// Unicode placeholders for `Kitty`, and `ratatui-image` protocols for `Sixel` and `Halfblocks`.
///
/// `ratatui-image`'s built-in `Iterm2` encoder emits `width={W}px;height={H}px` derived from
/// `CSI 16 t` logical points, which causes Retina iTerm2 displays (`Show inline images at
/// Retina resolution` enabled by default) to draw images at half width and half height in the
/// top-left corner of the target rect. Emitting cell counts (`width={cols};height={rows}`) with
/// a Retina-scaled PNG fixes both alignment and sizing.
///
/// For `Kitty`, [`TerminalImageProtocol`] alternates between a bounded pair of image IDs (`id` and
/// `id ^ 1`) whenever a previously rendered texture is replaced or resized. Flipping bit 0 of the
/// ID changes the placeholder foreground color (`cell.fg`) across every row so Ratatui's buffer
/// diff refreshes all rows instead of transmitting only row 0 after Kitty deletes the prior ID.
pub struct TerminalImageProtocol {
    inner: InnerProtocol,
}

enum InnerProtocol {
    /// Delegates `Halfblocks` and `Sixel` directly to `ratatui-image`.
    Ratatui(ProtocolType, StatefulProtocol),
    /// Double-buffers two Kitty image IDs (`id` and `id ^ 1`) so Ratatui's buffer diff
    /// refreshes all placeholder rows when replacing an image without leaking GPU IDs.
    Kitty(KittyDoubleBufferProtocol),
    /// Uses cell-unit `OSC 1337` escape sequences for `Iterm2`.
    Iterm2(Iterm2CellProtocol),
}

struct KittyDoubleBufferProtocol {
    image: DynamicImage,
    font_size: FontSize,
    background_color: Option<Rgba<u8>>,
    is_tmux: bool,
    compress: bool,
    active_id: u32,
    has_rendered: bool,
    stateful: StatefulProtocol,
}

impl KittyDoubleBufferProtocol {
    #[must_use]
    fn new(
        image: DynamicImage,
        font_size: FontSize,
        background_color: Option<Rgba<u8>>,
        is_tmux: bool,
        compress: bool,
        active_id: u32,
    ) -> Self {
        let stateful = StatefulProtocol::new(
            image.clone(),
            font_size,
            background_color,
            StatefulProtocolType::Kitty(StatefulKitty::new(active_id, is_tmux, compress)),
        );
        Self {
            image,
            font_size,
            background_color,
            is_tmux,
            compress,
            active_id,
            has_rendered: false,
            stateful,
        }
    }

    fn resize_encode(&mut self, resize: &Resize, size: Size) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if self.has_rendered {
            self.active_id ^= 1;
            self.has_rendered = false;
            self.stateful = StatefulProtocol::new(
                self.image.clone(),
                self.font_size,
                self.background_color,
                StatefulProtocolType::Kitty(StatefulKitty::new(
                    self.active_id,
                    self.is_tmux,
                    self.compress,
                )),
            );
        }
        self.stateful.resize_encode(resize, size);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.stateful.render(area, buf);
        self.has_rendered = true;
    }
}

struct Iterm2CellProtocol {
    image: DynamicImage,
    font_size: FontSize,
    background_color: Option<Rgba<u8>>,
    is_tmux: bool,
    encoded: Option<(Size, String)>,
}

impl TerminalImageProtocol {
    /// Returns the background compositing color configured on this protocol, if any.
    #[must_use]
    pub fn background_color(&self) -> Option<Rgba<u8>> {
        match &self.inner {
            InnerProtocol::Ratatui(_, proto) => proto.background_color(),
            InnerProtocol::Kitty(kitty) => kitty.background_color,
            InnerProtocol::Iterm2(iterm2) => iterm2.background_color,
        }
    }

    /// Returns the [`ProtocolType`] implemented by this protocol instance.
    #[must_use]
    pub fn protocol_kind(&self) -> ProtocolType {
        match &self.inner {
            InnerProtocol::Ratatui(kind, _) => *kind,
            InnerProtocol::Kitty(_) => ProtocolType::Kitty,
            InnerProtocol::Iterm2(_) => ProtocolType::Iterm2,
        }
    }

    /// Returns the terminal-cell [`Size`] this image occupies inside `available` under `resize`.
    #[must_use]
    pub fn size_for(&self, resize: &Resize, available: Size) -> Size {
        match &self.inner {
            InnerProtocol::Ratatui(_, proto) => proto.size_for(resize.clone(), available),
            InnerProtocol::Kitty(kitty) => kitty.stateful.size_for(resize.clone(), available),
            InnerProtocol::Iterm2(iterm2) => iterm2.size_for(resize, available),
        }
    }

    /// Creates a `Halfblocks` test protocol with custom [`FontSize`] metrics.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn new_halfblocks_with_font_size(
        image: DynamicImage,
        font_size: FontSize,
        background_color: Option<Rgba<u8>>,
    ) -> Self {
        use ratatui_image::protocol::StatefulProtocolType;
        use ratatui_image::protocol::halfblocks::Halfblocks;

        Self {
            inner: InnerProtocol::Ratatui(
                ProtocolType::Halfblocks,
                StatefulProtocol::new(
                    image,
                    font_size,
                    background_color,
                    StatefulProtocolType::Halfblocks(Halfblocks::default()),
                ),
            ),
        }
    }

    /// Creates an `Iterm2` test protocol with custom [`FontSize`] and tmux wrapping settings.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn new_iterm2_with_font_size(
        image: DynamicImage,
        font_size: FontSize,
        background_color: Option<Rgba<u8>>,
        tmux_mode: TmuxMode,
    ) -> Self {
        Self {
            inner: InnerProtocol::Iterm2(Iterm2CellProtocol::new(
                image,
                font_size,
                background_color,
                tmux_mode.is_tmux(),
            )),
        }
    }
}

/// Whether `iTerm2` escape sequences are wrapped in `tmux` DCS passthrough envelopes in tests.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TmuxMode {
    /// Emit standard `OSC 1337` sequences directly to the terminal.
    Direct,
    /// Wrap escape payloads inside `tmux` passthrough sequences.
    Tmux,
}

#[cfg(test)]
impl TmuxMode {
    #[must_use]
    const fn is_tmux(self) -> bool {
        matches!(self, Self::Tmux)
    }
}

impl ResizeEncodeRender for TerminalImageProtocol {
    fn resize_encode(&mut self, resize: &Resize, size: Size) {
        match &mut self.inner {
            InnerProtocol::Ratatui(_, proto) => proto.resize_encode(resize, size),
            InnerProtocol::Kitty(kitty) => kitty.resize_encode(resize, size),
            InnerProtocol::Iterm2(iterm2) => iterm2.resize_encode(resize, size),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        match &mut self.inner {
            InnerProtocol::Ratatui(_, proto) => proto.render(area, buf),
            InnerProtocol::Kitty(kitty) => kitty.render(area, buf),
            InnerProtocol::Iterm2(iterm2) => iterm2.render(area, buf),
        }
    }

    fn needs_resize(&self, resize: &Resize, size: Size) -> Option<Size> {
        match &self.inner {
            InnerProtocol::Ratatui(_, proto) => proto.needs_resize(resize, size),
            InnerProtocol::Kitty(kitty) => kitty.stateful.needs_resize(resize, size),
            InnerProtocol::Iterm2(iterm2) => iterm2.needs_resize(resize, size),
        }
    }
}

/// Pre-composites `image` onto `background_color` when transparent pixels are present.
#[must_use]
fn precomposite_on_bg(mut image: DynamicImage, background_color: Option<Rgba<u8>>) -> DynamicImage {
    if let Some(bg) = background_color
        && bg.0[3] != 0
        && image.color().has_alpha()
    {
        let mut canvas: DynamicImage =
            ImageBuffer::from_pixel(image.width(), image.height(), bg).into();
        imageops::overlay(&mut canvas, &image, 0, 0);
        image = canvas;
    }
    image
}

/// Appends RFC 4648 base64 encoding of `data` to `out`.
fn encode_base64_append(data: &[u8], out: &mut String) {
    let encoded_len = data.len().div_ceil(3) * 4;
    out.reserve(encoded_len);

    let (chunks, rem) = data.as_chunks::<3>();
    for &[b0, b1, b2] in chunks {
        let n = (usize::from(b0) << 16) | (usize::from(b1) << 8) | usize::from(b2);
        out.push(char::from(BASE64_ALPHABET[(n >> 18) & 0x3F]));
        out.push(char::from(BASE64_ALPHABET[(n >> 12) & 0x3F]));
        out.push(char::from(BASE64_ALPHABET[(n >> 6) & 0x3F]));
        out.push(char::from(BASE64_ALPHABET[n & 0x3F]));
    }

    match rem {
        [a] => {
            let n = usize::from(*a) << 16;
            out.push(char::from(BASE64_ALPHABET[(n >> 18) & 0x3F]));
            out.push(char::from(BASE64_ALPHABET[(n >> 12) & 0x3F]));
            out.push('=');
            out.push('=');
        }
        [a, b] => {
            let n = (usize::from(*a) << 16) | (usize::from(*b) << 8);
            out.push(char::from(BASE64_ALPHABET[(n >> 18) & 0x3F]));
            out.push(char::from(BASE64_ALPHABET[(n >> 12) & 0x3F]));
            out.push(char::from(BASE64_ALPHABET[(n >> 6) & 0x3F]));
            out.push('=');
        }
        _ => {}
    }
}

/// Erases any prior text/glyphs inside the `width × height` cell box before placing `OSC 1337`.
fn append_clear_area(out: &mut String, escape: &str, width: u16, height: u16) -> fmt::Result {
    if height == 1 {
        write!(out, "{escape}[{width}X")?;
    } else {
        for _ in 0..height {
            write!(out, "{escape}[{width}X{escape}[1B")?;
        }
        write!(out, "{escape}[{height}A")?;
    }
    Ok(())
}

impl Iterm2CellProtocol {
    #[must_use]
    fn new(
        image: DynamicImage,
        font_size: FontSize,
        background_color: Option<Rgba<u8>>,
        is_tmux: bool,
    ) -> Self {
        Self {
            image: precomposite_on_bg(image, background_color),
            font_size,
            background_color,
            is_tmux,
            encoded: None,
        }
    }

    #[must_use]
    fn size_for(&self, resize: &Resize, available: Size) -> Size {
        resize.size_for(&self.image, self.font_size, available)
    }

    #[must_use]
    fn needs_resize(&self, resize: &Resize, size: Size) -> Option<Size> {
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let target = self.size_for(resize, size);
        let clamped = Size::new(
            target.width.clamp(1, size.width),
            target.height.clamp(1, size.height),
        );
        match &self.encoded {
            Some((encoded_size, _)) if *encoded_size == clamped => None,
            _ => Some(clamped),
        }
    }

    fn resize_encode(&mut self, resize: &Resize, size: Size) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        // Always oversample 2x up to `MAX_ITERM2_CANVAS_PX` so even zoomed-in Retina iTerm2 fonts
        // retain native physical pixel sharpness without blurring.
        let font_w = u32::from(self.font_size.width.max(1));
        let font_h = u32::from(self.font_size.height.max(1));
        let cell_px_w = u32::from(size.width).saturating_mul(font_w).max(1);
        let cell_px_h = u32::from(size.height).saturating_mul(font_h).max(1);
        let dpi_scale: u32 = if cell_px_w.saturating_mul(2) <= MAX_ITERM2_CANVAS_PX
            && cell_px_h.saturating_mul(2) <= MAX_ITERM2_CANVAS_PX
        {
            2
        } else {
            1
        };
        let canvas_w = cell_px_w.saturating_mul(dpi_scale);
        let canvas_h = cell_px_h.saturating_mul(dpi_scale);

        let scaled = match resize {
            Resize::Fit(filter) => {
                let cap_w = canvas_w.min(self.image.width().saturating_mul(dpi_scale).max(1));
                let cap_h = canvas_h.min(self.image.height().saturating_mul(dpi_scale).max(1));
                self.image
                    .resize(cap_w, cap_h, filter.unwrap_or(FilterType::Nearest))
            }
            Resize::Scale(filter) => {
                self.image
                    .resize(canvas_w, canvas_h, filter.unwrap_or(FilterType::Nearest))
            }
            Resize::Crop(options) => {
                let clip_top = options.as_ref().is_some_and(|o| o.clip_top);
                let clip_left = options.as_ref().is_some_and(|o| o.clip_left);
                let width = min(self.image.width(), cell_px_w);
                let height = min(self.image.height(), cell_px_h);
                let crop_x = if clip_left {
                    self.image.width().saturating_sub(width)
                } else {
                    0
                };
                let crop_y = if clip_top {
                    self.image.height().saturating_sub(height)
                } else {
                    0
                };
                let cropped = self.image.crop_imm(crop_x, crop_y, width, height);
                if dpi_scale > 1 {
                    cropped.resize_exact(
                        width.saturating_mul(dpi_scale).max(1),
                        height.saturating_mul(dpi_scale).max(1),
                        FilterType::Nearest,
                    )
                } else {
                    cropped
                }
            }
        };

        let final_img = if scaled.width() == canvas_w && scaled.height() == canvas_h {
            scaled
        } else {
            // Pad centered onto the full cell canvas so sub-cell rounding stays centered.
            let mut canvas: DynamicImage = ImageBuffer::from_pixel(
                canvas_w,
                canvas_h,
                self.background_color.unwrap_or(TRANSPARENT_RGBA),
            )
            .into();
            let pad_x = i64::from(canvas_w.saturating_sub(scaled.width()) / 2);
            let pad_y = i64::from(canvas_h.saturating_sub(scaled.height()) / 2);
            imageops::overlay(&mut canvas, &scaled, pad_x, pad_y);
            canvas
        };

        let mut png = Vec::new();
        if let Err(err) = final_img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png) {
            log::warn!("Failed to encode Iterm2 PNG payload: {err}");
            return;
        }

        let (start, escape, end) = Parser::tmux_start_escape_end(self.is_tmux);
        let mut seq = String::from(start);
        if append_clear_area(&mut seq, escape, size.width, size.height).is_err() {
            return;
        }
        // Use cell units (`width=<cols>;height=<rows>`) rather than `px` because iTerm2 treats
        // `px` as physical Retina pixels while `CSI 16 t` reports logical points on macOS HiDPI.
        // Disable `preserveAspectRatio` when padded onto an opaque background so fractional
        // cell remainders cannot expose iTerm2's default (white) terminal background.
        let preserve_aspect = u8::from(self.background_color.is_none_or(|bg| bg.0[3] == 0));
        if write!(
            seq,
            "{escape}]1337;File=inline=1;size={};width={};height={};preserveAspectRatio={preserve_aspect};doNotMoveCursor=1:",
            png.len(),
            size.width,
            size.height,
        )
        .is_err()
        {
            return;
        }
        encode_base64_append(&png, &mut seq);
        if write!(seq, "\x07{end}").is_err() {
            return;
        }

        self.encoded = Some((size, seq));
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let Some((enc_size, seq)) = &self.encoded else {
            return;
        };
        if enc_size.width == 0
            || enc_size.height == 0
            || enc_size.width > area.width
            || enc_size.height > area.height
        {
            return;
        }

        if let Some(cell) = buf.cell_mut((area.x, area.y)) {
            cell.set_symbol(seq)
                .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::MIN));
        }

        let render_box = Rect::new(area.x, area.y, enc_size.width, enc_size.height);
        for y in render_box.top()..render_box.bottom() {
            for x in render_box.left()..render_box.right() {
                if x == render_box.left() && y == render_box.top() {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_diff_option(CellDiffOption::Skip);
                }
            }
        }
    }
}

/// Extracts the numeric image ID from a Kitty placeholder transmission escape (`i=<id>,a=T,U=1`).
#[cfg(test)]
#[must_use]
pub(crate) fn extract_kitty_transmit_id(symbol: &str) -> Option<u32> {
    let after_prefix = symbol.split("i=").nth(1)?;
    let id_str = after_prefix.split(",a=T,U=1").next()?;
    id_str.parse::<u32>().ok()
}

/// Builds a [`TerminalImageProtocol`] for `image` and reuses any compatible `existing` slot.
///
/// For `Kitty`, this double-buffers a pair of image IDs (`id` and `id ^ 1`), flipping to the
/// inactive ID only when `existing` has already rendered. That forces Ratatui's buffer diff to
/// update every placeholder row while keeping GPU texture storage bounded to two slots per widget.
#[must_use]
pub fn new_or_reused_protocol(
    picker: &Picker,
    existing: Option<TerminalImageProtocol>,
    image: DynamicImage,
    background_color: Option<Rgba<u8>>,
) -> TerminalImageProtocol {
    if picker.protocol_type() == ProtocolType::Iterm2 {
        return TerminalImageProtocol {
            inner: InnerProtocol::Iterm2(Iterm2CellProtocol::new(
                image,
                picker.font_size(),
                background_color,
                picker.tmux_detected(),
            )),
        };
    }

    if picker.protocol_type() == ProtocolType::Kitty {
        let compress = picker
            .capabilities()
            .contains(&Capability::KittyCompression);
        let active_id = match existing {
            Some(TerminalImageProtocol {
                inner: InnerProtocol::Kitty(prev),
            }) => {
                if prev.has_rendered {
                    prev.active_id ^ 1
                } else {
                    prev.active_id
                }
            }
            _ => allocate_kitty_base_id(),
        };
        return TerminalImageProtocol {
            inner: InnerProtocol::Kitty(KittyDoubleBufferProtocol::new(
                image,
                picker.font_size(),
                background_color,
                picker.tmux_detected(),
                compress,
                active_id,
            )),
        };
    }

    if let Some(prev) = existing
        && let InnerProtocol::Ratatui(kind, prev_proto) = prev.inner
        && kind == picker.protocol_type()
    {
        let proto_type = prev_proto.protocol_type_owned();
        return TerminalImageProtocol {
            inner: InnerProtocol::Ratatui(
                kind,
                StatefulProtocol::new(image, picker.font_size(), background_color, proto_type),
            ),
        };
    }

    let mut configured = picker.clone();
    configured.set_background_color(background_color);
    let proto = configured.new_resize_protocol(image);
    TerminalImageProtocol {
        inner: InnerProtocol::Ratatui(picker.protocol_type(), proto),
    }
}

/// Computes the centered terminal-cell [`Rect`] that `protocol` occupies inside `available`
/// under `resize`.
///
/// Uses the [`Picker`]'s measured cell pixel metrics inside `protocol` and returns an empty
/// [`Rect`] if `available` has zero width or height.
#[must_use]
pub fn centered_image_rect(
    protocol: &TerminalImageProtocol,
    resize: &Resize,
    available: Rect,
) -> Rect {
    // Return before calling `.clamp(1, ...)` because `u16::clamp(1, 0)` panics on zero-sized rects.
    if available.width == 0 || available.height == 0 {
        return Rect::new(available.x, available.y, 0, 0);
    }

    let target = protocol.size_for(resize, Size::new(available.width, available.height));
    let render_w = target.width.clamp(1, available.width);
    let render_h = target.height.clamp(1, available.height);
    let x = available.x + (available.width.saturating_sub(render_w)) / 2;
    let y = available.y + (available.height.saturating_sub(render_h)) / 2;
    Rect::new(x, y, render_w, render_h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::ui_state::UIState;
    use image::ImageBuffer;
    use ratatui::buffer::Buffer;
    use ratatui::widgets::StatefulWidget;
    use ratatui_image::{CropOptions, FilterType, StatefulImage};
    use std::time::Instant;

    fn decode_base64(input: &str) -> Vec<u8> {
        let mut vals = Vec::with_capacity(input.len());
        for b in input.bytes() {
            match b {
                b'A'..=b'Z' => vals.push(b - b'A'),
                b'a'..=b'z' => vals.push(b - b'a' + 26),
                b'0'..=b'9' => vals.push(b - b'0' + 52),
                b'+' => vals.push(62),
                b'/' => vals.push(63),
                b'=' => break,
                other => panic!("unexpected base64 byte: {other}"),
            }
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < vals.len() {
            let b0 = vals[i];
            let b1 = vals[i + 1];
            out.push((b0 << 2) | (b1 >> 4));
            if i + 2 < vals.len() {
                let b2 = vals[i + 2];
                out.push((b1 << 4) | (b2 >> 2));
                if i + 3 < vals.len() {
                    let b3 = vals[i + 3];
                    out.push((b2 << 6) | b3);
                }
            }
            i += 4;
        }
        out
    }

    #[test]
    fn test_create_headless_picker_is_halfblocks() {
        let picker = create_headless_picker();
        assert_eq!(picker.protocol_type(), ProtocolType::Halfblocks);
        assert_eq!(picker.font_size().width, 10);
        assert_eq!(picker.font_size().height, 20);

        let ui_state = UIState::new(Theme::default());
        let ui_picker = ui_state
            .picker
            .as_ref()
            .expect("UIState::new should initialize a headless picker");
        assert_eq!(ui_picker.protocol_type(), ProtocolType::Halfblocks);
        assert_eq!(ui_picker.font_size().width, 10);
        assert_eq!(ui_picker.font_size().height, 20);
    }

    #[test]
    fn test_build_query_options_blacklists_for_iterm2_family_only() {
        let iterm2_cases = [
            (Some("xterm-256color"), Some("iTerm.app"), None),
            (Some("xterm-256color"), Some("WezTerm"), None),
            (Some("xterm-256color"), Some("rio"), None),
            (Some("xterm-256color"), Some("mintty"), None),
            (Some("xterm-256color"), Some("Bobcat"), None),
            (Some("xterm-256color"), None, Some("iTerm2")),
            (None, Some("iTerm.app"), None),
        ];

        for (term, term_program, lc_terminal) in iterm2_cases {
            assert!(
                prefers_iterm2_over_sixel(term, term_program, lc_terminal),
                "expected iTerm2 preference for ({term:?}, {term_program:?}, {lc_terminal:?})"
            );
            let opts = build_query_options(term, term_program, lc_terminal);
            assert_eq!(
                opts.blacklist_protocols,
                vec![ProtocolType::Kitty, ProtocolType::Sixel]
            );
            assert_eq!(opts.timeout, Duration::from_millis(500));
            assert!(!opts.text_sizing_protocol);
            assert!(!opts.terminal_background_color_osc);
            assert!(!opts.kitty_compression);
        }

        let non_iterm2_cases = [
            (Some("xterm-ghostty"), Some("ghostty"), None),
            (Some("xterm-kitty"), None, None),
            (Some("foot"), None, None),
            (Some("xterm-256color"), Some("WarpTerminal"), None),
            (Some("xterm-256color"), Some("vscode"), None),
            (None, None, None),
            // Stale outer terminal hints inside tmux must never blacklist Kitty/Sixel.
            (Some("tmux-256color"), None, Some("iTerm.app")),
            (Some("tmux-256color"), Some("iTerm.app"), Some("iTerm.app")),
            (Some("xterm-256color"), Some("tmux"), Some("iTerm.app")),
            (Some("tmux"), Some("WezTerm"), Some("iTerm2")),
        ];

        for (term, term_program, lc_terminal) in non_iterm2_cases {
            assert!(
                !prefers_iterm2_over_sixel(term, term_program, lc_terminal),
                "expected no iTerm2 blacklist for ({term:?}, {term_program:?}, {lc_terminal:?})"
            );
            let opts = build_query_options(term, term_program, lc_terminal);
            assert!(opts.blacklist_protocols.is_empty());
            assert_eq!(opts.timeout, Duration::from_millis(500));
            assert!(!opts.text_sizing_protocol);
            assert!(!opts.terminal_background_color_osc);
            assert!(!opts.kitty_compression);
        }
    }

    #[test]
    fn test_probe_preguards_and_zero_font_size_fallback() {
        assert!(!should_probe_stdio(false, true, Some("xterm-256color")));
        assert!(!should_probe_stdio(true, false, Some("xterm-256color")));
        assert!(!should_probe_stdio(true, true, Some("dumb")));
        assert!(should_probe_stdio(true, true, Some("xterm-ghostty")));
        assert!(should_probe_stdio(true, true, None));

        assert!(!has_valid_font_size(FontSize::new(0, 20)));
        assert!(!has_valid_font_size(FontSize::new(10, 0)));
        assert!(!has_valid_font_size(FontSize::new(0, 0)));
        assert!(has_valid_font_size(FontSize::new(18, 40)));

        let term = env::var("TERM").ok();
        if !should_probe_stdio(
            io::stdin().is_terminal(),
            io::stdout().is_terminal(),
            term.as_deref(),
        ) {
            let start = Instant::now();
            let picker = probe_terminal_picker();
            assert_eq!(picker.protocol_type(), ProtocolType::Halfblocks);
            assert!(has_valid_font_size(picker.font_size()));
            assert!(start.elapsed() < Duration::from_millis(200));
        }
    }

    #[test]
    fn test_theme_color_to_rgba_and_indexed_palette() {
        assert_eq!(theme_color_to_rgba(Color::Reset), None);
        assert_eq!(
            theme_color_to_rgba(Color::Rgb(40, 42, 54)),
            Some(Rgba([40, 42, 54, 255]))
        );
        assert_eq!(
            theme_color_to_rgba(Color::Rgb(253, 246, 227)),
            Some(Rgba([253, 246, 227, 255]))
        );

        let ansi_cases = [
            (Color::Black, [0, 0, 0]),
            (Color::Red, [205, 0, 0]),
            (Color::Green, [0, 205, 0]),
            (Color::Yellow, [205, 205, 0]),
            (Color::Blue, [0, 0, 238]),
            (Color::Magenta, [205, 0, 205]),
            (Color::Cyan, [0, 205, 205]),
            (Color::Gray, [229, 229, 229]),
            (Color::DarkGray, [127, 127, 127]),
            (Color::LightRed, [255, 0, 0]),
            (Color::LightGreen, [0, 255, 0]),
            (Color::LightYellow, [255, 255, 0]),
            (Color::LightBlue, [92, 92, 255]),
            (Color::LightMagenta, [255, 0, 255]),
            (Color::LightCyan, [0, 255, 255]),
            (Color::White, [255, 255, 255]),
        ];
        for (color, [r, g, b]) in ansi_cases {
            assert_eq!(theme_color_to_rgba(color), Some(Rgba([r, g, b, 255])));
        }

        let indexed_ansi = [
            [0, 0, 0],
            [128, 0, 0],
            [0, 128, 0],
            [128, 128, 0],
            [0, 0, 128],
            [128, 0, 128],
            [0, 128, 128],
            [192, 192, 192],
            [128, 128, 128],
            [255, 0, 0],
            [0, 255, 0],
            [255, 255, 0],
            [0, 0, 255],
            [255, 0, 255],
            [0, 255, 255],
            [255, 255, 255],
        ];
        for (idx, expected) in indexed_ansi.iter().enumerate() {
            assert_eq!(
                theme_color_to_rgba(Color::Indexed(idx as u8)),
                Some(Rgba([expected[0], expected[1], expected[2], 255]))
            );
        }

        assert_eq!(
            theme_color_to_rgba(Color::Indexed(16)),
            Some(Rgba([0, 0, 0, 255]))
        );
        assert_eq!(
            theme_color_to_rgba(Color::Indexed(196)),
            Some(Rgba([255, 0, 0, 255]))
        );
        assert_eq!(
            theme_color_to_rgba(Color::Indexed(231)),
            Some(Rgba([255, 255, 255, 255]))
        );
        assert_eq!(
            theme_color_to_rgba(Color::Indexed(232)),
            Some(Rgba([8, 8, 8, 255]))
        );
        assert_eq!(
            theme_color_to_rgba(Color::Indexed(255)),
            Some(Rgba([238, 238, 238, 255]))
        );
    }

    #[test]
    fn test_centered_image_rect_uses_picker_font_size() {
        let image = DynamicImage::new_rgb8(320, 200);
        let available = Rect::new(10, 5, 78, 25);
        let fit_resize = Resize::Fit(Some(FilterType::Lanczos3));
        let scale_resize = Resize::Scale(Some(FilterType::Nearest));

        let proto_std = TerminalImageProtocol::new_halfblocks_with_font_size(
            image.clone(),
            FontSize::new(10, 20),
            None,
        );
        let fit_std = centered_image_rect(&proto_std, &fit_resize, available);
        assert_eq!(fit_std, Rect::new(33, 12, 32, 10));

        let scale_std = centered_image_rect(&proto_std, &scale_resize, available);
        assert_eq!(scale_std, Rect::new(10, 5, 78, 25));

        let proto_retina = TerminalImageProtocol::new_halfblocks_with_font_size(
            image,
            FontSize::new(18, 40),
            None,
        );
        let fit_retina = centered_image_rect(&proto_retina, &fit_resize, available);
        assert_eq!(fit_retina, Rect::new(40, 15, 18, 5));

        let scale_retina = centered_image_rect(&proto_retina, &scale_resize, available);
        assert_eq!(scale_retina, Rect::new(10, 6, 78, 22));
        assert!(scale_retina.width > 32 && scale_retina.height > 10);

        for rect in [fit_std, scale_std, fit_retina, scale_retina] {
            assert!(rect.x >= available.x);
            assert!(rect.y >= available.y);
            assert!(rect.right() <= available.right());
            assert!(rect.bottom() <= available.bottom());
        }
    }

    #[test]
    fn test_centered_image_rect_zero_area_clamp_safety() {
        let image = DynamicImage::new_rgb8(320, 200);
        let proto = TerminalImageProtocol::new_halfblocks_with_font_size(
            image,
            FontSize::new(10, 20),
            None,
        );
        let resize = Resize::Fit(Some(FilterType::Lanczos3));

        assert_eq!(
            centered_image_rect(&proto, &resize, Rect::new(4, 7, 0, 10)),
            Rect::new(4, 7, 0, 0)
        );
        assert_eq!(
            centered_image_rect(&proto, &resize, Rect::new(4, 7, 20, 0)),
            Rect::new(4, 7, 0, 0)
        );
        assert_eq!(
            centered_image_rect(&proto, &resize, Rect::new(4, 7, 0, 0)),
            Rect::new(4, 7, 0, 0)
        );
    }

    #[test]
    fn test_new_or_reused_protocol_preserves_kitty_variant() {
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Kitty);

        let img1: DynamicImage =
            ImageBuffer::from_pixel(20, 20, Rgba::<u8>([10, 20, 30, 255])).into();
        let img2: DynamicImage =
            ImageBuffer::from_pixel(20, 20, Rgba::<u8>([200, 100, 50, 255])).into();
        let img3: DynamicImage =
            ImageBuffer::from_pixel(20, 20, Rgba::<u8>([40, 80, 120, 255])).into();
        let img4: DynamicImage =
            ImageBuffer::from_pixel(20, 20, Rgba::<u8>([90, 180, 210, 255])).into();
        let img5: DynamicImage =
            ImageBuffer::from_pixel(20, 20, Rgba::<u8>([15, 25, 35, 255])).into();

        let area = Rect::new(0, 0, 10, 5);
        let resize = Resize::Scale(None);

        // 1. Frame 1 allocates an even 24-bit base ID (>= 2) and renders all 50 cells.
        let mut proto1 = new_or_reused_protocol(&picker, None, img1, None);
        let mut buf1 = Buffer::empty(area);
        StatefulImage::new()
            .resize(resize.clone())
            .render(area, &mut buf1, &mut proto1);

        let sym1 = buf1[(0, 0)].symbol();
        assert!(sym1.contains('\u{10EEEE}'));
        assert!(sym1.contains(",a=T,U=1"));
        let id1 = extract_kitty_transmit_id(sym1)
            .expect("first Kitty frame should include i=<id>,a=T,U=1 transmission sequence");
        assert!(id1 >= 2 && id1.is_multiple_of(2));
        assert_eq!(
            buf1[(0, 0)].fg,
            Color::Rgb((id1 >> 16) as u8, (id1 >> 8) as u8, id1 as u8)
        );

        // 2. Rebuilding after render flips to `id1 ^ 1`, forcing `Buffer::diff` across all rows.
        let bg = Some(Rgba([253, 246, 227, 255]));
        let mut proto2 = new_or_reused_protocol(&picker, Some(proto1), img2, bg);
        assert_eq!(proto2.background_color(), bg);
        assert_eq!(proto2.protocol_kind(), ProtocolType::Kitty);

        let mut buf2 = Buffer::empty(area);
        StatefulImage::new()
            .resize(resize.clone())
            .render(area, &mut buf2, &mut proto2);
        let sym2 = buf2[(0, 0)].symbol();
        assert!(sym2.contains('\u{10EEEE}'));
        assert_ne!(buf2[(0, 0)].fg, buf1[(0, 0)].fg);
        let id2 = extract_kitty_transmit_id(sym2)
            .expect("reused Kitty protocol with updated image should emit new frame");
        assert_eq!(id2, id1 ^ 1);

        let diff12 = buf1.diff(&buf2);
        assert_eq!(diff12.len(), usize::from(area.width * area.height));

        // 3. Multiple rebuilds while unrendered keep `id1` (do not flip back to `id2`), and once
        //    rendered into `buf3` flip `Buffer::diff` across all 50 cells again.
        let proto_unrendered1 = new_or_reused_protocol(&picker, Some(proto2), img3.clone(), bg);
        let mut proto_unrendered2 =
            new_or_reused_protocol(&picker, Some(proto_unrendered1), img4, bg);
        let mut buf3 = Buffer::empty(area);
        StatefulImage::new()
            .resize(resize.clone())
            .render(area, &mut buf3, &mut proto_unrendered2);
        let id3 = extract_kitty_transmit_id(buf3[(0, 0)].symbol())
            .expect("unrendered rebuild chain must still transmit on first render");
        assert_eq!(id3, id1);
        assert_eq!(
            buf2.diff(&buf3).len(),
            usize::from(area.width * area.height)
        );

        // 4. In-place `resize_encode` on an already-rendered Kitty protocol also flips `active_id`.
        let larger_area = Rect::new(0, 0, 12, 6);
        let mut buf_resized = Buffer::empty(larger_area);
        StatefulImage::new().resize(resize).render(
            larger_area,
            &mut buf_resized,
            &mut proto_unrendered2,
        );
        let id_resized = extract_kitty_transmit_id(buf_resized[(0, 0)].symbol())
            .expect("in-place resize_encode must transmit with flipped Kitty ID");
        assert_eq!(id_resized, id1 ^ 1);

        // 5. Protocol mismatch allocates a fresh pair.
        picker.set_protocol_type(ProtocolType::Sixel);
        assert_ne!(proto_unrendered2.protocol_kind(), picker.protocol_type());
        let proto_sixel = new_or_reused_protocol(&picker, Some(proto_unrendered2), img3, None);
        assert_eq!(proto_sixel.protocol_kind(), ProtocolType::Sixel);

        picker.set_protocol_type(ProtocolType::Kitty);
        let mut proto_fresh = new_or_reused_protocol(&picker, Some(proto_sixel), img5, None);
        let mut buf_fresh = Buffer::empty(area);
        StatefulImage::new().render(area, &mut buf_fresh, &mut proto_fresh);
        let id_fresh = extract_kitty_transmit_id(buf_fresh[(0, 0)].symbol())
            .expect("re-creating Kitty protocol after mismatch should transmit new ID");
        assert_ne!(id_fresh, id1);
        assert_ne!(id_fresh, id2);
    }

    #[test]
    fn test_sanitize_probed_protocol_blocks_unapproved_iterm2_fallbacks() {
        let blocked_iterm2 = [
            (Some("vscode"), None, false, false),
            (Some("WarpTerminal"), None, false, false),
            (Some("Tabby"), None, false, false),
            (Some("Hyper"), None, false, false),
            // Blocked inner terminals stay blocked even if outer iTerm/WezTerm env vars leaked.
            (Some("vscode"), Some("iTerm2"), true, true),
            (Some("WarpTerminal"), None, false, true),
            // No positive iTerm2/WezTerm indicator at all.
            (None, None, false, false),
            (Some("tmux"), None, false, false),
        ];
        for (term_program, lc_terminal, has_wezterm, has_iterm_id) in blocked_iterm2 {
            assert_eq!(
                sanitize_probed_protocol(
                    ProtocolType::Iterm2,
                    term_program,
                    lc_terminal,
                    has_wezterm,
                    has_iterm_id,
                ),
                ProtocolType::Halfblocks,
                "expected Halfblocks for unapproved Iterm2 fallback ({term_program:?}, {lc_terminal:?}, {has_wezterm}, {has_iterm_id})"
            );
        }

        let approved_iterm2 = [
            (Some("iTerm.app"), None, false, false),
            (Some("WezTerm"), None, false, false),
            (Some("rio"), None, false, false),
            (Some("mintty"), None, false, false),
            (Some("Bobcat"), None, false, false),
            (None, Some("iTerm2"), false, false),
            // WezTerm and iTerm2 running inside tmux (where TERM_PROGRAM="tmux").
            (Some("tmux"), None, true, false),
            (Some("tmux"), None, false, true),
            (None, None, true, false),
            (None, None, false, true),
        ];
        for (term_program, lc_terminal, has_wezterm, has_iterm_id) in approved_iterm2 {
            assert_eq!(
                sanitize_probed_protocol(
                    ProtocolType::Iterm2,
                    term_program,
                    lc_terminal,
                    has_wezterm,
                    has_iterm_id,
                ),
                ProtocolType::Iterm2,
                "expected Iterm2 for approved env ({term_program:?}, {lc_terminal:?}, {has_wezterm}, {has_iterm_id})"
            );
        }

        for passthrough in [
            ProtocolType::Kitty,
            ProtocolType::Sixel,
            ProtocolType::Halfblocks,
        ] {
            assert_eq!(
                sanitize_probed_protocol(passthrough, Some("vscode"), Some("iTerm2"), true, true,),
                passthrough
            );
        }
    }

    #[test]
    fn test_encode_base64_append_rfc4648_vectors() {
        let vectors = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, expected) in vectors {
            let mut out = String::new();
            encode_base64_append(plain.as_bytes(), &mut out);
            assert_eq!(out, expected);
            assert_eq!(decode_base64(&out), plain.as_bytes());
        }

        let all_bytes: Vec<u8> = (0u8..=255).collect();
        let mut encoded = String::new();
        encode_base64_append(&all_bytes, &mut encoded);
        assert_eq!(decode_base64(&encoded), all_bytes);
    }

    fn decode_iterm2_png(sym: &str, width: u16, height: u16) -> image::RgbaImage {
        let header = format!(";width={width};height={height};");
        let after_header = sym
            .split(&header)
            .nth(1)
            .expect("OSC 1337 sequence should contain expected cell dimensions");
        let payload_b64 = after_header
            .split(':')
            .nth(1)
            .and_then(|s| s.split('\x07').next())
            .expect("base64 PNG payload should follow OSC 1337 header");
        image::load_from_memory(&decode_base64(payload_b64))
            .expect("OSC 1337 payload must decode as valid PNG")
            .to_rgba8()
    }

    #[test]
    fn test_iterm2_cell_protocol_retina_oversampling_and_no_white_border_bleed() {
        // Reproduce user's iTerm2 Retina cell report (`7x17` points) and Catppuccin Mocha `dialog_bg`.
        let img: DynamicImage =
            ImageBuffer::from_pixel(320, 200, Rgba::<u8>([100, 180, 220, 255])).into();
        let mocha_bg = Rgba([49, 50, 68, 255]);
        let mut proto = TerminalImageProtocol::new_iterm2_with_font_size(
            img.clone(),
            FontSize::new(7, 17),
            Some(mocha_bg),
            TmuxMode::Direct,
        );

        let available = Rect::new(5, 2, 60, 20);
        let resize = Resize::Fit(Some(FilterType::Lanczos3));
        let render_area = centered_image_rect(&proto, &resize, available);
        assert_eq!(render_area.width, 46);
        assert_eq!(render_area.height, 12);

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 30));
        StatefulImage::new()
            .resize(resize.clone())
            .render(render_area, &mut buf, &mut proto);

        let sym = buf[(render_area.x, render_area.y)].symbol();
        assert!(!sym.contains("px;"), "must emit cell units, got: {sym}");
        let expected_header = format!(
            ";width={};height={};preserveAspectRatio=0;doNotMoveCursor=1:",
            render_area.width, render_area.height
        );
        assert!(
            sym.contains(&expected_header),
            "expected `{expected_header}` in OSC 1337 sequence"
        );
        assert_eq!(
            buf[(render_area.x, render_area.y)].diff_option,
            CellDiffOption::ForcedWidth(NonZeroU16::MIN)
        );
        assert_eq!(
            buf[(render_area.x + 1, render_area.y)].diff_option,
            CellDiffOption::Skip
        );
        assert_eq!(
            buf[(render_area.x, render_area.y + 1)].diff_option,
            CellDiffOption::Skip
        );

        let decoded_png = decode_iterm2_png(sym, render_area.width, render_area.height);
        assert_eq!(decoded_png.width(), u32::from(render_area.width) * 14);
        assert_eq!(decoded_png.height(), u32::from(render_area.height) * 34);
        assert_eq!(*decoded_png.get_pixel(0, 0), mocha_bg);
        assert_eq!(
            *decoded_png.get_pixel(decoded_png.width() - 1, decoded_png.height() - 1),
            mocha_bg
        );

        // Transparent `background_color = None` keeps `preserveAspectRatio=1` and wraps in tmux.
        let mut proto_tmux = TerminalImageProtocol::new_iterm2_with_font_size(
            img,
            FontSize::new(7, 17),
            None,
            TmuxMode::Tmux,
        );
        let mut tmux_buf = Buffer::empty(Rect::new(0, 0, 80, 30));
        StatefulImage::new()
            .resize(resize)
            .render(render_area, &mut tmux_buf, &mut proto_tmux);
        let tmux_sym = tmux_buf[(render_area.x, render_area.y)].symbol();
        assert!(tmux_sym.starts_with("\x1bPtmux;\x1b\x1b"));
        assert!(tmux_sym.ends_with("\x07\x1b\\"));
        assert!(tmux_sym.contains(";preserveAspectRatio=1;doNotMoveCursor=1:"));
    }

    #[test]
    fn test_iterm2_cell_protocol_zoomed_font_and_crop_retina_coordinates() {
        // (1) Zoomed font (14x30 logical points) still oversamples 2x under `MAX_ITERM2_CANVAS_PX`.
        let solid: DynamicImage =
            ImageBuffer::from_pixel(320, 200, Rgba::<u8>([10, 200, 30, 255])).into();
        let zoomed_resize = Resize::Scale(Some(FilterType::Nearest));
        let mut zoomed_proto = TerminalImageProtocol::new_iterm2_with_font_size(
            solid,
            FontSize::new(14, 30),
            Some(Rgba([0, 0, 0, 255])),
            TmuxMode::Direct,
        );
        let zoomed_available = Rect::new(0, 0, 20, 5);
        let zoomed_area = centered_image_rect(&zoomed_proto, &zoomed_resize, zoomed_available);
        assert_eq!(zoomed_area, Rect::new(1, 0, 18, 5));
        let mut zoomed_buf = Buffer::empty(zoomed_available);
        StatefulImage::new().resize(zoomed_resize).render(
            zoomed_area,
            &mut zoomed_buf,
            &mut zoomed_proto,
        );
        let zoomed_png = decode_iterm2_png(
            zoomed_buf[(zoomed_area.x, zoomed_area.y)].symbol(),
            zoomed_area.width,
            zoomed_area.height,
        );
        assert_eq!(zoomed_png.width(), u32::from(zoomed_area.width) * 28);
        assert_eq!(zoomed_png.height(), u32::from(zoomed_area.height) * 60);

        // (2) Resize::Crop slices in 1x cell pixels before 2x Retina scaling.
        let red = Rgba([220, 40, 40, 255]);
        let green = Rgba([40, 220, 40, 255]);
        let blue = Rgba([40, 40, 220, 255]);
        let yellow = Rgba([240, 220, 40, 255]);
        let quad_img: DynamicImage =
            ImageBuffer::from_fn(320, 200, |x, y| match (x >= 160, y >= 100) {
                (false, false) => red,
                (true, false) => green,
                (false, true) => blue,
                (true, true) => yellow,
            })
            .into();

        // 20x5 cells at 7x17 points = 140x85 1x pixels -> bottom-right crop starts at (180, 115),
        // strictly inside the yellow bottom-right quadrant ([160..320) x [100..200)).
        let bg = Rgba([15, 15, 15, 255]);
        let crop_area = Rect::new(0, 0, 20, 5);
        let mut crop_proto = TerminalImageProtocol::new_iterm2_with_font_size(
            quad_img,
            FontSize::new(7, 17),
            Some(bg),
            TmuxMode::Direct,
        );
        let mut crop_buf = Buffer::empty(crop_area);
        StatefulImage::new()
            .resize(Resize::Crop(Some(CropOptions {
                clip_top: true,
                clip_left: true,
            })))
            .render(crop_area, &mut crop_buf, &mut crop_proto);
        let cropped_png =
            decode_iterm2_png(crop_buf[(0, 0)].symbol(), crop_area.width, crop_area.height);
        assert_eq!(cropped_png.width(), 280);
        assert_eq!(cropped_png.height(), 170);
        assert_eq!(*cropped_png.get_pixel(0, 0), yellow);
        assert_eq!(*cropped_png.get_pixel(140, 85), yellow);
        assert_eq!(*cropped_png.get_pixel(279, 169), yellow);
    }
}
