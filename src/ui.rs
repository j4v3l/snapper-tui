use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, Clear, Table, Row, Cell, TableState, Scrollbar, ScrollbarState, ScrollbarOrientation, Tabs},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus, Mode, InputKind};
use crate::theme::THEME;

pub fn draw(frame: &mut Frame, app: &App) {
    // Layout: [tabs][main][status][userdata?]
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)];
    if app.show_userdata { constraints.push(Constraint::Length(3)); }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.size());

    // Top tabs for configs
    draw_config_tabs(frame, chunks[0], app);

    if app.snaps_fullscreen {
        draw_snapshots_fullscreen(frame, chunks[1], app);
    } else {
        // SnapperGUI-like: single snapshots table as main view
        draw_snapshots_only(frame, chunks[1], app);
    }

    // Status bar first
    let cfg = app.configs_state.selected.and_then(|i| app.configs.get(i)).map(|c| c.name.as_str()).unwrap_or("-");
    let snaps_total = app.snapshots.len();
    let snaps_filtered = app.filtered_snaps.len();
    let sudo = if app.use_sudo { "sudo:on" } else { "sudo:off" };
    let focus = match app.focus { Focus::Configs => "Configs", Focus::Snapshots => "Snapshots" };
    let filter_hint = if app.filter_text.trim().is_empty() { String::new() } else { format!(" · filter: {}", app.filter_text) };
    let snaps_label = if app.filter_text.trim().is_empty() { format!("snaps: {}", snaps_total) } else { format!("snaps: {}/{}", snaps_filtered, snaps_total) };
    let left = format!("cfg: {cfg}  {snaps_label}  {sudo}  focus: {focus}{filter_hint}");
    let right = "q quit · r refresh · c create · e edit · d delete · Enter details · x diff · m mount · U umount · R rollback · K cleanup · C view-config · g edit-config (form) · Q setup-quota · Y limine-sync · f fullscreen · F filter · Tab/Shift-Tab switch-config · [ ] switch-config · u userdata · S sudo · ? help";
    let status_line = Line::from(vec![
        Span::styled(left, Style::default()),
        Span::raw("  |  "),
        Span::styled(right, Style::default().add_modifier(Modifier::DIM)),
    ]);
    let status = Paragraph::new(status_line)
        .block(Block::default().borders(Borders::TOP).title(app.status.as_str()));
    frame.render_widget(status, chunks[2]);

    // Optional bottom userdata bar (like SnapperGUI)
    if app.show_userdata {
        let mut info_lines: Vec<Line> = Vec::new();
        if let Some(s_idx) = app.snaps_state.selected {
            if let Some(s) = app.filtered_snaps.get(s_idx) {
                info_lines.push(Line::from(vec![Span::raw("User: "), Span::styled(if s.user.is_empty() { "-" } else { s.user.as_str() }, THEME.highlight_style())]));
                info_lines.push(Line::from(vec![Span::raw("Type: "), Span::raw(if s.kind.is_empty() { "-" } else { s.kind.as_str() })]));
                info_lines.push(Line::from(vec![Span::raw("Cleanup: "), Span::raw(if s.cleanup.is_empty() { "-" } else { s.cleanup.as_str() })]));
            }
        } else {
            info_lines.push(Line::from(Span::styled("Userdata", THEME.muted_style())));
        }
        let bar = Paragraph::new(info_lines).block(Block::default().borders(Borders::TOP).title("Userdata"));
        let area = chunks.last().copied().unwrap_or_else(|| frame.size());
        frame.render_widget(bar, area);
    }

    // Then draw modals on top (no dim overlay)
    match &app.mode {
        Mode::Normal => {}
        Mode::Input(kind) => draw_input_modal(frame, app, kind),
        Mode::ConfirmDelete(id) => draw_confirm_modal(frame, *id),
        Mode::ConfirmRollback(id) => draw_confirm_rollback(frame, *id),
        Mode::ConfirmCleanup(alg) => draw_confirm_cleanup(frame, alg),
        Mode::Help => draw_help_modal(frame),
        Mode::Details => draw_details_modal(frame, app),
        Mode::Loading => draw_loading_modal(frame, app),
        Mode::ConfigForm => draw_config_form(frame, app),
    }
}

