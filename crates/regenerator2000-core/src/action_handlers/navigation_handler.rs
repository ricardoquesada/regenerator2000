//! Navigation domain action handler for cursor movement, view pane toggling, search, address jumps, and system settings.

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

fn sync_pane_cursor_to_disassembly(ctx: &mut ActionContext<'_>) {
    let cursor_line = ctx.state.disassembly.get(ctx.view.cursor_index);
    let addr = match cursor_line {
        Some(l) => l.address,
        None => return,
    };
    match ctx.view.active_pane {
        ActivePane::HexDump => {
            let origin = ctx.state.origin.0;
            if addr.0 >= origin {
                let offset = (addr.0 - origin) as usize;
                if offset < ctx.state.raw_data.len() {
                    ctx.view.hex_cursor_index = offset / 16;
                    ctx.view.sub_cursor_index = offset % 16;
                }
            }
        }
        ActivePane::Sprites => {
            let origin = ctx.state.origin.0;
            let padding = (64 - (origin % 64)) % 64;
            let first_aligned = origin + padding;
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
            let origin = ctx.state.origin.0;
            let first_aligned =
                ((origin / 8192) * 8192) + if origin.is_multiple_of(8192) { 0 } else { 8192 };
            if addr.0 >= first_aligned {
                let offset = (addr.0 - first_aligned) as usize;
                ctx.view.bitmap_cursor_index = offset / 8192;
            }
        }
        ActivePane::Blocks => {
            let items = ctx.state.get_blocks_view_items();
            if let Some(pos) = items.iter().position(|item| match item {
                crate::state::BlockItem::Block { start, end, .. } => addr >= *start && addr <= *end,
                crate::state::BlockItem::Splitter(s) => addr == *s,
                crate::state::BlockItem::Scope { start, end, .. } => addr >= *start && addr <= *end,
            }) {
                ctx.view.blocks_selected_index = Some(pos);
            }
        }
        _ => {}
    }
}

