use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Tabs, Wrap,
    },
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, InputKind, Mode};
use crate::theme::THEME;

pub fn draw(frame: &mut Frame, app: &mut App) {
    // Layout: [tabs][main][status][userdata?]
    let mut constraints: Vec<Constraint> = vec![
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ];
    if app.show_userdata {
        constraints.push(Constraint::Length(7));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.size());

    // Top tabs for configs
    draw_config_tabs(frame, chunks[0], app);

    // Always show the single snapshots table as main view
    draw_snapshots_only(frame, chunks[1], app);

    // Status bar first
    let cfg = app
        .configs_state
        .selected
        .and_then(|i| app.configs.get(i))
        .map(|c| c.name.as_str())
        .unwrap_or("-");
    let snaps_total = app.snapshots.len();
    let snaps_filtered = app.filtered_snaps.len();
    let sudo = if app.use_sudo { "sudo:on" } else { "sudo:off" };
    let filter_hint = if app.filter_text.trim().is_empty() {
        String::new()
    } else {
        format!(" · filter: {}", app.filter_text)
    };
    let snaps_label = if app.filter_text.trim().is_empty() {
        format!("snaps: {}", snaps_total)
    } else {
        format!("snaps: {}/{}", snaps_filtered, snaps_total)
    };
    let left = format!("cfg: {cfg}  {snaps_label}  {sudo}{filter_hint}");
    let right = "q quit · r refresh · c create · e edit · d delete · Enter details · x diff · m mount · U umount · R rollback · K cleanup · C view-config · g edit-config (form) · Q setup-quota · Y limine-sync · F filter · Tab/Shift-Tab switch-config · [ ] switch-config · u userdata · S sudo · ? help";
    let status_line = Line::from(vec![
        Span::styled(left, Style::default()),
        Span::raw("  |  "),
        Span::styled(right, Style::default().add_modifier(Modifier::DIM)),
    ]);
    let status = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::TOP)
            .title(app.status.as_str()),
    );
    frame.render_widget(status, chunks[2]);

    // Optional bottom userdata bar (like SnapperGUI)
    if app.show_userdata {
        let area = chunks.last().copied().unwrap_or_else(|| frame.size());
        let mut lines: Vec<Line> = Vec::new();
        if let Some(s_idx) = app.snaps_state.selected {
            if let Some(s) = app.filtered_snaps.get(s_idx) {
                let cfg_name = if s.config.is_empty() {
                    app.configs_state
                        .selected
                        .and_then(|i| app.configs.get(i))
                        .map(|c| c.name.as_str())
                        .unwrap_or("-")
                } else {
                    s.config.as_str()
                };
                lines.push(Line::from(vec![
                    Span::styled("ID: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{}", s.id)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Config: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(cfg_name),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Date: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(s.date.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("User: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(if s.user.is_empty() {
                        "-"
                    } else {
                        s.user.as_str()
                    }),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(if s.kind.is_empty() {
                        "-"
                    } else {
                        s.kind.as_str()
                    }),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Cleanup: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(if s.cleanup.is_empty() {
                        "-"
                    } else {
                        s.cleanup.as_str()
                    }),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        "Description: ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(s.description.clone()),
                ]));
                // Simple hints: mountpoint (best effort) and diff/status ranges
                if let Some(sel) = app.snaps_state.selected {
                    let from = if sel > 0 {
                        app.filtered_snaps.get(sel - 1).map(|p| p.id).unwrap_or(0)
                    } else {
                        0
                    };
                    let to = s.id;
                    lines.push(Line::from(vec![
                        Span::styled(
                            "Compare range: ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!(
                            "{}..{} (Enter to view status, x for diff)",
                            from, to
                        )),
                    ]));
                }
                // Mountpoint heuristic mirrors common snapper locations; UI-only best effort
                {
                    let cfg = app
                        .configs_state
                        .selected
                        .and_then(|i| app.configs.get(i))
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    let candidates = [
                        format!("/run/snapper/{}/{}/mount", cfg, s.id),
                        format!("/var/run/snapper/{}/{}/mount", cfg, s.id),
                        format!("/.snapshots/{}/snapshot", s.id),
                    ];
                    // Prefer detected mountpoint from computed metadata
                    if let Some(mp) = &app.selected_mount_point {
                        lines.push(Line::from(vec![
                            Span::styled(
                                "Mountpoint: ",
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(mp.clone()),
                        ]));
                    } else {
                        // We don't probe the filesystem here to avoid IO in the draw loop; just display candidates
                        lines.push(Line::from(vec![
                            Span::styled(
                                "Mountpoints: ",
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(candidates.join("  ")),
                        ]));
                    }
                }
                // Lightweight background summary (first lines of status) if available
                if let Some(summary) = &app.userdata_summary {
                    lines.push(Line::from(Span::styled(
                        "Summary:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )));
                    for l in summary.lines() {
                        lines.push(Line::from(l.to_string()));
                    }
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "No snapshot selected",
                    THEME.muted_style(),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "No snapshot selected",
                THEME.muted_style(),
            )));
        }
        let bar = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(THEME.block("Userdata"));
        frame.render_widget(bar, area);
    }

    // Then draw modals on top (no dim overlay)
    match &app.mode {
        Mode::Normal => {}
        Mode::Input(kind) => draw_input_modal(frame, app, kind),
        Mode::ConfirmDelete(id) => draw_confirm_modal(frame, *id),
        Mode::ConfirmRollback(id) => draw_confirm_rollback(frame, *id),
        Mode::ConfirmCleanup(alg) => draw_confirm_cleanup(frame, alg),
        Mode::Help => draw_help_modal(frame, app),
        Mode::Details => draw_details_modal(frame, app),
        Mode::Loading => draw_loading_modal(frame, app),
        Mode::ConfigForm => draw_config_form(frame, app),
    }
}

fn draw_snapshots_only(frame: &mut Frame, area: Rect, app: &App) {
    // Minimal size guard: if area too small, show a hint
    if area.width < 50 || area.height < 5 {
        let hint = Paragraph::new("Terminal too small. Recommended ≥ 80x24")
            .style(THEME.warn_style())
            .block(THEME.block("Resize terminal"));
        frame.render_widget(hint, area);
        return;
    }
    let block = THEME.block("Snapshots");

    if app.filtered_snaps.is_empty() {
        let empty = Paragraph::new("No snapshots")
            .style(THEME.muted_style())
            .block(block);
        frame.render_widget(empty, area);
        return;
    }

    let rows: Vec<Row> = app
        .filtered_snaps
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(format!("{}", s.id)),
                Cell::from(s.date.clone()),
                Cell::from(if s.user.is_empty() {
                    "-".to_string()
                } else {
                    s.user.clone()
                }),
                Cell::from(s.kind.clone()),
                Cell::from(s.cleanup.clone()),
                Cell::from(s.description.clone()),
            ])
        })
        .collect();
    // Compute inner area to decide if scrollbar is needed
    let inner_area = block.inner(area);
    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(26),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Min(10),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from("#"),
            Cell::from("Date"),
            Cell::from("User"),
            Cell::from("Type"),
            Cell::from("Cleanup"),
            Cell::from("Description"),
        ])
        .style(THEME.header_style().bg(THEME.header_bg)),
    )
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
        app.configs
            .iter()
            .map(|c| Line::from(c.name.clone()))
            .collect()
    };
    let idx = app
        .configs_state
        .selected
        .unwrap_or(0)
        .min(app.configs.len().saturating_sub(1));
    let tabs = Tabs::new(titles)
        .select(idx)
        .block(Block::default().borders(Borders::BOTTOM).title("Configs"))
        .style(Style::default().fg(THEME.fg))
        .highlight_style(THEME.highlight_style())
        .divider(Span::styled(" │ ", THEME.muted_style()));
    frame.render_widget(tabs, area);
}

