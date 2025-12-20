use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::state::AppState;

pub fn ui(f: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Menu
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.size());

    render_menu(f, chunks[0]);
    render_main_view(f, chunks[1], state);
    render_status_bar(f, chunks[2], state);
}

fn render_menu(f: &mut Frame, area: Rect) {
    let menu_text = " File  Edit  Jump  View ";
    let menu = Paragraph::new(menu_text)
        .style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_widget(menu, area);
}

fn render_main_view(f: &mut Frame, area: Rect, state: &mut AppState) {
    let items: Vec<ListItem> = state.disassembly
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let style = if i == state.cursor_index {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };

            let content = Line::from(vec![
                Span::styled(format!("{:04X}  ", line.address), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{: <12}", hex_bytes(&line.bytes)), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{: <4} ", line.mnemonic), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{: <15}", line.operand), Style::default().fg(Color::White)),
                Span::styled(format!("; {}", line.comment), Style::default().fg(Color::Gray)),
            ]);
            
            ListItem::new(content).style(style)
        })
        .collect();

    // Calculate scroll based on cursor to keep it in view
    // A simple basic list widget:
    // Ideally we use a ListState, but here we just render items.
    // Ratatui's List widget handles scrolling if we pass the state, but we are managing state manually for now via `state.disassembly` slice maybe?
    // Or we just pass the full list and set the state.
    
    // For large lists, we should only render what's visible or use ListState.
    // Let's use ListState and passing the items.
    
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Disassembly "))
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black)); // This is if we use state select

    // We need to manage the ListState in AppState or here.
    // If we use `cursor_index` as the selected item.
    let mut list_state = ListState::default();
    list_state.select(Some(state.cursor_index));
    
    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let status = format!(" Cursor: {:04X} | Origin: {:04X} | File: {:?}", 
        state.disassembly.get(state.cursor_index).map(|l| l.address).unwrap_or(0),
        state.origin,
        state.file_path.as_ref().map(|p| p.file_name().unwrap_or_default()).unwrap_or_default()
    );
    let bar = Paragraph::new(status)
        .style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_widget(bar, area);
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
}