fn handle_navigate_to_address(ctx: &mut ActionContext<'_>, target_addr: Addr) {
    let old_pane = ctx.view.active_pane;
    let old_target = match old_pane {
        ActivePane::Disassembly => ctx
            .state
            .disassembly
            .get(ctx.view.cursor_index)
            .map(|l| crate::view_state::NavigationTarget::Address(l.address.0))
            .unwrap_or(crate::view_state::NavigationTarget::Index(
                ctx.view.cursor_index,
            )),
        _ => crate::view_state::NavigationTarget::Index(ctx.view.cursor_index),
    };

    crate::navigation::perform_jump_to_address(ctx.state, ctx.view, target_addr);
    ctx.view.navigation_history.push((old_pane, old_target));
    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Jumped to ${:04X}",
        target_addr.0
    )));
    ctx.events.push(CoreEvent::ViewChanged);
}

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
                        query: String::new(),
                        filters: crate::state::search::SearchFilters::default(),
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
                match ctx.view.right_pane {
                    RightPane::HexDump16 => {
                        ctx.view.right_pane = RightPane::HexDump8;
                        ctx.view.last_hexdump_pane = RightPane::HexDump8;
                        ctx.view.active_pane = ActivePane::HexDump;
                        sync_pane_cursor_to_disassembly(ctx);
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Hex Dump View (8 columns)".to_string(),
                        ));
                    }
                    RightPane::HexDump8 => {
                        ctx.view.right_pane = RightPane::None;
                        ctx.events
                            .push(CoreEvent::StatusMessage("Hex Dump View Hidden".to_string()));
                        if ctx.view.active_pane == ActivePane::HexDump {
                            ctx.view.active_pane = ActivePane::Disassembly;
                        }
                    }
                    RightPane::None => {
                        ctx.view.right_pane = RightPane::HexDump16;
                        ctx.view.last_hexdump_pane = RightPane::HexDump16;
                        ctx.view.active_pane = ActivePane::HexDump;
                        sync_pane_cursor_to_disassembly(ctx);
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Hex Dump View (16 columns)".to_string(),
                        ));
                    }
                    _ => {
                        let restored = ctx.view.last_hexdump_pane;
                        ctx.view.right_pane = restored;
                        ctx.view.active_pane = ActivePane::HexDump;
                        sync_pane_cursor_to_disassembly(ctx);
                        let cols = if restored == RightPane::HexDump8 {
                            "8"
                        } else {
                            "16"
                        };
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Hex Dump View ({cols} columns)"
                        )));
                    }
                }
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleSpritesView => {
                use crate::view_state::RightPane;
                match ctx.view.right_pane {
                    RightPane::Sprites2Col => {
                        ctx.view.right_pane = RightPane::Sprites1Col;
                        ctx.view.last_sprites_pane = RightPane::Sprites1Col;
                        ctx.view.active_pane = ActivePane::Sprites;
                        sync_pane_cursor_to_disassembly(ctx);
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Sprites View (1 column)".to_string(),
                        ));
                    }
                    RightPane::Sprites1Col => {
                        ctx.view.right_pane = RightPane::None;
                        ctx.events
                            .push(CoreEvent::StatusMessage("Sprites View Hidden".to_string()));
                        if ctx.view.active_pane == ActivePane::Sprites {
                            ctx.view.active_pane = ActivePane::Disassembly;
                        }
                    }
                    RightPane::None => {
                        ctx.view.right_pane = RightPane::Sprites2Col;
                        ctx.view.last_sprites_pane = RightPane::Sprites2Col;
                        ctx.view.active_pane = ActivePane::Sprites;
                        sync_pane_cursor_to_disassembly(ctx);
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Sprites View (2 columns)".to_string(),
                        ));
                    }
                    _ => {
                        let restored = ctx.view.last_sprites_pane;
                        ctx.view.right_pane = restored;
                        ctx.view.active_pane = ActivePane::Sprites;
                        sync_pane_cursor_to_disassembly(ctx);
                        let cols = if restored == RightPane::Sprites1Col {
                            "1"
                        } else {
                            "2"
                        };
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Sprites View ({cols} column{s})",
                            s = if cols == "1" { "" } else { "s" }
                        )));
                    }
                }
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleCharsetView => {
                use crate::view_state::RightPane;
                match ctx.view.right_pane {
                    RightPane::Charset8Col => {
                        ctx.view.right_pane = RightPane::Charset4Col;
                        ctx.view.last_charset_pane = RightPane::Charset4Col;
                        ctx.view.active_pane = ActivePane::Charset;
                        sync_pane_cursor_to_disassembly(ctx);
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Charset View (4 columns)".to_string(),
                        ));
                    }
                    RightPane::Charset4Col => {
                        ctx.view.right_pane = RightPane::None;
                        ctx.events
                            .push(CoreEvent::StatusMessage("Charset View Hidden".to_string()));
                        if ctx.view.active_pane == ActivePane::Charset {
                            ctx.view.active_pane = ActivePane::Disassembly;
                        }
                    }
                    RightPane::None => {
                        ctx.view.right_pane = RightPane::Charset8Col;
                        ctx.view.last_charset_pane = RightPane::Charset8Col;
                        ctx.view.active_pane = ActivePane::Charset;
                        sync_pane_cursor_to_disassembly(ctx);
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Charset View (8 columns)".to_string(),
                        ));
                    }
                    _ => {
                        let restored = ctx.view.last_charset_pane;
                        ctx.view.right_pane = restored;
                        ctx.view.active_pane = ActivePane::Charset;
                        sync_pane_cursor_to_disassembly(ctx);
                        let cols = if restored == RightPane::Charset4Col {
                            "4"
                        } else {
                            "8"
                        };
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Charset View ({cols} columns)"
                        )));
                    }
                }
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleBitmapView => {
                if ctx.view.right_pane == crate::view_state::RightPane::Bitmap {
                    ctx.view.right_pane = crate::view_state::RightPane::None;
                    ctx.events
                        .push(CoreEvent::StatusMessage("Bitmap View Hidden".to_string()));
                    if ctx.view.active_pane == ActivePane::Bitmap {
                        ctx.view.active_pane = ActivePane::Disassembly;
                    }
                } else {
                    ctx.view.right_pane = crate::view_state::RightPane::Bitmap;
                    ctx.view.active_pane = ActivePane::Bitmap;
                    sync_pane_cursor_to_disassembly(ctx);
                    ctx.events
                        .push(CoreEvent::StatusMessage("Bitmap View Shown".to_string()));
                }
                ctx.events.push(CoreEvent::ViewChanged);
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
                    let addr = if line.bytes.is_empty() {
                        line.external_label_address.unwrap_or(line.address)
                    } else if line.bytes.len() > 1 {
                        let mut resolved = line.address;
                        let mut current_sub_index = 0;
                        let mut found = false;
                        for offset in 1..line.bytes.len() {
                            let mid_addr = line.address.wrapping_add(offset as u16);
                            if let Some(labels) = ctx.state.labels.get(&mid_addr) {
                                for _ in labels {
                                    if current_sub_index == ctx.view.sub_cursor_index {
                                        resolved = mid_addr;
                                        found = true;
                                        break;
                                    }
                                    current_sub_index += 1;
                                }
                            }
                            if found {
                                break;
                            }
                        }
                        resolved
                    } else {
                        line.address
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
                if ctx.view.right_pane == crate::view_state::RightPane::Blocks {
                    ctx.view.right_pane = crate::view_state::RightPane::None;
                    ctx.events
                        .push(CoreEvent::StatusMessage("Blocks View Hidden".to_string()));
                    if ctx.view.active_pane == ActivePane::Blocks {
                        ctx.view.active_pane = ActivePane::Disassembly;
                    }
                } else {
                    ctx.view.right_pane = crate::view_state::RightPane::Blocks;
                    ctx.view.active_pane = ActivePane::Blocks;
                    sync_pane_cursor_to_disassembly(ctx);
                    ctx.events
                        .push(CoreEvent::StatusMessage("Blocks View Shown".to_string()));
                }
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleDebuggerView => {
                if ctx.view.right_pane == crate::view_state::RightPane::Debugger {
                    ctx.view.right_pane = crate::view_state::RightPane::None;
                    ctx.events
                        .push(CoreEvent::StatusMessage("Debugger View Hidden".to_string()));
                    if ctx.view.active_pane == ActivePane::Debugger {
                        ctx.view.active_pane = ActivePane::Disassembly;
                    }
                } else {
                    ctx.view.right_pane = crate::view_state::RightPane::Debugger;
                    ctx.view.active_pane = ActivePane::Debugger;
                    ctx.events
                        .push(CoreEvent::StatusMessage("Debugger View Shown".to_string()));
                }
                ctx.events.push(CoreEvent::ViewChanged);
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
                        crate::view_state::NavigationTarget::Index(idx) => {
                            ctx.view.cursor_index = idx;
                            ctx.view.scroll_index = idx;
                            ctx.view.scroll_sub_index = 0;
                            ctx.view.sub_cursor_index = 0;
                        }
                    }
                    ctx.view.status_message = Some("Navigated back".to_string());
                    ctx.events.push(CoreEvent::ViewChanged);
                } else {
                    ctx.view.status_message = Some("No history".to_string());
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
                                                crate::state::ImmediateFormat::LowByte(target) => {
                                                    Some(target)
                                                }
                                                crate::state::ImmediateFormat::HighByte(target) => {
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
                                    | AddressingMode::AbsoluteY => {
                                        if line.bytes.len() >= 3 {
                                            Some(crate::state::Addr(
                                                u16::from(line.bytes[2]) << 8
                                                    | u16::from(line.bytes[1]),
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                    AddressingMode::Indirect => {
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
                        let alignment_padding = origin % 16;
                        let aligned_origin = origin - alignment_padding;
                        Some(crate::state::Addr(
                            (aligned_origin + ctx.view.hex_cursor_index * 16) as u16,
                        ))
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
                        if idx < blocks.len() {
                            match blocks[idx] {
                                crate::state::BlockItem::Block { start, .. } => Some(start),
                                crate::state::BlockItem::Splitter(addr) => Some(addr),
                                crate::state::BlockItem::Scope { start, .. } => Some(start),
                            }
                        } else {
                            None
                        }
                    }
                    ActivePane::Bitmap => {
                        let origin = ctx.state.origin.0 as usize;
                        let first_aligned_addr = ((origin / 8192) * 8192)
                            + if origin.is_multiple_of(8192) { 0 } else { 8192 };
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
}