// draw_snapshots_fullscreen removed

// draw_left and draw_right removed (legacy, unused)

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
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// dim overlay removed per user preference

fn draw_input_modal(frame: &mut Frame, app: &App, kind: &InputKind) {
    // Use a very compact modal for Filter (similar to a password box)
    let area = centered_rect_fixed(frame.size(), 50, 5);
    frame.render_widget(Clear, area); // clear background
    let title = match kind {
        InputKind::Create => "Create snapshot description",
        InputKind::Edit(id) => Box::leak(format!("Edit description for #{}", id).into_boxed_str()),
        InputKind::CleanupAlgorithm => "Cleanup algorithm (number/timeline/empty-pre-post)",
        InputKind::DetailsSearch => "Find in details (/)",
        InputKind::ConfigFieldEdit(idx) => {
            Box::leak(format!("Edit value for field #{}", idx + 1).into_boxed_str())
        }
        InputKind::Filter => "Filter snapshots",
    };
    let bottom = "Enter · Esc";
    let block = THEME
        .modal_warn_block(title)
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
        Paragraph::new(Line::from(Span::styled(placeholder, THEME.muted_style())))
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
        if cursor_x > right_edge {
            cursor_x = right_edge;
        }
        let cursor_y = input_area.y; // single-line input
        frame.set_cursor(cursor_x, cursor_y);
    }
    // Bottom hint is now part of the modal's bottom title
}