fn draw_snapshots_only(frame: &mut Frame, area: Rect, app: &App) {
    let mut block = THEME.block("Snapshots");
    if app.focus == Focus::Snapshots { block = THEME.block_focused("Snapshots"); }

    if app.filtered_snaps.is_empty() {
        let empty = Paragraph::new("No snapshots").style(THEME.muted_style()).block(block);
        frame.render_widget(empty, area);
        return;
    }

    let rows: Vec<Row> = app.filtered_snaps.iter().map(|s| {
        Row::new(vec![
            Cell::from(format!("{}", s.id)),
            Cell::from(s.date.clone()),
            Cell::from(if s.user.is_empty() { "-".to_string() } else { s.user.clone() }),
            Cell::from(s.kind.clone()),
            Cell::from(s.cleanup.clone()),
            Cell::from(s.description.clone()),
        ])
    }).collect();
    // Compute inner area to decide if scrollbar is needed
    let inner_area = block.inner(area);
    let table = Table::new(rows, [Constraint::Length(6), Constraint::Length(26), Constraint::Length(8), Constraint::Length(10), Constraint::Length(14), Constraint::Min(10)])
        .header(Row::new(vec![Cell::from("#"), Cell::from("Date"), Cell::from("User"), Cell::from("Type"), Cell::from("Cleanup"), Cell::from("Description")])
            .style(THEME.header_style().bg(THEME.header_bg)))
        .block(block)
        .highlight_style(THEME.highlight_style())
        .highlight_symbol("▶ ");
    let mut tstate = TableState::default();
    tstate.select(app.snaps_state.selected);
    frame.render_stateful_widget(table, area, &mut tstate);

    if let Some(sel) = app.snaps_state.selected {
        let total = app.filtered_snaps.len();
        // inner height available for rows = inner.height minus 1 for header
        let visible_rows: usize = inner_area.height.saturating_sub(1) as usize;
        if total > visible_rows {
            let mut sb = ScrollbarState::new(total).position(sel);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(scrollbar, area, &mut sb);
        }
    }
}

fn draw_config_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = if app.configs.is_empty() {
        vec![Line::from("(no configs)")]
    } else {
        app.configs.iter().map(|c| Line::from(c.name.clone())).collect()
    };
    let idx = app.configs_state.selected.unwrap_or(0).min(app.configs.len().saturating_sub(1));
    let tabs = Tabs::new(titles)
        .select(idx)
        .block(Block::default().borders(Borders::BOTTOM).title("Configs"))
        .style(Style::default().fg(THEME.fg))
        .highlight_style(THEME.highlight_style())
        .divider(Span::styled(" │ ", THEME.muted_style()));
    frame.render_widget(tabs, area);
}

fn draw_snapshots_fullscreen(frame: &mut Frame, area: Rect, app: &App) {
    let mut block = THEME.block("Snapshots (fullscreen)");
    if app.focus == Focus::Snapshots { block = THEME.block_focused("Snapshots (fullscreen)"); }

    if app.filtered_snaps.is_empty() {
        let empty = Paragraph::new("No snapshots").style(THEME.muted_style()).block(block);
        frame.render_widget(empty, area);
        return;
    }

    let rows: Vec<Row> = app.filtered_snaps.iter().map(|s| {
        Row::new(vec![
            Cell::from(format!("{}", s.id)),
            Cell::from(s.date.clone()),
            Cell::from(if s.user.is_empty() { "-".to_string() } else { s.user.clone() }),
            Cell::from(s.kind.clone()),
            Cell::from(s.cleanup.clone()),
            Cell::from(s.description.clone()),
        ])
    }).collect();
    // Compute inner area to decide if scrollbar is needed
    let inner_area = block.inner(area);
    let table = Table::new(rows, [Constraint::Length(6), Constraint::Length(26), Constraint::Length(8), Constraint::Length(10), Constraint::Length(14), Constraint::Min(10)])
        .header(Row::new(vec![Cell::from("#"), Cell::from("Date"), Cell::from("User"), Cell::from("Type"), Cell::from("Cleanup"), Cell::from("Description")])
            .style(THEME.header_style().bg(THEME.header_bg)))
        .block(block)
        .highlight_style(THEME.highlight_style())
        .highlight_symbol("▶ ");
    let mut tstate = TableState::default();
    tstate.select(app.snaps_state.selected);
    frame.render_stateful_widget(table, area, &mut tstate);

    if let Some(sel) = app.snaps_state.selected {
        let total = app.filtered_snaps.len();
        let visible_rows: usize = inner_area.height.saturating_sub(1) as usize;
        if total > visible_rows {
            let mut sb = ScrollbarState::new(total).position(sel);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(scrollbar, area, &mut sb);
        }
    }
}

