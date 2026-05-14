use chrono::{DateTime, Local};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::types::{AppState, Direction as TradeDir, WICK_THRESHOLD, VOLUME_THRESHOLD};

pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(8),  // candle + cvd
            Constraint::Length(9),  // últimos candles fechados
            Constraint::Min(6),     // sinais
            Constraint::Length(1),  // footer
        ])
        .split(area);

    render_header(f, chunks[0], state);
    render_info(f, chunks[1], state);
    render_recent_candles(f, chunks[2], state);
    render_signals(f, chunks[3], state);
    render_footer(f, chunks[4]);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let cvd = state.live_cvd();
    let cvd_color = if cvd < 0.0 { Color::Red } else { Color::Green };
    let status_color = if state.connected { Color::Green } else { Color::Yellow };
    let price = state.current.as_ref().map(|c| c.close).unwrap_or(0.0);

    let alert_span = if state.is_alerting() {
        Span::styled(
            " ⚡ SIGNAL! ⚡ ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
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

    f.render_widget(
        Paragraph::new(title).block(Block::default().borders(Borders::ALL).title(" BTCUSDT · 15m ")),
        area,
    );
}

fn render_info(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let candle_lines = if let Some(c) = &state.current {
        let avg = state.avg_volume();
        let vol_ratio = if avg > 0.0 { c.volume / avg } else { 0.0 };
        let dir_color = if c.close >= c.open { Color::Green } else { Color::Red };
        let dir_sym = if c.close >= c.open { "▲" } else { "▼" };

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
                Span::styled(format!("{:.3} BTC  ({:.1}x avg)", c.volume, vol_ratio), vol_style(vol_ratio)),
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
        Paragraph::new(candle_lines).block(Block::default().borders(Borders::ALL).title(" Candle Atual ")),
        chunks[0],
    );

    let cvd = state.live_cvd();
    let cvd_color = if cvd < 0.0 { Color::Red } else { Color::Green };
    let trend_label = if cvd < 0.0 { "BEARISH ↓ → procurar LONG" } else { "BULLISH ↑ → procurar SHORT" };
    let current_delta = state.current.as_ref().map(|c| c.delta()).unwrap_or(0.0);
    let delta_color = if current_delta < 0.0 { Color::Red } else { Color::Green };

    let session_start = DateTime::from_timestamp_millis(state.session_day_start)
        .unwrap_or_default()
        .with_timezone(&Local)
        .format("%d/%m %H:%M")
        .to_string();

    let cvd_lines = vec![
        Line::from(vec![
            Span::raw("  CVD sessão: "),
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
            Span::raw("  Sessão desde: "),
            Span::styled(format!("{} (UTC meia-noite)", session_start), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("  Vol médio: "),
            Span::styled(format!("{:.3} BTC", state.avg_volume()), Style::default().fg(Color::White)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(cvd_lines).block(Block::default().borders(Borders::ALL).title(" CVD ")),
        chunks[1],
    );
}

fn render_recent_candles(f: &mut Frame, area: Rect, state: &AppState) {
    let avg_vol = state.avg_volume();

    let header = Row::new(vec![
        Cell::from("Hora").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Dir").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("↓Wck").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("↑Wck").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Vol/Avg").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Delta").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("CVD sess").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Motivo").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let n = state.candles.len();
    let rows: Vec<Row> = (0..5_usize.min(n)).map(|age| {
        let c = &state.candles[n - 1 - age];
        let vol_ratio = if avg_vol > 0.0 { c.volume / avg_vol } else { 0.0 };
        let cvd_sess = state.historical_session_cvd(age);
        let lower = c.lower_wick_ratio();
        let upper = c.upper_wick_ratio();
        let is_bear = c.close < c.open;
        let is_bull = c.close > c.open;

        let dir_color = if is_bull { Color::Green } else { Color::Red };
        let dir_sym = if is_bull { "▲" } else { "▼" };

        let time_str = DateTime::from_timestamp_millis(c.open_time)
            .unwrap_or_default()
            .with_timezone(&Local)
            .format("%H:%M")
            .to_string();

        let miss = signal_miss(cvd_sess, lower, upper, vol_ratio, is_bear, is_bull);

        Row::new(vec![
            Cell::from(time_str),
            Cell::from(dir_sym).style(Style::default().fg(dir_color)),
            Cell::from(format!("{:.0}%", lower * 100.0)).style(wick_style(lower)),
            Cell::from(format!("{:.0}%", upper * 100.0)).style(wick_style(upper)),
            Cell::from(format!("{:.1}x", vol_ratio)).style(vol_style(vol_ratio)),
            Cell::from(format!("{:+.1}", c.delta()))
                .style(Style::default().fg(if c.delta() >= 0.0 { Color::Green } else { Color::Red })),
            Cell::from(format!("{:+.0}", cvd_sess))
                .style(Style::default().fg(if cvd_sess >= 0.0 { Color::Green } else { Color::Red })),
            Cell::from(miss).style(Style::default().fg(Color::Yellow)),
        ])
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),  // hora
            Constraint::Length(4),  // dir
            Constraint::Length(5),  // lower wick
            Constraint::Length(5),  // upper wick
            Constraint::Length(7),  // vol
            Constraint::Length(8),  // delta
            Constraint::Length(9),  // cvd sess
            Constraint::Min(10),    // motivo
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Últimos 5 Candles  (CVD = sessão diária, reset 00:00 UTC) "));

    let mut ts = TableState::default();
    f.render_stateful_widget(table, area, &mut ts);
}

// Returns a short reason why the signal didn't fire for this candle, or "OK" if it would have.
fn signal_miss(cvd: f64, lower: f64, upper: f64, vol_ratio: f64, is_bear: bool, is_bull: bool) -> &'static str {
    let long_cvd = cvd < 0.0;
    let short_cvd = cvd > 0.0;

    // Would it qualify for LONG?
    if long_cvd && is_bear && lower >= WICK_THRESHOLD && lower > upper && vol_ratio >= VOLUME_THRESHOLD {
        return "LONG ✓";
    }
    // Would it qualify for SHORT?
    if short_cvd && is_bull && upper >= WICK_THRESHOLD && upper > lower && vol_ratio >= VOLUME_THRESHOLD {
        return "SHORT ✓";
    }

    // Find the first failing condition for whichever side is "closer"
    if long_cvd || is_bear {
        if !long_cvd          { return "CVD>0"; }
        if !is_bear           { return "fechou ▲"; }
        if lower < WICK_THRESHOLD { return "sombra↓<35%"; }
        if lower <= upper     { return "sombra↑>↓"; }
        if vol_ratio < VOLUME_THRESHOLD { return "vol<1.5x"; }
    }
    if short_cvd || is_bull {
        if !short_cvd         { return "CVD<0"; }
        if !is_bull           { return "fechou ▼"; }
        if upper < WICK_THRESHOLD { return "sombra↑<35%"; }
        if upper <= lower     { return "sombra↓>↑"; }
        if vol_ratio < VOLUME_THRESHOLD { return "vol<1.5x"; }
    }

    "—"
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
            Cell::from(sig.time.with_timezone(&Local).format("%H:%M:%S").to_string()),
            Cell::from(sig.direction.to_string()).style(Style::default().fg(dir_color)),
            Cell::from(format!("${:.2}", sig.price)),
            Cell::from(format!("{:.1}%", sig.wick_ratio * 100.0)).style(wick_style(sig.wick_ratio)),
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
            Constraint::Length(10),
            Constraint::Length(9),
            Constraint::Length(13),
            Constraint::Length(9),
            Constraint::Length(8),
            Constraint::Min(12),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(Span::styled(title, title_style)));

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
    } else if ratio >= WICK_THRESHOLD {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn vol_style(ratio: f64) -> Style {
    if ratio >= 2.0 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if ratio >= VOLUME_THRESHOLD {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    }
}