fn draw_details_modal(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(frame.size(), 80, 70);
    frame.render_widget(Clear, area);

    // Compute content area first to derive pagination metrics for footer
    let tmp_block = THEME.modal_block(&app.details_title);
    let inner = tmp_block.inner(area);

    // Fixed-width prewrap and pagination
    const FIXED_WRAP_COLS: u16 = 100;
    let mut content_area = inner;
    if content_area.width > 1 {
        content_area.width -= 1; // reserve rightmost col for scrollbar
    }
    let rendered_text = normalize_text_for_ui(app.details_text.as_str());
    let prewrapped = prewrap_text(rendered_text.as_str(), FIXED_WRAP_COLS);
    let lines: Vec<&str> = prewrapped.lines().collect();
    let total_lines = lines.len();
    let visible_h = content_area.height as usize;
    // Store actual visible page lines for paging keys
    app.details_page_lines = visible_h as u16;
    let max_scroll = total_lines.saturating_sub(visible_h);
    let clamped_scroll: usize = app.details_scroll.min(max_scroll as u16) as usize;
    // Write back clamped value to keep state in sync and avoid perceived freeze at bounds
    let clamped_u16 = if clamped_scroll > u16::MAX as usize {
        u16::MAX
    } else {
        clamped_scroll as u16
    };
    app.details_scroll = clamped_u16;
    // Write back the clamped value so input handling doesn't accumulate past the end
    // (prevents feeling of freeze when at the bottom)
    // SAFETY: casting down is fine since details_scroll is u16 and clamped within max_scroll.
    // Note: this mutates app during draw; acceptable for UI sync.
    // Ideally, clamp during input or state update, but this keeps a single source of truth.
    // If needed later, we can refactor to a setter.
    // WARNING: Requires &mut App, but draw receives &App. So we cannot write back here.
    // We'll emulate by bounding the value used below; to prevent perceived freeze, we show 'End' badge.
    let start = clamped_scroll;
    let end = (start + visible_h).min(total_lines);

    // Footer with pagination and boundary hints
    let page = if visible_h == 0 {
        1
    } else {
        start / visible_h + 1
    };
    let total_pages = if visible_h == 0 {
        1
    } else {
        total_lines.div_ceil(visible_h)
    };
    let at_top = start == 0;
    let at_end = start >= max_scroll;
    let mut footer = format!(
        "↑/↓ PageUp/PageDown · Home/End · / find · n/N next/prev · Esc · Page {}/{}",
        page, total_pages
    );
    if at_top {
        footer.push_str(" · Top");
    }
    if at_end {
        footer.push_str(" · End");
    }
    let block = THEME
        .modal_block(&app.details_title)
        .title_bottom(Line::from(footer).centered());
    frame.render_widget(block.clone(), area);

    // Render only visible slice without extra wrapping
    let content = lines[start..end].join("\n");
    let para = Paragraph::new(content).style(Style::default().bg(THEME.bg).fg(THEME.fg));
    frame.render_widget(para, content_area);

    // Scrollbar only when needed
    if total_lines > visible_h {
        // Use range (max_scroll + 1) so the thumb can reach the absolute bottom
        let mut sb = ScrollbarState::new(max_scroll + 1).position(clamped_scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_style(Style::default().bg(THEME.header_bg))
            .thumb_style(Style::default().fg(THEME.accent));
        frame.render_stateful_widget(scrollbar, inner, &mut sb);
    }
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
        Span::styled(
            id.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("?"),
    ]))
    .style(THEME.error_style());
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
        Span::styled(
            id.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("? This will revert the subvolume."),
    ]))
    .style(THEME.error_style());
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