fn draw_left(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(area);

    // Configs list
    let configs_items: Vec<ListItem> = if app.configs.is_empty() {
        vec![ListItem::new("No configs").style(THEME.muted_style())]
    } else {
        app.configs
            .iter()
            .map(|c| ListItem::new(c.name.clone()))
            .collect()
    };
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(app.configs_state.selected);
    let mut block = THEME.block("Configs");
    if app.focus == Focus::Configs { block = THEME.block_focused("Configs"); }
    let configs_list = List::new(configs_items)
        .block(block)
        .highlight_style(THEME.highlight_style())
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(configs_list, chunks[0], &mut list_state);

    // Snapshots table
    let mut block = THEME.block("Snapshots");
    if app.focus == Focus::Snapshots { block = THEME.block_focused("Snapshots"); }

    if app.filtered_snaps.is_empty() {
        let empty = Paragraph::new("No snapshots")
            .style(THEME.muted_style())
            .block(block.clone());
        frame.render_widget(empty, chunks[1]);
    } else {
        let rows: Vec<Row> = app.filtered_snaps.iter().map(|s| {
            Row::new(vec![
                Cell::from(format!("{}", s.id)),
                Cell::from(s.date.clone()),
                Cell::from(if s.user.is_empty() { "-".to_string() } else { s.user.clone() }),
                Cell::from(s.kind.clone()),
                Cell::from(s.cleanup.clone()),
                Cell::from(s.description.clone()),
            ])
        }).collect();
        let table = Table::new(rows, [Constraint::Length(6), Constraint::Length(26), Constraint::Length(8), Constraint::Length(10), Constraint::Length(14), Constraint::Min(10)])
            .header(Row::new(vec![Cell::from("#"), Cell::from("Date"), Cell::from("User"), Cell::from("Type"), Cell::from("Cleanup"), Cell::from("Description")])
                .style(THEME.header_style().bg(THEME.header_bg)))
            .block(block)
            .highlight_style(THEME.highlight_style())
            .highlight_symbol("▶ ");
        let mut tstate = TableState::default();
        tstate.select(app.snaps_state.selected);
        frame.render_stateful_widget(table, chunks[1], &mut tstate);

        // Optional: scrollbar reflecting selection position
        if let Some(sel) = app.snaps_state.selected {
            let total = app.filtered_snaps.len().max(1);
            let mut sb = ScrollbarState::new(total).position(sel);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(scrollbar, chunks[1], &mut sb);
        }
    }
}

fn draw_right(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(cfg_idx) = app.configs_state.selected {
        if let Some(cfg) = app.configs.get(cfg_idx) {
            lines.push(Line::from(vec![Span::styled("Config: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&cfg.name)]));
        }
    }
    if let Some(s_idx) = app.snaps_state.selected {
        if let Some(s) = app.filtered_snaps.get(s_idx) {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Snapshot ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("#{}", s.id)),
                Span::raw("  (config: "),
                Span::raw(&s.config),
                Span::raw(")"),
            ]));
            lines.push(Line::from(format!("User: {}", if s.user.is_empty() { "-" } else { s.user.as_str() })));
            lines.push(Line::from(format!("Type: {}", if s.kind.is_empty() { "-" } else { s.kind.as_str() })));
            lines.push(Line::from(format!("Cleanup: {}", if s.cleanup.is_empty() { "-" } else { s.cleanup.as_str() })));
            lines.push(Line::from(format!("Date: {}", s.date)));
            lines.push(Line::from(format!("Description: {}", s.description)));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("Select a config and snapshot", Style::default().fg(Color::DarkGray))));
    }
    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(THEME.block("Details").style(Style::default().bg(THEME.bg).fg(THEME.fg)));
    frame.render_widget(para, area);
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1]);
    horiz[1]
}

// Center a rectangle of fixed width/height (in cells) within r.
fn centered_rect_fixed(r: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}

// dim overlay removed per user preference

