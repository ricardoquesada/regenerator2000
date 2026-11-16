//! Navigation domain action handler for cursor movement, pane toggling, search, address jumps,
//! symbolic-offset base redirection, and view settings.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::event::CoreEvent;
use crate::state::Addr;
use crate::state::actions::AppAction;
use crate::view_state::ActivePane;

/// Handler for navigation, search, and view management actions.
#[derive(Debug, Default)]
pub struct NavigationActionHandler;

impl NavigationActionHandler {
    /// Creates a new [`NavigationActionHandler`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Synchronizes the active right-pane cursor with the current disassembly cursor address
/// when the corresponding `sync_*` option in `system_config` is enabled.
fn sync_pane_cursor_to_disassembly(ctx: &mut ActionContext<'_>) {
    use crate::navigation::AlignedGridRange;

    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
        return;
    };
    if line.external_label_address.is_some() {
        return;
    }
    let addr = line.address;
    let addr_usize = addr.0 as usize;
    let is_splitter = line.is_splitter();
    match ctx.view.active_pane {
        ActivePane::HexDump if ctx.state.system_config.sync_hex_dump => {
            let grid = AlignedGridRange::hex_dump(ctx.state, ctx.view.right_pane);
            if let Some(row) = grid.index_for_addr(addr_usize) {
                ctx.view.hex_cursor_index = row;
                ctx.view.hex_col_cursor = (addr_usize - grid.aligned_origin) % grid.stride;
            }
        }
        ActivePane::Sprites if ctx.state.system_config.sync_sprites_view => {
            let grid = AlignedGridRange::sprites(ctx.state);
            if let Some(idx) = grid.index_for_addr(addr_usize) {
                ctx.view.sprites_cursor_index = idx;
            }
        }
        ActivePane::Charset if ctx.state.system_config.sync_charset_view => {
            let grid = AlignedGridRange::charset(ctx.state);
            if let Some(idx) = grid.index_for_addr(addr_usize) {
                ctx.view.charset_cursor_index = idx;
            }
        }
        ActivePane::Bitmap if ctx.state.system_config.sync_bitmap_view => {
            let grid = AlignedGridRange::bitmap(ctx.state);
            if let Some(idx) = grid.index_for_addr(addr_usize) {
                ctx.view.bitmap_cursor_index = idx;
            }
        }
        ActivePane::Blocks if ctx.state.system_config.sync_blocks_view => {
            if let Some(pos) = ctx.state.get_block_index_for_address(addr, is_splitter) {
                ctx.view.blocks_selected_index = Some(pos);
            }
        }
        _ => {}
    }
}

/// Cycles a two-layout right-hand pane (`wide -> narrow -> None -> wide`) or restores `last_pane`.
#[allow(clippy::too_many_arguments)]
fn toggle_two_column_pane(
    ctx: &mut ActionContext<'_>,
    active_pane: ActivePane,
    wide: crate::view_state::RightPane,
    narrow: crate::view_state::RightPane,
    last_pane: fn(&mut crate::view_state::CoreViewState) -> &mut crate::view_state::RightPane,
    wide_msg: &str,
    narrow_msg: &str,
    hidden_msg: &str,
) {
    use crate::navigation::AlignedGridRange;
    use crate::view_state::RightPane;
    match ctx.view.right_pane {
        p if p == wide => {
            // Preserve the exact byte address when cycling HexDump16 -> HexDump8 with sync disabled.
            let old_byte_addr = (active_pane == ActivePane::HexDump
                && !ctx.state.system_config.sync_hex_dump)
                .then(|| {
                    let old_grid = AlignedGridRange::hex_dump(ctx.state, p);
                    old_grid.item_addr(ctx.view.hex_cursor_index)
                        + ctx.view.hex_col_cursor.min(old_grid.stride - 1)
                });
            ctx.view.right_pane = narrow;
            *last_pane(ctx.view) = narrow;
            ctx.view.active_pane = active_pane;
            if let Some(byte_addr) = old_byte_addr {
                let new_grid = AlignedGridRange::hex_dump(ctx.state, narrow);
                if let Some(row) = new_grid.index_for_addr(byte_addr) {
                    ctx.view.hex_cursor_index = row;
                    ctx.view.hex_col_cursor =
                        (byte_addr - new_grid.aligned_origin) % new_grid.stride;
                }
            } else {
                sync_pane_cursor_to_disassembly(ctx);
            }
            ctx.events
                .push(CoreEvent::StatusMessage(narrow_msg.to_string()));
        }
        p if p == narrow => {
            ctx.view.right_pane = RightPane::None;
            ctx.events
                .push(CoreEvent::StatusMessage(hidden_msg.to_string()));
            if ctx.view.active_pane == active_pane {
                ctx.view.active_pane = ActivePane::Disassembly;
            }
        }
        RightPane::None => {
            ctx.view.right_pane = wide;
            *last_pane(ctx.view) = wide;
            ctx.view.active_pane = active_pane;
            sync_pane_cursor_to_disassembly(ctx);
            ctx.events
                .push(CoreEvent::StatusMessage(wide_msg.to_string()));
        }
        _ => {
            let restored = *last_pane(ctx.view);
            ctx.view.right_pane = restored;
            ctx.view.active_pane = active_pane;
            sync_pane_cursor_to_disassembly(ctx);
            let msg = if restored == narrow {
                narrow_msg
            } else {
                wide_msg
            };
            ctx.events.push(CoreEvent::StatusMessage(msg.to_string()));
        }
    }
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Toggles a single-layout right-hand pane (`right_pane <-> None`) and synchronizes its cursor.
fn toggle_single_pane(
    ctx: &mut ActionContext<'_>,
    right_pane: crate::view_state::RightPane,
    active_pane: ActivePane,
    shown_msg: &str,
    hidden_msg: &str,
) {
    if ctx.view.right_pane == right_pane {
        ctx.view.right_pane = crate::view_state::RightPane::None;
        ctx.events
            .push(CoreEvent::StatusMessage(hidden_msg.to_string()));
        if ctx.view.active_pane == active_pane {
            ctx.view.active_pane = ActivePane::Disassembly;
        }
    } else {
        ctx.view.right_pane = right_pane;
        ctx.view.active_pane = active_pane;
        sync_pane_cursor_to_disassembly(ctx);
        ctx.events
            .push(CoreEvent::StatusMessage(shown_msg.to_string()));
    }
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Jumps the disassembly cursor to `target_addr` and records navigation history.
fn handle_navigate_to_address(ctx: &mut ActionContext<'_>, target_addr: Addr) {
    crate::navigation::perform_jump_to_address(ctx.state, ctx.view, target_addr);
    if let Some(msg) = &ctx.view.status_message {
        ctx.events.push(CoreEvent::StatusMessage(msg.clone()));
    }
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Resolves the single jump target address for right-hand panes (`HexDump`, `Sprites`, `Charset`,
/// `Blocks`, and `Bitmap`).
///
/// Right-hand panes always map a cursor slot to one disassembly address directly instead of
/// opening the multi-target operand picker.
#[must_use]
fn resolve_non_disassembly_jump_target(ctx: &ActionContext<'_>) -> Option<Addr> {
    use crate::navigation::AlignedGridRange;

    match ctx.view.active_pane {
        ActivePane::HexDump => {
            let grid = AlignedGridRange::hex_dump(ctx.state, ctx.view.right_pane);
            grid.clamped_addr(
                grid.item_addr(ctx.view.hex_cursor_index)
                    + ctx.view.hex_col_cursor.min(grid.stride - 1),
            )
        }
        ActivePane::Sprites => {
            AlignedGridRange::sprites(ctx.state).clamped_item_addr(ctx.view.sprites_cursor_index)
        }
        ActivePane::Charset => {
            AlignedGridRange::charset(ctx.state).clamped_item_addr(ctx.view.charset_cursor_index)
        }
        ActivePane::Blocks => {
            let blocks = ctx.state.get_blocks_view_items();
            let idx = ctx.view.blocks_selected_index.unwrap_or(0);
            blocks.get(idx).map(|item| match *item {
                crate::state::BlockItem::Block { start, .. }
                | crate::state::BlockItem::Scope { start, .. } => start,
                crate::state::BlockItem::Splitter(addr) => addr,
            })
        }
        ActivePane::Bitmap => {
            AlignedGridRange::bitmap(ctx.state).clamped_item_addr(ctx.view.bitmap_cursor_index)
        }
        ActivePane::Disassembly | ActivePane::Debugger => None,
    }
}

/// Jumps to the next or previous `BlockType::Undefined` region in the binary.
fn handle_jump_unexplored(ctx: &mut ActionContext<'_>, forward: bool) {
    use crate::state::BlockType;

    let block_types = &ctx.state.block_types;
    if block_types.is_empty() {
        ctx.events
            .push(CoreEvent::StatusMessage("No data loaded".to_string()));
        return;
    }

    let current_offset = ctx
        .state
        .disassembly
        .get(ctx.view.cursor_index)
        .map(|line| line.address.offset_from(ctx.state.origin))
        .unwrap_or(0);

    let found = if forward {
        let mut i = current_offset;
        let current_type = block_types.get(i).copied().unwrap_or(BlockType::Code);

        while i < block_types.len() && block_types[i] == current_type {
            i += 1;
        }

        while i < block_types.len() {
            if block_types[i] == BlockType::Undefined {
                break;
            }
            i += 1;
        }

        if i < block_types.len() { Some(i) } else { None }
    } else if current_offset == 0 {
        None
    } else {
        let mut i = current_offset - 1;

        if block_types[i] == BlockType::Undefined {
            while i > 0 && block_types[i - 1] == BlockType::Undefined {
                i -= 1;
            }
            if i == 0 {
                ctx.events.push(CoreEvent::StatusMessage(
                    "No previous unexplored block".to_string(),
                ));
                return;
            }
            i -= 1;
        }

        loop {
            if block_types[i] == BlockType::Undefined {
                while i > 0 && block_types[i - 1] == BlockType::Undefined {
                    i -= 1;
                }
                break;
            }
            if i == 0 {
                ctx.events.push(CoreEvent::StatusMessage(
                    "No previous unexplored block".to_string(),
                ));
                return;
            }
            i -= 1;
        }

        Some(i)
    };

    if let Some(offset) = found {
        let target_addr = ctx.state.origin.wrapping_add(offset as u16);
        crate::navigation::perform_jump_to_address(ctx.state, ctx.view, target_addr);
        let direction = if forward { "next" } else { "previous" };
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Jumped to {direction} unexplored block at ${target_addr:04X}"
        )));
        ctx.events.push(CoreEvent::ViewChanged);
    } else {
        let direction = if forward { "next" } else { "previous" };
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "No {direction} unexplored block found"
        )));
    }
}

