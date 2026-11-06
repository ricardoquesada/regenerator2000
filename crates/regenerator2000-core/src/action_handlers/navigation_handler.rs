//! Navigation domain action handler for cursor movement, pane toggling, search, address jumps, and settings.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::cpu::AddressingMode;
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

/// Synchronizes the active right-pane cursor with the current disassembly cursor address.
fn sync_pane_cursor_to_disassembly(ctx: &mut ActionContext<'_>) {
    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
        return;
    };
    let addr = line.address;
    let is_splitter = line.is_splitter();
    match ctx.view.active_pane {
        ActivePane::HexDump => {
            let origin = ctx.state.origin.0 as usize;
            let addr_val = addr.0 as usize;
            if addr_val >= origin {
                let offset = addr_val - origin;
                if offset < ctx.state.raw_data.len() {
                    let bytes_per_row =
                        if ctx.view.right_pane == crate::view_state::RightPane::HexDump8 {
                            8
                        } else {
                            16
                        };
                    let alignment_padding = origin % bytes_per_row;
                    ctx.view.hex_cursor_index = (offset + alignment_padding) / bytes_per_row;
                }
            }
        }
        ActivePane::Sprites => {
            let origin = ctx.state.origin.0;
            let padding = (64 - (origin % 64)) % 64;
            let first_aligned = origin.wrapping_add(padding);
            if addr.0 >= first_aligned {
                let offset = (addr.0 - first_aligned) as usize;
                ctx.view.sprites_cursor_index = offset / 64;
            }
        }
        ActivePane::Charset => {
            let origin = ctx.state.origin.0;
            let aligned_start = (origin / 1024) * 1024;
            if addr.0 >= aligned_start {
                let offset = (addr.0 - aligned_start) as usize;
                ctx.view.charset_cursor_index = offset / 8;
            }
        }
        ActivePane::Bitmap => {
            let origin = ctx.state.origin.0 as usize;
            let first_aligned = origin.div_ceil(8192) * 8192;
            if (addr.0 as usize) >= first_aligned {
                let offset = (addr.0 as usize) - first_aligned;
                ctx.view.bitmap_cursor_index = offset / 8192;
            }
        }
        ActivePane::Blocks => {
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
    use crate::view_state::RightPane;
    match ctx.view.right_pane {
        p if p == wide => {
            ctx.view.right_pane = narrow;
            *last_pane(ctx.view) = narrow;
            ctx.view.active_pane = active_pane;
            sync_pane_cursor_to_disassembly(ctx);
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
                    let addr = line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);
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
                let target_addr = match ctx.view.active_pane {
                    ActivePane::Disassembly => {
                        if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                            if let Some(opcode) = &line.opcode {
                                match opcode.mode {
                                    AddressingMode::Immediate => {
                                        if let Some(fmt) = ctx
                                            .state
                                            .annotations
                                            .get(line.address)
                                            .and_then(|e| e.immediate_format)
                                        {
                                            match fmt {
                                                crate::state::ImmediateFormat::LowByte(target)
                                                | crate::state::ImmediateFormat::HighByte(target) => {
                                                    Some(target)
                                                }
                                                _ => None,
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                    AddressingMode::Absolute
                                    | AddressingMode::AbsoluteX
                                    | AddressingMode::AbsoluteY
                                    | AddressingMode::Indirect => {
                                        if line.bytes.len() >= 3 {
                                            Some(crate::state::Addr(
                                                u16::from(line.bytes[2]) << 8
                                                    | u16::from(line.bytes[1]),
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                    AddressingMode::Relative => {
                                        if line.bytes.len() >= 2 {
                                            let offset = line.bytes[1] as i8;
                                            Some(
                                                line.address
                                                    .wrapping_add(2)
                                                    .wrapping_add(offset as u16),
                                            )
                                        } else {
                                            None
                                        }
                                    }
                                    AddressingMode::ZeroPage
                                    | AddressingMode::ZeroPageX
                                    | AddressingMode::ZeroPageY
                                    | AddressingMode::IndirectX
                                    | AddressingMode::IndirectY => {
                                        if line.bytes.len() >= 2 {
                                            Some(crate::state::Addr(u16::from(line.bytes[1])))
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                }
                            } else {
                                line.external_label_address
                            }
                        } else {
                            None
                        }
                    }
                    ActivePane::HexDump => {
                        let origin = ctx.state.origin.0 as usize;
                        let bytes_per_row =
                            if ctx.view.right_pane == crate::view_state::RightPane::HexDump8 {
                                8
                            } else {
                                16
                            };
                        let alignment_padding = origin % bytes_per_row;
                        let aligned_origin = origin - alignment_padding;
                        let row_addr = (aligned_origin + ctx.view.hex_cursor_index * bytes_per_row)
                            .max(origin);
                        Some(crate::state::Addr(row_addr as u16))
                    }
                    ActivePane::Sprites => {
                        let origin = ctx.state.origin.0 as usize;
                        let padding = (64 - (origin % 64)) % 64;
                        Some(crate::state::Addr(
                            (origin + padding + ctx.view.sprites_cursor_index * 64) as u16,
                        ))
                    }
                    ActivePane::Charset => {
                        let origin = ctx.state.origin.0 as usize;
                        let base_alignment = 0x400;
                        let aligned_start_addr = (origin / base_alignment) * base_alignment;
                        Some(crate::state::Addr(
                            (aligned_start_addr + ctx.view.charset_cursor_index * 8) as u16,
                        ))
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
                        let origin = ctx.state.origin.0 as usize;
                        let first_aligned_addr = origin.div_ceil(8192) * 8192;
                        let bitmap_addr =
                            first_aligned_addr + (ctx.view.bitmap_cursor_index * 8192);
                        Some(crate::state::Addr(bitmap_addr as u16))
                    }
                    _ => None,
                };

                if let Some(addr) = target_addr {
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
}
