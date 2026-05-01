use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::app::{App, Focus, PendingAction, Screen, ACCENT, CURSOR_BLINK_MS, FIELDS};

const POPUP_HINT: &str = "←/→ or Tab to choose, Enter to confirm, Esc to cancel";

pub fn draw(f: &mut ratatui::Frame, app: &App) {
    draw_editor(f, app);

    let has_overlay = matches!(app.screen, Screen::Help) || app.pending_action.is_some();
    if has_overlay {
        dim_background(f);
    }

    if matches!(app.screen, Screen::Help) {
        draw_help(f);
    }

    if let Some(action) = &app.pending_action {
        draw_confirm_popup(f, app, action);
    }
}

fn dim_background(f: &mut ratatui::Frame) {
    let area = f.area();
    let buf = f.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(cell.style().add_modifier(Modifier::DIM));
            }
        }
    }
}

fn focused_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default()
    }
}

fn panel_title(label: &str, focused: bool) -> String {
    if focused {
        format!(" {} ● ", label)
    } else {
        format!(" {} ", label)
    }
}

fn selected_button_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::White).bg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn popup_buttons(selected: usize, label0: &str, label1: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {} ", label0), selected_button_style(selected == 0)),
        Span::raw("    "),
        Span::styled(format!(" {} ", label1), selected_button_style(selected == 1)),
    ])
}

fn render_popup(f: &mut ratatui::Frame, title: &str, lines: Vec<Line>, area: Rect) {
    f.render_widget(ratatui::widgets::Clear, area);
    let popup = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        );
    f.render_widget(popup, area);
}

fn render_scrollbar(f: &mut ratatui::Frame, area: Rect, total: usize, position: usize) {
    let mut state = ScrollbarState::new(total).position(position);
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        area,
        &mut state,
    );
}

fn draw_confirm_popup(f: &mut ratatui::Frame, app: &App, action: &PendingAction) {
    match action {
        PendingAction::ApplyFieldToAll { field, value, selected_choice } => {
            let area = centered_rect(50, 11, f.area());
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Apply field to all other fonts?",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("Field: {}", field.label())),
                Line::from(format!("Value: {}", value)),
                Line::from(""),
                popup_buttons(*selected_choice, "Cancel", "Apply"),
                Line::from(""),
                Line::from(POPUP_HINT),
            ];
            render_popup(f, "Confirm", lines, area);
        }
        PendingAction::ConfirmQuit { selected_choice } => {
            let area = centered_rect(40, 11, f.area());
            let unsaved = app.fonts.iter().filter(|f| f.has_changes()).count();

            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Quit FontMeta?",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            if unsaved > 0 {
                lines.push(Line::from(Span::styled(
                    format!("{} font(s) have unsaved changes", unsaved),
                    Style::default().fg(Color::Yellow),
                )));
                lines.push(Line::from(""));
            }

            lines.push(popup_buttons(*selected_choice, "Cancel", "Quit"));
            lines.push(Line::from(""));
            lines.push(Line::from(POPUP_HINT));

            render_popup(f, "Quit", lines, area);
        }
    }
}

fn draw_editor(f: &mut ratatui::Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(4)])
        .split(f.area());

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(outer[0]);

    draw_font_list(f, app, panes[0]);
    draw_info_panel(f, app, panes[1]);
    draw_footer(f, app, outer[1]);
}

fn draw_font_list(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let font_items: Vec<ListItem> = app
        .fonts
        .iter()
        .map(|font| {
            let name = font
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>");

            let mut spans = if font.has_changes() {
                vec![Span::styled("※ ", Style::default().fg(Color::Yellow))]
            } else {
                vec![Span::raw("  ")]
            };
            if font.variable {
                spans.push(Span::styled("V ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)));
            }
            spans.push(Span::raw(name));
            let line = Line::from(spans);
            ListItem::new(line)
        })
        .collect();

    let font_count = font_items.len();
    let mut font_state = ListState::default();
    if font_count > 0 {
        font_state.select(Some(app.selected_font));
    }

    let focused = matches!(app.focus, Focus::Fonts);

    let font_block = Block::default()
        .title(panel_title("Fonts", focused))
        .borders(Borders::ALL)
        .border_style(focused_border_style(focused));

    let inner_area = font_block.inner(area);

    let font_list = List::new(font_items)
        .block(font_block)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always)
        .scroll_padding(1);

    f.render_stateful_widget(font_list, area, &mut font_state);

    if font_count > inner_area.height as usize {
        render_scrollbar(f, inner_area, font_count, app.selected_font);
    }
}

fn draw_info_panel(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let focused = matches!(app.focus, Focus::Fields);

    let label = app.current_font()
        .and_then(|f| f.path.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("Info");
    let title = panel_title(label, focused);

    let info_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(focused_border_style(focused));

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(info_block.inner(area));

    let field_items = current_info_items(app);

    let mut field_state = ListState::default();
    if app.current_font().is_some() {
        field_state.select(Some(app.selected_field));
    }

    let fields_list = List::new(field_items)
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always)
        .scroll_padding(1);

    f.render_stateful_widget(fields_list, split[0], &mut field_state);

    if FIELDS.len() > split[0].height as usize {
        render_scrollbar(f, split[0], FIELDS.len(), app.selected_field);
    }

    if app.current_font().is_some() {
        let description = FIELDS[app.selected_field].description();
        let desc_para = Paragraph::new(description)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        f.render_widget(desc_para, split[1]);
    }

    f.render_widget(info_block, area);
}

fn draw_footer(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let key_style = Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD);

    let hint = if app.editing {
        Line::from(vec![
            Span::styled("ENTER", key_style),
            Span::raw(" Save  "),
            Span::styled("ESC", key_style),
            Span::raw(" Cancel"),
        ])
    } else {
        let show_revert = match app.focus {
            Focus::Fonts => app.current_font().map(|f| f.has_changes()).unwrap_or(false),
            Focus::Fields => app.current_font().map(|f| {
                let field = FIELDS[app.selected_field];
                f.edited.get(field) != f.original.get(field)
            }).unwrap_or(false),
        };

        let mut spans = vec![
            Span::styled("TAB", key_style),
            Span::raw(" Change Pane  "),
            Span::styled("↑↓", key_style),
            Span::raw(" Move  "),
            Span::styled("E", key_style),
            Span::raw("dit  "),
        ];

        if matches!(app.focus, Focus::Fields) {
            spans.push(Span::styled("A", key_style));
            spans.push(Span::raw("pply to All  "));
        }

        if show_revert {
            spans.push(Span::styled("R", key_style));
            spans.push(Span::raw(match app.focus {
                Focus::Fonts => "evert All  ",
                Focus::Fields => "evert  ",
            }));
        }

        spans.extend([
            Span::styled("S", key_style),
            Span::raw("ave  "),
            Span::styled("⬆ S", key_style),
            Span::raw("ave All  "),
            Span::styled("H", key_style),
            Span::raw("elp  "),
            Span::styled("Q", key_style),
            Span::raw("uit"),
        ]);

        Line::from(spans)
    };
    let footer = vec![hint, Line::from(app.status.as_str())];

    let help = Paragraph::new(footer)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Commands ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    f.render_widget(help, area);
}