fn draw_input_modal(frame: &mut Frame, app: &App, kind: &InputKind) {
    // Use a very compact modal for Filter (similar to a password box)
    let area = match kind {
        // All input/edit boxes use the same compact fixed size now
        _ => centered_rect_fixed(frame.size(), 50, 5),
    };
    frame.render_widget(Clear, area); // clear background
    let title = match kind {
        InputKind::Create => "Create snapshot description",
        InputKind::Edit(id) => Box::leak(format!("Edit description for #{}", id).into_boxed_str()),
        InputKind::CleanupAlgorithm => "Cleanup algorithm (number/timeline/empty-pre-post)",
        InputKind::DetailsSearch => "Find in details (/)",
        InputKind::ConfigFieldEdit(idx) => Box::leak(format!("Edit value for field #{}", idx + 1).into_boxed_str()),
        InputKind::Filter => "Filter snapshots",
    };
    let bottom = "Enter · Esc";
    let block = THEME.modal_warn_block(title)
        .title_bottom(Line::from(bottom).centered());
    frame.render_widget(block.clone(), area);
    let content_area = block.inner(area);

    // We keep a single border for all inputs; labels are implied by the title now.
    // For Filter keep a single border (modal block) to reduce height; otherwise draw an inner input block
    // Keep a single border (modal block) for all inputs for a tighter layout
    let input_area = content_area;

    // Placeholder when empty
    let placeholder = match kind {
        InputKind::Create | InputKind::Edit(_) => "Type description…",
        InputKind::CleanupAlgorithm => "e.g., number, timeline, empty-pre-post",
        InputKind::DetailsSearch => "Type search text…",
        InputKind::ConfigFieldEdit(_) => "Type value…",
        InputKind::Filter => "Type filter…",
    };
    let mut paragraph = if app.input.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            placeholder,
            THEME.muted_style(),
        )))
    } else {
        Paragraph::new(app.input.as_str())
    };
    // Horizontal scroll to keep cursor visible
    let before = app.input.chars().take(app.input_cursor).collect::<String>();
    let w = UnicodeWidthStr::width(before.as_str()) as u16;
    let area_w = input_area.width.saturating_sub(1); // leave space for caret
    let hscroll: u16 = if w >= area_w { w - area_w + 1 } else { 0 };
    paragraph = paragraph.scroll((0, hscroll));
    frame.render_widget(paragraph, input_area);

    // Position cursor inside the input box using unicode width and hscroll
    if input_area.width > 0 {
        let right_edge = input_area.x + input_area.width - 1;
        let mut cursor_x = input_area.x.saturating_add(w.saturating_sub(hscroll));
        if cursor_x > right_edge { cursor_x = right_edge; }
        let cursor_y = input_area.y; // single-line input
        frame.set_cursor(cursor_x, cursor_y);
    }
    // Bottom hint is now part of the modal's bottom title
}