fn draw_help_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.size(), 72, 72);
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled("[Help]", THEME.header_style())),
        Line::from(""),
        Line::from(Span::styled(
            "[Basics]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  q  Quit   ·   r  Refresh   ·   ?  Toggle this help"),
        Line::from("  S  Toggle sudo (use if snapper needs privileges)"),
        Line::from(""),
        Line::from(Span::styled(
            "[Navigation]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab / Shift-Tab / ← → / [ ]  Switch config tabs"),
        Line::from("  ↑/↓  Move selection   ·   PgUp/PgDn  Jump by 10   ·   Home/End  First/Last"),
        Line::from("  Mouse wheel  Scroll list or details"),
        Line::from(""),
        Line::from(Span::styled(
            "[Snapshots]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  Enter  Show status (prev..selected)"),
        Line::from("  x      Show diff (prev..selected)"),
        Line::from("  m/U    Mount / Unmount"),
        Line::from("  R      Rollback (confirm)"),
        Line::from("  Y      Sync to Limine"),
        Line::from("  K      Cleanup (enter algorithm: number | timeline | empty-pre-post)"),
        Line::from("  c/e/d  Create / Edit / Delete"),
        Line::from("  F / Ctrl-F  Filter"),
        Line::from("  u      Toggle bottom Userdata panel"),
        Line::from(""),
        Line::from(Span::styled(
            "[Config management]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  C      View config (get-config)"),
        Line::from(
            "  g      Edit config (form): ↑/↓ select · Enter/e edit · s/y save · Esc cancel",
        ),
        Line::from("  Q      Setup quota"),
        Line::from(""),
        Line::from(Span::styled(
            "[Modals & overlays]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  Esc    Close/cancel (Help, Input, Confirm, Details)"),
        Line::from("  Details overlay: ↑/↓/PgUp/PgDn/Home/End · '/' find · n/N next/prev · Esc"),
        Line::from(""),
        Line::from(Span::styled(
            "[Notes]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(
            "  If you see permission/DBus errors, toggle sudo (S) or run via 'make sudo-run'.",
        ),
    ];
    let block = THEME
        .modal_block("Help")
        .title_bottom(Line::from("↑/↓ PgUp/PgDn scroll · Esc to close").centered());
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    // Scrollable help content
    let total_lines = lines.len().max(1);
    let visible_h = inner.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_h);
    let clamped_scroll = app.help_scroll.min(max_scroll as u16);
    let mut para = Paragraph::new(lines).wrap(Wrap { trim: true });
    para = para.scroll((clamped_scroll, 0));
    frame.render_widget(para, inner);

    // Scrollbar at right (only when needed)
    if total_lines > visible_h {
        let mut sb = ScrollbarState::new(total_lines).position(clamped_scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .track_style(Style::default().bg(THEME.header_bg))
            .thumb_style(Style::default().fg(THEME.accent));
        frame.render_stateful_widget(scrollbar, inner, &mut sb);
    }
}

// ConfigPicker removed; 'g' opens form editor directly.

fn draw_config_form(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.size(), 80, 70);
    frame.render_widget(Clear, area);
    let block = THEME.modal_block("Edit Config (Form)").title_bottom(
        Line::from("Up/Down select · Enter/e edit · s or y save · Esc cancel").centered(),
    );
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let rows: Vec<Row> = if app.cfg_fields.is_empty() {
        vec![Row::new(vec![Cell::from(Span::styled(
            "No config fields",
            THEME.muted_style(),
        ))])]
    } else {
        app.cfg_fields
            .iter()
            .map(|f| {
                let mut key = f.key.clone();
                if f.modified {
                    key.push_str(" *");
                }
                Row::new(vec![Cell::from(key), Cell::from(f.value.clone())])
            })
            .collect()
    };
    let widths = [Constraint::Percentage(35), Constraint::Percentage(65)];
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![Cell::from("Key"), Cell::from("Value")])
                .style(THEME.header_style().bg(THEME.header_bg)),
        )
        .highlight_style(THEME.highlight_style())
        .highlight_symbol("▶ ");
    let mut state = TableState::default();
    state.select(app.cfg_field_idx);
    frame.render_stateful_widget(table.block(Block::default()), inner, &mut state);
}

// (Removed duplicate draw_details_modal; single pagination version defined earlier)

// Normalize diff/status text for stable display in a terminal:
// - Expand tabs to 4 spaces (diffs often contain tabs)
// - Strip carriage returns (CR) that can cause overwriting artifacts
// - Remove ANSI escape sequences if any slipped through
fn normalize_text_for_ui(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut esc = false;
    let mut ansi = false;
    for ch in text.chars() {
        if ansi {
            // End ANSI on letter (rough heuristic) or 'm'
            if ch.is_ascii_alphabetic() || ch == 'm' {
                ansi = false;
            }
            continue;
        }
        if esc {
            // Expect '[' then parameters...
            if ch == '[' {
                ansi = true;
                esc = false;
                continue;
            }
            esc = false;
            continue;
        }
        match ch {
            '\t' => out.push_str("    "),
            '\r' => {}
            '\u{1b}' => esc = true, // ESC
            _ => out.push(ch),
        }
    }
    out
}

// Insert soft line breaks so that each line is at most `width` cells wide (unicode-aware)
fn prewrap_text(text: &str, width: u16) -> String {
    let maxw = width.max(1) as usize;
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut current_width = 0usize;
        for ch in line.chars() {
            let cw = UnicodeWidthStr::width(ch.encode_utf8(&mut [0; 4]));
            if cw == 0 {
                out.push(ch);
                continue;
            }
            if current_width + cw > maxw {
                out.push('\n');
                current_width = 0;
            }
            out.push(ch);
            current_width += cw;
        }
        out.push('\n');
    }
    out
}

fn draw_loading_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.size(), 40, 20);
    frame.render_widget(Clear, area);
    let dots = match (app.tick / 6) % 4 {
        // animate roughly every ~600ms
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
    let msg_text = if app.loading_message.is_empty() {
        "Please wait…".to_string()
    } else {
        app.loading_message.clone()
    };
    let msg = Paragraph::new(msg_text);
    frame.render_widget(msg, inner);
}