fn current_info_items(app: &App) -> Vec<ListItem<'static>> {
    let Some(font) = app.current_font() else { return vec![]; };
    let info = &font.edited;
    let original = &font.original;
    let cursor_visible = app.cursor_started.elapsed().as_millis() / CURSOR_BLINK_MS % 2 == 0;

    FIELDS
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let selected = matches!(app.focus, Focus::Fields) && i == app.selected_field;

            let label_style = if selected {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let value = info.get(*field);
            let original_value = original.get(*field);
            let changed = value != original_value;

            let displayed_value = if app.editing && selected && field.is_editable() {
                if cursor_visible {
                    format!("{}█", app.input)
                } else {
                    format!("{} ", app.input)
                }
            } else {
                value
            };

            let line_style = if selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let value_style = if !field.is_editable() {
                line_style.fg(Color::Cyan)
            } else if changed {
                line_style.fg(Color::Yellow)
            } else {
                line_style
            };

            let line = Line::from(vec![
                Span::styled(format!("{:<23}", field.label()), label_style),
                Span::styled(" : ", Style::default().fg(Color::DarkGray)),
                Span::styled(displayed_value, value_style),
            ]);

            ListItem::new(line)
        })
        .collect()
}

fn draw_help(f: &mut ratatui::Frame) {
    let area = centered_rect(70, 21, f.area());

    f.render_widget(ratatui::widgets::Clear, area);

    let command_style = Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "FontMeta – Keyboard Shortcuts",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation", command_style),
            Span::raw(" – Use "),
            Span::styled("Tab", command_style),
            Span::raw(" to switch panes, "),
            Span::styled("↑↓", command_style),
            Span::raw(" to move"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Editing", command_style),
            Span::raw(" – Press "),
            Span::styled("Enter", command_style),
            Span::raw(" or "),
            Span::styled("E", command_style),
            Span::raw(" on a field to edit"),
        ]),
        Line::from(vec![
            Span::raw("           While editing: "),
            Span::styled("Enter", command_style),
            Span::raw(" to save, "),
            Span::styled("Esc", command_style),
            Span::raw(" to cancel"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Batch Operations", command_style),
            Span::raw(" – "),
            Span::styled("A", command_style),
            Span::raw(" applies current field value to all fonts"),
        ]),
        Line::from(vec![
            Span::raw("                     "),
            Span::styled("R", command_style),
            Span::raw(" reverts field to original value"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("File Operations", command_style),
            Span::raw(" – "),
            Span::styled("S", command_style),
            Span::raw(" saves current font, "),
            Span::styled("Shift+S", command_style),
            Span::raw(" saves all fonts"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Indicators", command_style),
            Span::raw(" – "),
            Span::styled("⁕", Style::default()),
            Span::raw(" in font list means unsaved changes"),
        ]),
        Line::from(vec![
            Span::raw("              "),
            Span::styled("Yellow", Style::default().fg(Color::Yellow)),
            Span::raw(" field label marks modified fields"),
        ]),
        Line::from(vec![
            Span::raw("              "),
            Span::styled("Cyan", Style::default().fg(Color::Cyan)),
            Span::raw(" text indicates non-editable fields"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Other", command_style),
            Span::raw(" – "),
            Span::styled("H", command_style),
            Span::raw(" shows this help, "),
            Span::styled("Q", command_style),
            Span::raw(" to quit"),
        ]),
        Line::from(""),
    ];

    let popup = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        );

    f.render_widget(popup, area);
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height),
            Constraint::Fill(1),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(width),
            Constraint::Fill(1),
        ])
        .split(vertical[1])[1]
}