fn draw_confirm_modal(frame: &mut Frame, id: u64) {
    let area = centered_rect(frame.size(), 50, 25);
    frame.render_widget(Clear, area);
    let block = THEME
        .modal_error_block("Confirm delete")
        .title_bottom(Line::from("y to confirm  ·  n or Esc to cancel").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let text = Paragraph::new(Line::from(vec![
        Span::raw("Delete snapshot #"),
        Span::styled(id.to_string(), Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("?"),
    ])).style(THEME.error_style());
    frame.render_widget(text, inner);
}

fn draw_confirm_rollback(frame: &mut Frame, id: u64) {
    let area = centered_rect(frame.size(), 50, 25);
    frame.render_widget(Clear, area);
    let block = THEME
        .modal_error_block("Confirm rollback")
        .title_bottom(Line::from("y to confirm  ·  n or Esc to cancel").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let text = Paragraph::new(Line::from(vec![
        Span::raw("Rollback to snapshot #"),
        Span::styled(id.to_string(), Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("? This will revert the subvolume."),
    ])).style(THEME.error_style());
    frame.render_widget(text, inner);
}

fn draw_confirm_cleanup(frame: &mut Frame, alg: &str) {
    let area = centered_rect(frame.size(), 55, 28);
    frame.render_widget(Clear, area);
    let block = THEME
        .modal_warn_block("Confirm cleanup")
        .title_bottom(Line::from("y to confirm  ·  n or Esc to cancel").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let msg = format!("Run 'snapper cleanup {}' for current config?", alg);
    let text = Paragraph::new(msg).style(THEME.warn_style());
    frame.render_widget(text, inner);
}

fn draw_help_modal(frame: &mut Frame) {
    let area = centered_rect(frame.size(), 70, 65);
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from("Global:"),
        Line::from("  q  Quit  ·  r  Refresh  ·  ?  Help"),
        Line::from("  S  Toggle sudo (may be required for snapper)"),
        Line::from(""),
    Line::from("Mouse:"),
    Line::from("  Wheel Up/Down  Scroll list or details"),
    Line::from(""),
        Line::from("Navigation:"),
        Line::from("  Tab            Switch focus (Configs ⟷ Snapshots)"),
        Line::from("  Up/Down        Move selection"),
        Line::from("  PageUp/Down    Jump by 10 items"),
        Line::from("  Home/End       First/Last item"),
        Line::from(""),
        Line::from("Snapshots:"),
        Line::from("  f              Toggle fullscreen table (view more rows)"),
        Line::from("  F / Ctrl-F     Filter snapshots (by id/date/description)"),
    Line::from("  Enter          Open 'snapper status' overlay for selected (prev..current)"),
    Line::from("  x              'snapper diff' between previous and selected"),
    Line::from("  m              Mount selected snapshot"),
    Line::from("  U              Unmount selected snapshot"),
    Line::from("  R              Rollback to selected snapshot (confirm)"),
    Line::from("  Y              Sync selected snapshot to Limine (logs shown)"),
    Line::from("  K              Cleanup (enter algorithm: number|timeline|empty-pre-post; confirm)"),
        Line::from("  c              Create snapshot (enter description)"),
        Line::from("  e              Edit description of selected snapshot"),
        Line::from("  d              Delete selected snapshot (with confirmation)"),
    Line::from(""),
    Line::from("Config management:"),
    Line::from("  C              View config (get-config)"),
    Line::from("  g              Edit config (form): Up/Down, Enter/e edit field, s/y save, Esc cancel"),
    Line::from("  Q              Setup quota (requires sudo)"),
        Line::from(""),
    // removed config picker to repurpose 'g' for edit-config
        Line::from(""),
        Line::from("Modals & overlays:"),
    Line::from("  Esc            Close/cancel (Help, Input, Confirm, Details)"),
        Line::from("  Input modal    Enter to submit; Esc to cancel"),
        Line::from("  Confirm delete y to confirm; n or Esc to cancel"),
    Line::from("  Details overlay Up/Down/PageUp/PageDown/Home/End; '/' find; n/N next/prev; Esc"),
        Line::from(""),
        Line::from("Notes:"),
        Line::from("  - If snapper reports permission/DBus errors, toggle sudo (S) or run via 'make sudo-run'."),
    ];
    let block = THEME
        .modal_block("Help")
        .title_bottom(Line::from("Esc to close").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}

// ConfigPicker removed; 'g' opens form editor directly.

fn draw_config_form(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.size(), 80, 70);
    frame.render_widget(Clear, area);
    let block = THEME
        .modal_block("Edit Config (Form)")
        .title_bottom(Line::from("Up/Down select · Enter/e edit · s or y save · Esc cancel").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let rows: Vec<Row> = if app.cfg_fields.is_empty() {
        vec![Row::new(vec![Cell::from(Span::styled("No config fields", THEME.muted_style()))])]
    } else {
        app.cfg_fields.iter().map(|f| {
            let mut key = f.key.clone();
            if f.modified { key.push_str(" *"); }
            Row::new(vec![
                Cell::from(key),
                Cell::from(f.value.clone()),
            ])
        }).collect()
    };
    let widths = [Constraint::Percentage(35), Constraint::Percentage(65)];
    let table = Table::new(rows, widths)
    .header(Row::new(vec![Cell::from("Key"), Cell::from("Value")]).style(THEME.header_style().bg(THEME.header_bg)))
        .highlight_style(THEME.highlight_style())
        .highlight_symbol("▶ ");
    let mut state = TableState::default();
    state.select(app.cfg_field_idx);
    frame.render_stateful_widget(table.block(Block::default()), inner, &mut state);
}

fn draw_details_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.size(), 80, 70);
    frame.render_widget(Clear, area);
    let block = THEME
        .modal_block(&app.details_title)
        .title_bottom(Line::from("Up/Down/PageUp/PageDown · Home/End · / find · n/N next/prev · Esc to close").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    // Paragraph with vertical scroll
    let mut para = Paragraph::new(app.details_text.as_str())
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(THEME.bg).fg(THEME.fg));
    para = para.scroll((app.details_scroll, 0));
    frame.render_widget(para, inner);

    // Draw a thin scrollbar at right
    let lines = app.details_lines.max(1) as usize;
    let pos = app.details_scroll.min(app.details_lines.saturating_sub(1)) as usize;
    let mut sb = ScrollbarState::new(lines).position(pos);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight).track_style(Style::default().bg(THEME.header_bg)).thumb_style(Style::default().fg(THEME.accent));
    frame.render_stateful_widget(scrollbar, inner, &mut sb);
}

fn draw_loading_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.size(), 40, 20);
    frame.render_widget(Clear, area);
    let dots = match (app.tick / 6) % 4 { // animate roughly every ~600ms
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    };
    let title = format!("Working{dots}");
    let block = THEME
        .modal_block(title)
        .title_bottom(Line::from("Esc/q to cancel").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let msg_text = if app.loading_message.is_empty() { "Please wait…".to_string() } else { app.loading_message.clone() };
    let msg = Paragraph::new(msg_text);
    frame.render_widget(msg, inner);
}