/// Searches disassembly lines and collapsed blocks in `forward` or reverse order for `last_search_query`.
fn perform_search(ctx: &mut ActionContext<'_>, forward: bool) {
    let query = ctx.view.last_search_query.clone();
    if query.is_empty() {
        ctx.events
            .push(CoreEvent::StatusMessage("No search query".to_string()));
        return;
    }

    let query_lower = query.to_lowercase();
    let disassembly_len = ctx.state.disassembly.len();
    if disassembly_len == 0 {
        return;
    }

    let start_idx = ctx.view.cursor_index;
    use crate::state::search;

    let regex = if ctx.view.search_filters.use_regex {
        match search::compile_regex(&query) {
            Ok(re) => Some(re),
            Err(e) => {
                ctx.events
                    .push(CoreEvent::StatusMessage(format!("Invalid regex: {e}")));
                return;
            }
        }
    } else {
        None
    };

    let hex_pattern = if !ctx.view.search_filters.use_regex && ctx.view.search_filters.hex_bytes {
        search::parse_hex_pattern(&query)
    } else {
        None
    };
    let filters = &ctx.view.search_filters;

    if let Some(line) = ctx.state.disassembly.get(start_idx) {
        let matches = search::get_line_matches(
            line,
            ctx.state,
            &query_lower,
            hex_pattern.as_deref(),
            regex.as_ref(),
            filters,
        );

        let candidate = if forward {
            matches
                .into_iter()
                .find(|&sub| sub > ctx.view.sub_cursor_index)
        } else {
            matches
                .into_iter()
                .rev()
                .find(|&sub| sub < ctx.view.sub_cursor_index)
        };

        if let Some(sub) = candidate {
            ctx.view.navigation_history.push((
                ActivePane::Disassembly,
                crate::view_state::NavigationTarget::Index(ctx.view.cursor_index),
            ));
            ctx.view.sub_cursor_index = sub;
            ctx.events
                .push(CoreEvent::StatusMessage(format!("Found '{query}'")));
            ctx.events.push(CoreEvent::ViewChanged);
            return;
        }
    }

    for i in 1..disassembly_len {
        let idx = if forward {
            (start_idx + i) % disassembly_len
        } else if i <= start_idx {
            start_idx - i
        } else {
            disassembly_len - (i - start_idx)
        };

        if let Some(line) = ctx.state.disassembly.get(idx) {
            let matches = search::get_line_matches(
                line,
                ctx.state,
                &query_lower,
                hex_pattern.as_deref(),
                regex.as_ref(),
                filters,
            );

            if !matches.is_empty() {
                ctx.view.navigation_history.push((
                    ActivePane::Disassembly,
                    crate::view_state::NavigationTarget::Index(ctx.view.cursor_index),
                ));
                ctx.view.cursor_index = idx;
                ctx.view.sub_cursor_index = if forward {
                    matches[0]
                } else {
                    matches[matches.len() - 1]
                };
                ctx.events
                    .push(CoreEvent::StatusMessage(format!("Found '{query}'")));
                ctx.events.push(CoreEvent::ViewChanged);
                return;
            }

            // Collapsed blocks appear as a single summary line at their starting offset (`pc`).
            // Inspect `collapsed_blocks` via `search_collapsed_content` to match hidden lines.
            let pc = line.address.offset_from(ctx.state.origin);
            if ctx
                .state
                .collapsed_blocks
                .iter()
                .find(|(s, _)| *s == pc)
                .copied()
                .is_some_and(|(start, end)| {
                    search::search_collapsed_content(
                        ctx.state,
                        start,
                        end,
                        &query_lower,
                        hex_pattern.as_deref(),
                        regex.as_ref(),
                        filters,
                    )
                })
            {
                ctx.view.navigation_history.push((
                    ActivePane::Disassembly,
                    crate::view_state::NavigationTarget::Index(ctx.view.cursor_index),
                ));
                ctx.view.cursor_index = idx;
                ctx.view.sub_cursor_index = 0;
                ctx.events
                    .push(CoreEvent::StatusMessage(format!("Found '{query}'")));
                ctx.events.push(CoreEvent::ViewChanged);
                return;
            }
        }
    }

    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Search string '{query}' not found"
    )));
}

