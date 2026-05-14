use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::types::{AppState, Direction as TradeDir};

pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(8),  // info panels
            Constraint::Min(8),     // signals
            Constraint::Length(1),  // footer
        ])
        .split(area);

    render_header(f, chunks[0], state);
    render_info(f, chunks[1], state);
    render_signals(f, chunks[2], state);
    render_footer(f, chunks[3]);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let cvd = state.live_cvd();
    let cvd_color = if cvd < 0.0 { Color::Red } else { Color::Green };
    let status_color = if state.connected { Color::Green } else { Color::Yellow };
    let price = state.current.as_ref().map(|c| c.close).unwrap_or(0.0);

    let alert_style = if state.is_alerting() {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let alert_span = if state.is_alerting() {
        Span::styled(" ⚡ SIGNAL! ⚡ ", alert_style)
    } else {
        Span::raw("             ")
    };

    let title = Line::from(vec![
        Span::styled(" BTC15 Engine ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        Span::styled(format!("${:.2}", price), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" │ CVD: "),
        Span::styled(format!("{:+.0}", cvd), Style::default().fg(cvd_color).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(&state.status, Style::default().fg(status_color)),
        Span::raw(" │"),
        alert_span,
    ]);

    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title(" BTCUSDT · 15m "));
    f.render_widget(header, area);
}

fn render_info(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: current candle
    let candle_lines = if let Some(c) = &state.current {
        let avg = state.avg_volume();
        let vol_ratio = if avg > 0.0 { c.volume / avg } else { 0.0 };
        let dir_color = if c.close >= c.open { Color::Green } else { Color::Red };
        let dir_sym = if c.close >= c.open { "▲" } else { "▼" };
        let vol_color = if vol_ratio >= 1.5 { Color::Yellow } else { Color::White };

        vec![
            Line::from(vec![
                Span::raw("  Open  "),
                Span::styled(format!("${:.2}", c.open), Style::default().fg(Color::White)),
                Span::raw("  High  "),
                Span::styled(format!("${:.2}", c.high), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("  Low   "),
                Span::styled(format!("${:.2}", c.low), Style::default().fg(Color::Red)),
                Span::raw("  Close "),
                Span::styled(format!("${:.2} {}", c.close, dir_sym), Style::default().fg(dir_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("  Volume: "),
                Span::styled(format!("{:.3} BTC  ({:.1}x avg)", c.volume, vol_ratio), Style::default().fg(vol_color)),
            ]),
            Line::from(vec![
                Span::raw("  ↓ Wick: "),
                Span::styled(format!("{:.1}%", c.lower_wick_ratio() * 100.0), wick_style(c.lower_wick_ratio())),
                Span::raw("   ↑ Wick: "),
                Span::styled(format!("{:.1}%", c.upper_wick_ratio() * 100.0), wick_style(c.upper_wick_ratio())),
            ]),
            Line::from(vec![
                Span::raw("  Range:  "),
                Span::styled(format!("${:.2}", c.range()), Style::default().fg(Color::Gray)),
            ]),
        ]
    } else {
        vec![Line::from("  Aguardando dados...")]
    };

    f.render_widget(
        Paragraph::new(candle_lines)
            .block(Block::default().borders(Borders::ALL).title(" Candle Atual ")),
        chunks[0],
    );

    // Right: CVD state
    let cvd = state.live_cvd();
    let cvd_color = if cvd < 0.0 { Color::Red } else { Color::Green };
    let trend_label = if cvd < 0.0 {
        "BEARISH ↓ → procurar LONG"
    } else {
        "BULLISH ↑ → procurar SHORT"
    };
    let current_delta = state.current.as_ref().map(|c| c.delta()).unwrap_or(0.0);
    let delta_color = if current_delta < 0.0 { Color::Red } else { Color::Green };

    let cvd_lines = vec![
        Line::from(vec![
            Span::raw("  CVD (20 candles): "),
            Span::styled(format!("{:+.0}", cvd), Style::default().fg(cvd_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("  Regime: "),
            Span::styled(trend_label, Style::default().fg(cvd_color)),
        ]),
        Line::from(vec![
            Span::raw("  Delta atual: "),
            Span::styled(format!("{:+.3}", current_delta), Style::default().fg(delta_color)),
        ]),
        Line::from(vec![
            Span::raw("  Vol médio: "),
            Span::styled(format!("{:.3} BTC", state.avg_volume()), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Histórico: "),
            Span::styled(format!("{} candles", state.candles.len()), Style::default().fg(Color::DarkGray)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(cvd_lines)
            .block(Block::default().borders(Borders::ALL).title(" CVD ")),
        chunks[1],
    );
}

fn render_signals(f: &mut Frame, area: Rect, state: &AppState) {
    let header = Row::new(vec![
        Cell::from("Hora").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Dir").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Preço").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Sombra%").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Vol/Avg").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("CVD").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = state.signals.iter().enumerate().map(|(i, sig)| {
        let is_new = i == 0 && state.is_alerting();
        let dir_color = match sig.direction {
            TradeDir::Long => Color::Green,
            TradeDir::Short => Color::Red,
        };
        let row_style = if is_new {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        Row::new(vec![
            Cell::from(sig.time.format("%H:%M:%S").to_string()),
            Cell::from(sig.direction.to_string()).style(Style::default().fg(dir_color)),
            Cell::from(format!("${:.2}", sig.price)),
            Cell::from(format!("{:.1}%", sig.wick_ratio * 100.0))
                .style(wick_style(sig.wick_ratio)),
            Cell::from(format!("{:.1}x", sig.volume_ratio))
                .style(Style::default().fg(if sig.volume_ratio >= 2.0 { Color::Yellow } else { Color::White })),
            Cell::from(format!("{:+.0}", sig.cvd))
                .style(Style::default().fg(if sig.cvd < 0.0 { Color::Red } else { Color::Green })),
        ]).style(row_style)
    }).collect();

    let title = if state.is_alerting() {
        " ⚡ Sinais - NOVO SINAL! ⚡ "
    } else if state.signals.is_empty() {
        " Sinais - Aguardando... "
    } else {
        " Sinais "
    };

    let title_style = if state.is_alerting() {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(10), // hora
            Constraint::Length(9),  // dir
            Constraint::Length(13), // preço
            Constraint::Length(9),  // sombra
            Constraint::Length(8),  // vol
            Constraint::Min(12),    // cvd
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, title_style)),
    );

    let mut ts = TableState::default();
    f.render_stateful_widget(table, area, &mut ts);
}

fn render_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw("/Esc: sair  "),
        Span::styled("c", Style::default().fg(Color::Yellow)),
        Span::raw(": limpar alerta"),
    ]))
    .alignment(Alignment::Left);
    f.render_widget(footer, area);
}

fn wick_style(ratio: f64) -> Style {
    if ratio >= 0.5 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if ratio >= 0.35 {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