impl DomainActionHandler for NavigationActionHandler {
    fn handle_action(
        &self,
        action: &AppAction,
        ctx: &mut ActionContext<'_>,
    ) -> Result<bool, CoreError> {
        match action {
            AppAction::Exit => {
                ctx.events.push(CoreEvent::QuitRequested);
                Ok(true)
            }
            AppAction::About => {
                ctx.events
                    .push(CoreEvent::DialogRequested(crate::event::DialogType::About));
                ctx.events.push(CoreEvent::StatusMessage(
                    "About Regenerator 2000".to_string(),
                ));
                Ok(true)
            }
            AppAction::OpenExamples => {
                ctx.events.push(CoreEvent::OpenUrl(
                    "https://regenerator2000.readthedocs.io/en/latest/examples/".to_string(),
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Opening Examples URL...".to_string(),
                ));
                Ok(true)
            }
            AppAction::OpenDocumentation => {
                ctx.events.push(CoreEvent::OpenUrl(
                    "https://regenerator2000.readthedocs.io/en/latest/".to_string(),
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Opening Documentation URL...".to_string(),
                ));
                Ok(true)
            }
            AppAction::KeyboardShortcuts => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::KeyboardShortcuts,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Keyboard Shortcuts".to_string()));
                Ok(true)
            }
            AppAction::SystemSettings => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::Settings,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("System Settings".to_string()));
                Ok(true)
            }
            AppAction::StartMcpServer => {
                if ctx.state.mcp_server_running {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "MCP server is already running.".to_string(),
                    ));
                } else {
                    ctx.events.push(CoreEvent::StartMcpServerRequested);
                }
                Ok(true)
            }
            AppAction::StopMcpServer => {
                if !ctx.state.mcp_server_running {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "MCP server is not running.".to_string(),
                    ));
                } else {
                    ctx.events.push(CoreEvent::StopMcpServerRequested);
                }
                Ok(true)
            }
            AppAction::DocumentSettings => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::DocumentSettings,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Document Settings".to_string()));
                Ok(true)
            }
            AppAction::JumpToAddress => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::JumpToAddress,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Enter address (Hex)".to_string()));
                Ok(true)
            }
            AppAction::JumpToLine => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::JumpToLine,
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Enter Line Number (Dec)".to_string(),
                ));
                Ok(true)
            }
            AppAction::JumpNextUnexplored => {
                handle_jump_unexplored(ctx, true);
                Ok(true)
            }
            AppAction::JumpPrevUnexplored => {
                handle_jump_unexplored(ctx, false);
                Ok(true)
            }
            AppAction::Search => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::Search {
                        query: ctx.view.last_search_query.clone(),
                        filters: ctx.view.search_filters.clone(),
                    },
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Search...".to_string()));
                Ok(true)
            }
            AppAction::GoToSymbol => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::GoToSymbol,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Go to Symbol...".to_string()));
                Ok(true)
            }
            AppAction::ToggleHexDump => {
                use crate::view_state::RightPane;
                toggle_two_column_pane(
                    ctx,
                    ActivePane::HexDump,
                    RightPane::HexDump16,
                    RightPane::HexDump8,
                    |v| &mut v.last_hexdump_pane,
                    "Hex Dump View (16 columns)",
                    "Hex Dump View (8 columns)",
                    "Hex Dump View Hidden",
                );
                Ok(true)
            }
            AppAction::ToggleSpritesView => {
                use crate::view_state::RightPane;
                toggle_two_column_pane(
                    ctx,
                    ActivePane::Sprites,
                    RightPane::Sprites2Col,
                    RightPane::Sprites1Col,
                    |v| &mut v.last_sprites_pane,
                    "Sprites View (2 columns)",
                    "Sprites View (1 column)",
                    "Sprites View Hidden",
                );
                Ok(true)
            }
            AppAction::ToggleCharsetView => {
                use crate::view_state::RightPane;
                toggle_two_column_pane(
                    ctx,
                    ActivePane::Charset,
                    RightPane::Charset8Col,
                    RightPane::Charset4Col,
                    |v| &mut v.last_charset_pane,
                    "Charset View (8 columns)",
                    "Charset View (4 columns)",
                    "Charset View Hidden",
                );
                Ok(true)
            }
            AppAction::ToggleBitmapView => {
                toggle_single_pane(
                    ctx,
                    crate::view_state::RightPane::Bitmap,
                    ActivePane::Bitmap,
                    "Bitmap View Shown",
                    "Bitmap View Hidden",
                );
                Ok(true)
            }
            AppAction::NavigateToAddress(target_addr) => {
                handle_navigate_to_address(ctx, *target_addr);
                Ok(true)
            }
            AppAction::ToggleSplitter => {
                if ctx.view.active_pane == ActivePane::Blocks {
                    let blocks = ctx.state.get_blocks_view_items();
                    if let Some(idx) = ctx.view.blocks_selected_index
                        && idx < blocks.len()
                        && let crate::state::BlockItem::Splitter(addr) = blocks[idx]
                    {
                        let command = crate::commands::Command::ToggleSplitter { address: addr };
                        command.apply(ctx.state);
                        ctx.state.push_command(command);
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Removed splitter at ${addr:04X}"
                        )));
                        ctx.events.push(CoreEvent::StateChanged);
                    }
                } else if ctx.view.active_pane == ActivePane::Disassembly
                    && let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index)
                {
                    let addr = line.address;
                    let command = crate::commands::Command::ToggleSplitter { address: addr };
                    command.apply(ctx.state);
                    ctx.state.push_command(command);
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Toggled splitter at ${addr:04X}"
                    )));
                    ctx.events.push(CoreEvent::StateChanged);
                }
                Ok(true)
            }
            AppAction::FindNext => {
                perform_search(ctx, true);
                Ok(true)
            }
            AppAction::FindPrevious => {
                perform_search(ctx, false);
                Ok(true)
            }
            AppAction::FindReferences => {
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    // Resolve `sub_cursor_index` via `address_at_sub_index`.
                    // Mid-instruction label rows and external equate rows inspect that target address.
                    let raw_addr = line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);
                    // When `raw_addr` carries an active Tier 2 `address_offset` (such as an
                    // internal struct field `struct_base + 1`), `analyzer::analyze` routes all
                    // incoming `XRef`s to `sym.base`, so redirect `Ctrl+x` to `sym.base`.
                    let addr = if crate::disassembler::symbols::has_active_address_offset(
                        raw_addr,
                        &ctx.state.labels,
                        &ctx.state.annotations,
                    ) {
                        ctx.state
                            .annotations
                            .get(raw_addr)
                            .and_then(|e| e.address_offset)
                            .filter(|sym| sym.matches_target(raw_addr))
                            .map_or(raw_addr, |sym| sym.base)
                    } else {
                        raw_addr
                    };
                    ctx.events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::FindReferences(addr),
                    ));
                }
                Ok(true)
            }
            AppAction::ToggleSpriteMulticolor => {
                ctx.view.sprite_multicolor_mode = !ctx.view.sprite_multicolor_mode;
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleCharsetMulticolor => {
                ctx.view.charset_multicolor_mode = !ctx.view.charset_multicolor_mode;
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleBitmapMulticolor => {
                ctx.view.bitmap_multicolor_mode = !ctx.view.bitmap_multicolor_mode;
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleBlocksView => {
                toggle_single_pane(
                    ctx,
                    crate::view_state::RightPane::Blocks,
                    ActivePane::Blocks,
                    "Blocks View Shown",
                    "Blocks View Hidden",
                );
                Ok(true)
            }
            AppAction::ToggleDebuggerView => {
                toggle_single_pane(
                    ctx,
                    crate::view_state::RightPane::Debugger,
                    ActivePane::Debugger,
                    "Debugger View Shown",
                    "Debugger View Hidden",
                );
                Ok(true)
            }
            AppAction::NavigateBack => {
                if let Some((pane, target)) = ctx.view.navigation_history.pop() {
                    ctx.view.active_pane = pane;
                    match target {
                        crate::view_state::NavigationTarget::Address(addr) => {
                            crate::navigation::perform_jump_to_address_no_history(
                                ctx.state,
                                ctx.view,
                                crate::state::Addr(addr),
                            );
                        }
                        crate::view_state::NavigationTarget::Index(idx) => match pane {
                            ActivePane::Disassembly => {
                                let max_idx = ctx.state.disassembly.len().saturating_sub(1);
                                let clamped = idx.min(max_idx);
                                ctx.view.cursor_index = clamped;
                                ctx.view.scroll_index = clamped;
                                ctx.view.scroll_sub_index = 0;
                                ctx.view.sub_cursor_index = 0;
                            }
                            ActivePane::HexDump => {
                                ctx.view.hex_cursor_index = idx;
                            }
                            ActivePane::Sprites => {
                                ctx.view.sprites_cursor_index = idx;
                            }
                            ActivePane::Charset => {
                                ctx.view.charset_cursor_index = idx;
                            }
                            ActivePane::Bitmap => {
                                ctx.view.bitmap_cursor_index = idx;
                            }
                            ActivePane::Blocks => {
                                let max_idx =
                                    ctx.state.get_blocks_view_items().len().saturating_sub(1);
                                ctx.view.blocks_selected_index = Some(idx.min(max_idx));
                            }
                            ActivePane::Debugger => {
                                let max_idx = ctx.state.disassembly.len().saturating_sub(1);
                                ctx.view.cursor_index = idx.min(max_idx);
                            }
                        },
                    }
                    let msg = "Navigated back".to_string();
                    ctx.view.status_message = Some(msg.clone());
                    ctx.events.push(CoreEvent::StatusMessage(msg));
                    ctx.events.push(CoreEvent::ViewChanged);
                } else {
                    let msg = "No history".to_string();
                    ctx.view.status_message = Some(msg.clone());
                    ctx.events.push(CoreEvent::StatusMessage(msg));
                    ctx.events.push(CoreEvent::ViewChanged);
                }
                Ok(true)
            }
            AppAction::JumpToOperand => {
                if ctx.view.active_pane == ActivePane::Disassembly {
                    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
                        ctx.events.push(CoreEvent::StatusMessage(
                            "No valid operand to jump to".to_string(),
                        ));
                        return Ok(true);
                    };

                    // Dispatch by deduplicated destination count: 0 targets report status,
                    // 1 jumps immediately with history, and 2+ open the picker without
                    // mutating cursor position or navigation history until a target is chosen.
                    let mut targets = ctx.state.resolve_disassembly_jump_targets(line);
                    match targets.len() {
                        0 => {
                            ctx.events.push(CoreEvent::StatusMessage(
                                "No valid operand to jump to".to_string(),
                            ));
                        }
                        1 => {
                            if let Some(target) = targets.pop() {
                                handle_navigate_to_address(ctx, target.target_address);
                            }
                        }
                        _ => {
                            let source_address =
                                line.external_label_address.unwrap_or(line.address);
                            ctx.events.push(CoreEvent::DialogRequested(
                                crate::event::DialogType::JumpToOperand {
                                    source_address,
                                    targets,
                                },
                            ));
                        }
                    }
                    return Ok(true);
                }

                if let Some(addr) = resolve_non_disassembly_jump_target(ctx) {
                    handle_navigate_to_address(ctx, addr);
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "No valid operand to jump to".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::CyclePane => {
                ctx.view.active_pane = match ctx.view.active_pane {
                    ActivePane::Disassembly => match ctx.view.right_pane {
                        crate::view_state::RightPane::None => ActivePane::Disassembly,
                        crate::view_state::RightPane::HexDump16
                        | crate::view_state::RightPane::HexDump8 => ActivePane::HexDump,
                        crate::view_state::RightPane::Sprites2Col
                        | crate::view_state::RightPane::Sprites1Col => ActivePane::Sprites,
                        crate::view_state::RightPane::Charset8Col
                        | crate::view_state::RightPane::Charset4Col => ActivePane::Charset,
                        crate::view_state::RightPane::Bitmap => ActivePane::Bitmap,
                        crate::view_state::RightPane::Blocks => ActivePane::Blocks,
                        crate::view_state::RightPane::Debugger => ActivePane::Debugger,
                    },
                    ActivePane::HexDump
                    | ActivePane::Sprites
                    | ActivePane::Charset
                    | ActivePane::Bitmap
                    | ActivePane::Blocks
                    | ActivePane::Debugger => ActivePane::Disassembly,
                };
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::HexdumpViewModeNext => {
                use crate::state::types::HexdumpViewMode;
                ctx.view.hexdump_view_mode = match ctx.view.hexdump_view_mode {
                    HexdumpViewMode::ScreencodeShifted => HexdumpViewMode::ScreencodeUnshifted,
                    HexdumpViewMode::ScreencodeUnshifted => HexdumpViewMode::PETSCIIShifted,
                    HexdumpViewMode::PETSCIIShifted => HexdumpViewMode::PETSCIIUnshifted,
                    HexdumpViewMode::PETSCIIUnshifted => HexdumpViewMode::ScreencodeShifted,
                };
                ctx.events.push(CoreEvent::StatusMessage(format!(
                    "Hex Dump Mode: {:?}",
                    ctx.view.hexdump_view_mode
                )));
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::HexdumpViewModePrev => {
                use crate::state::types::HexdumpViewMode;
                ctx.view.hexdump_view_mode = match ctx.view.hexdump_view_mode {
                    HexdumpViewMode::ScreencodeShifted => HexdumpViewMode::PETSCIIUnshifted,
                    HexdumpViewMode::ScreencodeUnshifted => HexdumpViewMode::ScreencodeShifted,
                    HexdumpViewMode::PETSCIIShifted => HexdumpViewMode::ScreencodeUnshifted,
                    HexdumpViewMode::PETSCIIUnshifted => HexdumpViewMode::PETSCIIShifted,
                };
                ctx.events.push(CoreEvent::StatusMessage(format!(
                    "Hex Dump Mode: {:?}",
                    ctx.view.hexdump_view_mode
                )));
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::Cancel => {
                if ctx.view.is_visual_mode {
                    ctx.view.is_visual_mode = false;
                    ctx.view.selection_start = None;
                    ctx.events
                        .push(CoreEvent::StatusMessage("Visual Mode Exited".to_string()));
                } else if ctx.view.selection_start.is_some() {
                    ctx.view.selection_start = None;
                    ctx.events
                        .push(CoreEvent::StatusMessage("Selection cleared".to_string()));
                }
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::state::types::HexdumpViewMode;
    use crate::view_state::CoreViewState;

    #[test]
    fn test_cycle_hexdump_view_mode() {
        let mut state = AppState::new();
        let mut view = CoreViewState::new();
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();

        assert_eq!(
            ctx.view.hexdump_view_mode,
            HexdumpViewMode::ScreencodeShifted
        );

        // Next
        assert!(
            handler
                .handle_action(&AppAction::HexdumpViewModeNext, &mut ctx)
                .unwrap()
        );
        assert_eq!(
            ctx.view.hexdump_view_mode,
            HexdumpViewMode::ScreencodeUnshifted
        );

        assert!(
            handler
                .handle_action(&AppAction::HexdumpViewModeNext, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.hexdump_view_mode, HexdumpViewMode::PETSCIIShifted);

        assert!(
            handler
                .handle_action(&AppAction::HexdumpViewModeNext, &mut ctx)
                .unwrap()
        );
        assert_eq!(
            ctx.view.hexdump_view_mode,
            HexdumpViewMode::PETSCIIUnshifted
        );

        assert!(
            handler
                .handle_action(&AppAction::HexdumpViewModeNext, &mut ctx)
                .unwrap()
        );
        assert_eq!(
            ctx.view.hexdump_view_mode,
            HexdumpViewMode::ScreencodeShifted
        );

        // Prev
        assert!(
            handler
                .handle_action(&AppAction::HexdumpViewModePrev, &mut ctx)
                .unwrap()
        );
        assert_eq!(
            ctx.view.hexdump_view_mode,
            HexdumpViewMode::PETSCIIUnshifted
        );

        assert!(
            handler
                .handle_action(&AppAction::HexdumpViewModePrev, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.hexdump_view_mode, HexdumpViewMode::PETSCIIShifted);
    }

    #[test]
    fn test_jump_to_operand_absolute() {
        use crate::cpu::{AddressingMode, Opcode};
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0x4C, 0x34, 0x12, 0xEA];
        state.block_types = vec![crate::state::BlockType::Code; 4];

        let line0 = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x4C, 0x34, 0x12],
            mnemonic: "JMP".to_string(),
            operand: "$1234".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "JMP",
                AddressingMode::Absolute,
                3,
                3,
                "Jump to address",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let line1 = DisassemblyLine {
            address: Addr(0x1234),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "NOP",
                AddressingMode::Implied,
                1,
                2,
                "No operation",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        state.disassembly = vec![line0, line1];

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Disassembly;
        view.cursor_index = 0;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();
        let handled = handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert!(handled);
        assert_eq!(ctx.view.cursor_index, 1);
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x1234)
        );
    }

    #[test]
    fn test_jump_to_operand_relative() {
        use crate::cpu::{AddressingMode, Opcode};
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        let line0 = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0xD0, 0x05],
            mnemonic: "BNE".to_string(),
            operand: "$1007".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "BNE",
                AddressingMode::Relative,
                2,
                2,
                "Branch if not equal",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let line1 = DisassemblyLine {
            address: Addr(0x1007),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "NOP",
                AddressingMode::Implied,
                1,
                2,
                "No operation",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        state.disassembly = vec![line0, line1];

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Disassembly;
        view.cursor_index = 0;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();
        let handled = handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert!(handled);
        assert_eq!(ctx.view.cursor_index, 1);
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x1007)
        );
    }

    #[test]
    fn test_jump_to_operand_immediate_formatted() {
        use crate::cpu::{AddressingMode, Opcode};
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        let line0 = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0xA9, 0x00],
            mnemonic: "LDA".to_string(),
            operand: "#<target".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "LDA",
                AddressingMode::Immediate,
                2,
                2,
                "Load accumulator",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let line1 = DisassemblyLine {
            address: Addr(0x2000),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "NOP",
                AddressingMode::Implied,
                1,
                2,
                "No operation",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        state.disassembly = vec![line0, line1];
        state.annotations.update(Addr(0x1000), |e| {
            e.immediate_format = Some(crate::state::ImmediateFormat::LowByte(Addr(0x2000)));
        });

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Disassembly;
        view.cursor_index = 0;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();
        let handled = handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert!(handled);
        assert_eq!(ctx.view.cursor_index, 1);
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x2000)
        );
        assert_eq!(ctx.view.navigation_history.len(), 1);
    }

    #[test]
    fn test_jump_twice_and_navigate_back_twice() {
        use crate::cpu::{AddressingMode, Opcode};
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        let line0 = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x4C, 0x00, 0x20],
            mnemonic: "JMP".to_string(),
            operand: "$2000".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "JMP",
                AddressingMode::Absolute,
                3,
                3,
                "Jump to address",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let line1 = DisassemblyLine {
            address: Addr(0x2000),
            bytes: vec![0x4C, 0x00, 0x30],
            mnemonic: "JMP".to_string(),
            operand: "$3000".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "JMP",
                AddressingMode::Absolute,
                3,
                3,
                "Jump to address",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let line2 = DisassemblyLine {
            address: Addr(0x3000),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: Some(Opcode::new(
                "NOP",
                AddressingMode::Implied,
                1,
                2,
                "No operation",
            )),
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        state.disassembly = vec![line0, line1, line2];

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Disassembly;
        view.cursor_index = 0;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();

        // Initial state at $1000
        assert_eq!(ctx.view.cursor_index, 0);
        assert_eq!(ctx.view.navigation_history.len(), 0);

        // Jump 1: Enter on JMP $2000 -> moves to index 1 ($2000)
        assert!(
            handler
                .handle_action(&AppAction::JumpToOperand, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.cursor_index, 1);
        assert_eq!(ctx.view.navigation_history.len(), 1);

        // Jump 2: Enter on JMP $3000 -> moves to index 2 ($3000)
        assert!(
            handler
                .handle_action(&AppAction::JumpToOperand, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.cursor_index, 2);
        assert_eq!(ctx.view.navigation_history.len(), 2);

        // Back 1: Backspace -> returns to index 1 ($2000)
        assert!(
            handler
                .handle_action(&AppAction::NavigateBack, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.cursor_index, 1);
        assert_eq!(ctx.view.navigation_history.len(), 1);

        // Back 2: Backspace -> returns to index 0 ($1000)
        assert!(
            handler
                .handle_action(&AppAction::NavigateBack, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.cursor_index, 0);
        assert_eq!(ctx.view.navigation_history.len(), 0);

        // Back 3: Backspace with empty history -> stays at index 0 ($1000)
        assert!(
            handler
                .handle_action(&AppAction::NavigateBack, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.cursor_index, 0);
        assert_eq!(ctx.view.status_message.as_deref(), Some("No history"));
    }

    #[test]
    fn test_navigate_back_to_hexdump_pane() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA; 64];
        state.block_types = vec![crate::state::BlockType::Code; 64];
        state.disassemble();

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::HexDump;
        view.hex_cursor_index = 3;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();

        // Jump to disassembly address from HexDump
        assert!(
            handler
                .handle_action(&AppAction::NavigateToAddress(Addr(0x1000)), &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.active_pane, ActivePane::Disassembly);
        assert_eq!(ctx.view.navigation_history.len(), 1);

        // Navigate back -> restores HexDump pane and hex_cursor_index
        assert!(
            handler
                .handle_action(&AppAction::NavigateBack, &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.active_pane, ActivePane::HexDump);
        assert_eq!(ctx.view.hex_cursor_index, 3);
        assert_eq!(ctx.view.navigation_history.len(), 0);
    }

    #[test]
    fn test_hexdump8_sync_and_jump_to_operand_with_unaligned_origin() {
        let mut state = AppState::new();
        state.load_binary(Addr(0x0801), vec![0xEA; 32]).unwrap();

        let mut view = CoreViewState::new();
        // Put cursor at $0810 (index 15 -> offset 15, plus padding 1 = 16 -> row 1 in 16-col, row 2 in 8-col)
        view.cursor_index = state.get_line_index_for_address(Addr(0x0810)).unwrap();
        view.sub_cursor_index = 2;

        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();
        // Default right_pane is HexDump16, so first ToggleHexDump switches to HexDump8 (row 2)
        handler
            .handle_action(&AppAction::ToggleHexDump, &mut ctx)
            .unwrap();
        assert_eq!(ctx.view.right_pane, crate::view_state::RightPane::HexDump8);
        assert_eq!(ctx.view.hex_cursor_index, 2);
        assert_eq!(ctx.view.sub_cursor_index, 2);

        // JumpToOperand from HexDump8 row 2 -> $0810
        ctx.view.cursor_index = 0;
        handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x0810)
        );

        // JumpToOperand from HexDump8 row 0 (padded $0800..$0807) clamps to origin $0801
        ctx.view.active_pane = ActivePane::HexDump;
        ctx.view.hex_cursor_index = 0;
        handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x0801)
        );
    }

    #[test]
    fn test_navigate_back_clamps_index_when_disassembly_shrinks() {
        let mut state = AppState::new();
        state.load_binary(Addr(0x1000), vec![0xEA; 4]).unwrap();

        let mut view = CoreViewState::new();
        view.navigation_history.push((
            ActivePane::Disassembly,
            crate::view_state::NavigationTarget::Index(99),
        ));

        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();
        handler
            .handle_action(&AppAction::NavigateBack, &mut ctx)
            .unwrap();
        assert_eq!(ctx.view.cursor_index, ctx.state.disassembly.len() - 1);
        assert!(
            ctx.events
                .contains(&CoreEvent::StatusMessage("Navigated back".to_string()))
        );
    }

    #[test]
    fn test_jump_to_operand_redirects_to_symbolic_offset_base_across_all_line_kinds() {
        use crate::disassembler::DisassemblyLine;
        use crate::state::{
            BlockType, ImmediateFormat, Label, LabelKind, LabelType, OffsetRadix, SymbolicOffset,
        };

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // Layout ($1000..=$1040, 0x41 bytes):
        // $1000 (3): BD 2F 10          LDA $102F,X (Tier 1 -> sub_base $1030 - 1)
        // $1003 (3): AD 3A 10          LDA $103A   (Tier 2 on $103A -> struct_base $1038 + 2)
        // $1006 (2): A9 2F             LDA #$2F    (LowByte($102F) + Tier 1 -> sub_base $1030 - 1)
        // $1008 (2): 2F 10             .word $102F (BlockType::Address + Tier 1 -> sub_base $1030 - 1)
        // $100A (4): 2F 2F 10 10       LoHiAddress (2 pairs: $102F, $102F -> Tier 1 on $100A)
        // $100E..=$102F: NOPs / padding
        // $1030: EA 60                 sub_base
        // $1038..=$103B: 00 00 00 00   struct_base
        let mut raw = vec![0xEA; 0x41];
        raw[0x00..0x03].copy_from_slice(&[0xBD, 0x2F, 0x10]);
        raw[0x03..0x06].copy_from_slice(&[0xAD, 0x3A, 0x10]);
        raw[0x06..0x08].copy_from_slice(&[0xA9, 0x2F]);
        raw[0x08..0x0A].copy_from_slice(&[0x2F, 0x10]);
        raw[0x0A..0x0E].copy_from_slice(&[0x2F, 0x2F, 0x10, 0x10]);
        raw[0x31] = 0x60;
        raw[0x38..0x3C].copy_from_slice(&[0x00; 4]);
        state.raw_data = raw;

        let mut btypes = vec![BlockType::Code; 0x41];
        btypes[0x08] = BlockType::Address;
        btypes[0x09] = BlockType::Address;
        for b in &mut btypes[0x0A..0x0E] {
            *b = BlockType::LoHiAddress;
        }
        for b in &mut btypes[0x38..0x3C] {
            *b = BlockType::DataByte;
        }
        state.block_types = btypes;

        state.labels.insert(
            Addr(0x1030),
            vec![Label {
                name: "sub_base".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.labels.insert(
            Addr(0x1038),
            vec![Label {
                name: "struct_base".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        let sub_minus_1 = SymbolicOffset {
            base: Addr(0x1030),
            offset: -1,
            radix: OffsetRadix::Decimal,
        };
        for site in [
            Addr(0x1000),
            Addr(0x1006),
            Addr(0x1008),
            Addr(0x100A),
            Addr(0x100B),
        ] {
            state.annotations.update(site, |e| {
                e.operand_offset = Some(sub_minus_1);
            });
        }
        state.annotations.update(Addr(0x1006), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0x102F)));
        });
        state.annotations.update(Addr(0x103A), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1038),
                offset: 2,
                radix: OffsetRadix::Decimal,
            });
        });

        // External equate ($0FFF -> base $1000 - 1)
        state.annotations.update(Addr(0x0FFF), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1000),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });

        state.perform_analysis();

        // Prepend synthetic external equate row for $0FFF to verify external equate JumpToOperand
        state.disassembly.insert(
            0,
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "ext_0fff = $0fff".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: Some(Addr(0x0FFF)),
                is_collapsed: false,
            },
        );

        let find_body_line = |s: &AppState, addr: Addr| -> usize {
            s.disassembly
                .iter()
                .position(|l| {
                    l.external_label_address.is_none() && l.address == addr && !l.bytes.is_empty()
                })
                .unwrap_or_else(|| panic!("body line for ${addr:04X} must exist"))
        };

        let cases = [
            (0usize, Addr(0x1000), "external equate $0FFF -> $1000"),
            (
                find_body_line(&state, Addr(0x1000)),
                Addr(0x1030),
                "LDA sub_base - 1,X",
            ),
            (
                find_body_line(&state, Addr(0x1003)),
                Addr(0x1038),
                "LDA struct_base + 2 (Tier 2)",
            ),
            (
                find_body_line(&state, Addr(0x1006)),
                Addr(0x1030),
                "LDA #<(sub_base - 1)",
            ),
            (
                find_body_line(&state, Addr(0x1008)),
                Addr(0x1030),
                ".word sub_base - 1",
            ),
            (
                find_body_line(&state, Addr(0x100A)),
                Addr(0x1030),
                "LoHiAddress first half (.byte <...)",
            ),
            (
                find_body_line(&state, Addr(0x100C)),
                Addr(0x1030),
                "LoHiAddress second half (.byte >...)",
            ),
        ];

        let handler = NavigationActionHandler::new();
        for (start_idx, expected_base, desc) in cases {
            let mut view = CoreViewState::new();
            view.active_pane = ActivePane::Disassembly;
            view.cursor_index = start_idx;
            let mut events = Vec::new();
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };

            assert!(
                handler
                    .handle_action(&AppAction::JumpToOperand, &mut ctx)
                    .unwrap(),
                "{desc}"
            );
            assert_eq!(
                ctx.state.disassembly[ctx.view.cursor_index].address, expected_base,
                "JumpToOperand failed for {desc}"
            );
            assert_eq!(
                ctx.view.navigation_history.len(),
                1,
                "navigation history must be recorded for {desc}"
            );

            // Backspace returns to the originating line
            handler
                .handle_action(&AppAction::NavigateBack, &mut ctx)
                .unwrap();
            assert_eq!(
                ctx.view.cursor_index, start_idx,
                "NavigateBack failed for {desc}"
            );
        }
    }

    #[test]
    fn test_find_references_on_tier2_struct_field_redirects_to_base() {
        use crate::event::DialogType;
        use crate::state::{BlockType, Label, LabelKind, LabelType, OffsetRadix, SymbolicOffset};

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: AD 39 10 (LDA $1039), $1038..=$1039: struct bytes
        let mut raw = vec![0xEA; 0x40];
        raw[0..3].copy_from_slice(&[0xAD, 0x39, 0x10]);
        raw[0x38] = 0x00;
        raw[0x39] = 0x00;
        state.raw_data = raw;

        let mut btypes = vec![BlockType::Code; 0x40];
        btypes[0x38] = BlockType::DataByte;
        btypes[0x39] = BlockType::DataByte;
        state.block_types = btypes;
        state.settings.bytes_per_line = 1;

        state.labels.insert(
            Addr(0x1038),
            vec![Label {
                name: "struct_base".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );
        state.annotations.update(Addr(0x1039), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1038),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        state.perform_analysis();

        let field_line_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1039) && !l.bytes.is_empty())
            .expect("$1039 struct field line must exist");

        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Disassembly;
        view.cursor_index = field_line_idx;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler::new();
        assert!(
            handler
                .handle_action(&AppAction::FindReferences, &mut ctx)
                .unwrap()
        );
        assert!(
            ctx.events
                .contains(&CoreEvent::DialogRequested(DialogType::FindReferences(
                    Addr(0x1038)
                ))),
            "FindReferences on Tier 2 struct field $1039 must redirect to base $1038, got: {:?}",
            ctx.events
        );
    }

    #[test]
    fn test_jump_to_operand_ignores_plain_external_equate_and_jumps_from_external_file_equate() {
        use crate::state::{BlockType, Label, LabelKind, LabelType};

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.all_labels = true;
        // $1000: JSR $FFD2 (20 D2 FF), $1003: RTS (60), $1004..=$1007: ExternalFile
        state.raw_data = vec![0x20, 0xD2, 0xFF, 0x60, 0xAA, 0xBB, 0xCC, 0xDD];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
        ];
        state.labels.insert(
            Addr(0xFFD2),
            vec![Label {
                name: "CHROUT".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );
        state.labels.insert(
            Addr(0x1004),
            vec![Label {
                name: "music_init".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.disassemble();

        let chrout_idx = state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0xFFD2)))
            .expect("CHROUT preamble equate must exist");
        let ext_file_eq_idx = state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x1004)))
            .expect("ExternalFile preamble equate must exist");

        let handler = NavigationActionHandler::new();

        // 1. Enter on plain external equate `CHROUT = $ffd2` is a no-op with zero history push.
        let mut view = CoreViewState::new();
        view.active_pane = ActivePane::Disassembly;
        view.cursor_index = chrout_idx;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };
        handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert_eq!(ctx.view.cursor_index, chrout_idx);
        assert!(ctx.view.navigation_history.is_empty());

        // 2. Enter on `ExternalFile` equate `music_init = $1004` jumps to the body line at $1004.
        ctx.view.cursor_index = ext_file_eq_idx;
        ctx.events.clear();
        handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        let landed = &ctx.state.disassembly[ctx.view.cursor_index];
        assert!(landed.external_label_address.is_none());
        assert_eq!(landed.address, Addr(0x1004));
        assert_eq!(ctx.view.navigation_history.len(), 1);
    }

    #[test]
    fn test_jump_to_operand_multi_target_split_table_emits_dialog() {
        use crate::event::DialogType;
        use crate::state::BlockType;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..$1004: 2-entry LoHiAddress table targeting $100A and $100C.
        // $1004..$100A: 6 NOPs.
        // $100A..$100C: LDA #$01; $100C..$100E: LDA #$02; $100E: RTS.
        let mut raw = vec![0x0A, 0x0C, 0x10, 0x10];
        raw.extend(std::iter::repeat_n(0xEA, 6));
        raw.extend_from_slice(&[0xA9, 0x01, 0xA9, 0x02, 0x60]);
        state.raw_data = raw;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        for bt in &mut state.block_types[0..4] {
            *bt = BlockType::LoHiAddress;
        }
        state.disassemble();

        let lo_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        let hi_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();

        let handler = NavigationActionHandler;

        for (idx, expected_src) in [(lo_idx, Addr(0x1000)), (hi_idx, Addr(0x1002))] {
            let mut view = CoreViewState::new();
            view.cursor_index = idx;
            let mut events = Vec::new();
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };

            assert!(
                handler
                    .handle_action(&AppAction::JumpToOperand, &mut ctx)
                    .unwrap()
            );
            assert_eq!(ctx.view.cursor_index, idx);
            assert!(ctx.view.navigation_history.is_empty());

            let dialog = ctx.events.iter().find_map(|ev| match ev {
                CoreEvent::DialogRequested(DialogType::JumpToOperand {
                    source_address,
                    targets,
                }) => Some((*source_address, targets.clone())),
                _ => None,
            });
            let (src, targets) = dialog.expect("expected JumpToOperand dialog request");
            assert_eq!(src, expected_src);
            let addrs: Vec<Addr> = targets.iter().map(|t| t.target_address).collect();
            assert_eq!(addrs, vec![Addr(0x100A), Addr(0x100C)]);
        }

        // Selecting a target from the picker via NavigateToAddress moves cursor and pushes history.
        let mut view = CoreViewState::new();
        view.cursor_index = lo_idx;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };
        assert!(
            handler
                .handle_action(&AppAction::NavigateToAddress(Addr(0x100C)), &mut ctx)
                .unwrap()
        );
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x100C)
        );
        assert_eq!(ctx.view.navigation_history.len(), 1);
    }

    #[test]
    fn test_jump_to_operand_multi_word_vs_dedup_single_jumps_or_opens_dialog() {
        use crate::event::DialogType;
        use crate::state::BlockType;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..$1004: .word $1010, $1014 (2 distinct targets -> dialog)
        // $1004..$1008: .word $1010, $1010 (deduplicates to 1 target -> direct jump)
        let mut raw = vec![0x10, 0x10, 0x14, 0x10, 0x10, 0x10, 0x10, 0x10];
        raw.extend(std::iter::repeat_n(0xEA, 16));
        state.raw_data = raw;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        for bt in &mut state.block_types[0..8] {
            *bt = BlockType::Address;
        }
        state.splitters.insert(Addr(0x1004));
        state.disassemble();

        let multi_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        let dedup_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1004) && !l.bytes.is_empty())
            .unwrap();

        let handler = NavigationActionHandler;

        // Multi-target .word emits JumpToOperand dialog with no history push.
        let mut view = CoreViewState::new();
        view.cursor_index = multi_idx;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };
        handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert_eq!(ctx.view.cursor_index, multi_idx);
        assert!(ctx.view.navigation_history.is_empty());
        assert!(ctx.events.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(DialogType::JumpToOperand {
                source_address: Addr(0x1000),
                targets,
            }) if targets.iter().map(|t| t.target_address).collect::<Vec<_>>() == vec![Addr(0x1010), Addr(0x1014)]
        )));

        // Duplicate-target .word deduplicates to 1 target and jumps directly.
        let mut view = CoreViewState::new();
        view.cursor_index = dedup_idx;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };
        handler
            .handle_action(&AppAction::JumpToOperand, &mut ctx)
            .unwrap();
        assert_eq!(
            ctx.state.disassembly[ctx.view.cursor_index].address,
            Addr(0x1010)
        );
        assert_eq!(ctx.view.navigation_history.len(), 1);
        assert!(
            !ctx.events
                .iter()
                .any(|ev| matches!(ev, CoreEvent::DialogRequested(_)))
        );
    }

    #[test]
    fn test_jump_to_operand_zero_targets_and_oob_cursor_shows_status() {
        use crate::state::BlockType;

        let handler = NavigationActionHandler;

        // 1. Line with zero targets (`NOP`).
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA, 0x60];
        state.block_types = vec![BlockType::Code; 2];
        state.disassemble();

        let nop_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();

        for cursor in [nop_idx, usize::MAX] {
            let mut view = CoreViewState::new();
            view.cursor_index = cursor;
            let mut events = Vec::new();
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            assert!(
                handler
                    .handle_action(&AppAction::JumpToOperand, &mut ctx)
                    .unwrap()
            );
            assert!(ctx.view.navigation_history.is_empty());
            assert!(ctx.events.contains(&CoreEvent::StatusMessage(
                "No valid operand to jump to".to_string()
            )));
            assert!(
                !ctx.events
                    .iter()
                    .any(|ev| matches!(ev, CoreEvent::DialogRequested(_)))
            );
        }

        // 2. Empty disassembly does not panic and emits status message.
        let mut empty_state = AppState::new();
        let mut view = CoreViewState::new();
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut empty_state,
            view: &mut view,
            events: &mut events,
        };
        assert!(
            handler
                .handle_action(&AppAction::JumpToOperand, &mut ctx)
                .unwrap()
        );
        assert!(ctx.events.contains(&CoreEvent::StatusMessage(
            "No valid operand to jump to".to_string()
        )));
    }

    #[test]
    fn test_jump_to_operand_non_disassembly_panes_jump_without_dialog() {
        use crate::state::BlockType;

        let mut state = AppState::new();
        state.origin = Addr(0x2000);
        state.raw_data = vec![0xEA; 0x2000];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.disassemble();

        let handler = NavigationActionHandler;

        let cases = [
            (ActivePane::HexDump, Addr(0x2010)),
            (ActivePane::Sprites, Addr(0x2040)),
            (ActivePane::Charset, Addr(0x2008)),
            (ActivePane::Bitmap, Addr(0x2000)),
            (ActivePane::Blocks, Addr(0x2000)),
        ];

        for (pane, expected_addr) in cases {
            let mut view = CoreViewState::new();
            view.active_pane = pane;
            view.hex_cursor_index = 1;
            view.sprites_cursor_index = 1;
            view.charset_cursor_index = 1;
            view.bitmap_cursor_index = 0;
            view.blocks_selected_index = Some(0);
            let mut events = Vec::new();
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };

            assert!(
                handler
                    .handle_action(&AppAction::JumpToOperand, &mut ctx)
                    .unwrap()
            );
            assert_eq!(ctx.view.active_pane, ActivePane::Disassembly);
            assert_eq!(
                ctx.state.disassembly[ctx.view.cursor_index].address,
                expected_addr
            );
            assert_eq!(ctx.view.navigation_history.len(), 1);
            assert_eq!(ctx.view.navigation_history[0].0, pane);
            assert!(
                !ctx.events
                    .iter()
                    .any(|ev| matches!(ev, CoreEvent::DialogRequested(_)))
            );
        }
    }

    #[test]
    fn test_navigate_to_address_out_of_range_shows_status_and_keeps_history() {
        use crate::state::BlockType;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA; 8];
        state.block_types = vec![BlockType::Code; 8];
        state.disassemble();

        let start_idx = state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();

        let mut view = CoreViewState::new();
        view.cursor_index = start_idx;
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = NavigationActionHandler;
        assert!(
            handler
                .handle_action(&AppAction::NavigateToAddress(Addr(0xFFD2)), &mut ctx)
                .unwrap()
        );
        assert_eq!(ctx.view.cursor_index, start_idx);
        assert!(ctx.view.navigation_history.is_empty());
        assert!(ctx.events.contains(&CoreEvent::StatusMessage(
            "Address $FFD2 out of range ($1000-$1007)".to_string()
        )));
    }

    #[test]
    fn test_toggle_views_respect_sync_config_and_floor_alignment() {
        use crate::state::BlockType;
        use crate::view_state::RightPane;

        let mut state = AppState::new();
        state.origin = Addr(0x0801);
        state.raw_data = vec![0xEA; 0x6000 - 0x0801];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.disassemble();

        let handler = NavigationActionHandler;
        let idx_0801 = state
            .get_line_index_containing_address(Addr(0x0801))
            .unwrap();
        let idx_0815 = state
            .get_line_index_containing_address(Addr(0x0815))
            .unwrap();
        let idx_0840 = state
            .get_line_index_containing_address(Addr(0x0840))
            .unwrap();
        let idx_0900 = state
            .get_line_index_containing_address(Addr(0x0900))
            .unwrap();
        let idx_2000 = state
            .get_line_index_containing_address(Addr(0x2000))
            .unwrap();

        // 1. ToggleBitmapView respects sync_bitmap_view and uses floor alignment.
        let mut view = CoreViewState::new();
        view.bitmap_cursor_index = 2;
        view.cursor_index = idx_2000;
        state.system_config.sync_bitmap_view = false;
        let mut events = Vec::new();
        {
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            handler
                .handle_action(&AppAction::ToggleBitmapView, &mut ctx)
                .unwrap();
        }
        assert_eq!(view.right_pane, RightPane::Bitmap);
        assert_eq!(view.bitmap_cursor_index, 2);

        // Toggle off, enable sync_bitmap_view, and reopen at $2000 -> slot 1; at $0900 -> slot 0.
        state.system_config.sync_bitmap_view = true;
        {
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            handler
                .handle_action(&AppAction::ToggleBitmapView, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.right_pane, RightPane::None);
            handler
                .handle_action(&AppAction::ToggleBitmapView, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.bitmap_cursor_index, 1);

            handler
                .handle_action(&AppAction::ToggleBitmapView, &mut ctx)
                .unwrap();
            ctx.view.cursor_index = idx_0900;
            handler
                .handle_action(&AppAction::ToggleBitmapView, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.bitmap_cursor_index, 0);
        }

        // 2. ToggleSpritesView respects sync_sprites_view and uses floor alignment ($0800 base).
        view.right_pane = RightPane::None;
        view.sprites_cursor_index = 5;
        view.cursor_index = idx_0840;
        state.system_config.sync_sprites_view = false;
        {
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            handler
                .handle_action(&AppAction::ToggleSpritesView, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.sprites_cursor_index, 5);
        }
        view.right_pane = RightPane::None;
        state.system_config.sync_sprites_view = true;
        {
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            ctx.view.cursor_index = idx_0801;
            handler
                .handle_action(&AppAction::ToggleSpritesView, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.sprites_cursor_index, 0);

            ctx.view.right_pane = RightPane::None;
            ctx.view.cursor_index = idx_0840;
            handler
                .handle_action(&AppAction::ToggleSpritesView, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.sprites_cursor_index, 1);
        }

        // 3. ToggleHexDump updates hex_col_cursor when synced, and preserves byte addr when unsynced.
        state.system_config.sync_hex_dump = true;
        view.right_pane = RightPane::HexDump16;
        view.cursor_index = idx_0815; // $0815 -> in HexDump8: row 2 ($0810), col 5
        {
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            handler
                .handle_action(&AppAction::ToggleHexDump, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.right_pane, RightPane::HexDump8);
            assert_eq!(ctx.view.hex_cursor_index, 2);
            assert_eq!(ctx.view.hex_col_cursor, 5);
        }

        state.system_config.sync_hex_dump = false;
        view.right_pane = RightPane::HexDump16;
        view.active_pane = ActivePane::HexDump;
        view.hex_cursor_index = 1; // $0810 + 11 = $081B
        view.hex_col_cursor = 11;
        view.cursor_index = idx_0801;
        {
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut view,
                events: &mut events,
            };
            handler
                .handle_action(&AppAction::ToggleHexDump, &mut ctx)
                .unwrap();
            assert_eq!(ctx.view.right_pane, RightPane::HexDump8);
            assert_eq!(ctx.view.hex_cursor_index, 3); // $0818
            assert_eq!(ctx.view.hex_col_cursor, 3); // $0818 + 3 = $081B
        }

        // 4. JumpToOperand from Bitmap and Sprites on unaligned origin $0801 uses floor alignment + clamping.
        let jump_cases = [
            (ActivePane::Bitmap, 0, Addr(0x0801)),
            (ActivePane::Bitmap, 1, Addr(0x2000)),
            (ActivePane::Sprites, 0, Addr(0x0801)),
            (ActivePane::Sprites, 1, Addr(0x0840)),
        ];
        for (pane, idx, expected) in jump_cases {
            let mut jump_view = CoreViewState {
                active_pane: pane,
                bitmap_cursor_index: idx,
                sprites_cursor_index: idx,
                ..CoreViewState::new()
            };
            let mut ctx = ActionContext {
                state: &mut state,
                view: &mut jump_view,
                events: &mut events,
            };
            handler
                .handle_action(&AppAction::JumpToOperand, &mut ctx)
                .unwrap();
            assert_eq!(
                ctx.state.disassembly[ctx.view.cursor_index].address,
                expected
            );
        }
    }
}
