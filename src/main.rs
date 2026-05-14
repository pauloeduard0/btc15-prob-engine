mod binance;
mod signals;
mod types;
mod ui;

use std::{io, time::Duration};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use types::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    // Fetch historical candles before starting
    let client = reqwest::Client::new();
    let history = binance::fetch_history(&client).await;

    let mut state = AppState::new();
    state.load_history(history);

    // Start WebSocket task
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    tokio::spawn(binance::run_ws(event_tx));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut kb = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));

    loop {
        tokio::select! {
            Some(ev) = event_rx.recv() => {
                if let Some(signal) = state.process(ev) {
                    state.push_signal(signal);
                }
            }
            Some(Ok(Event::Key(key))) = kb.next() => {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') => state.clear_alert(),
                    _ => {}
                }
            }
            _ = ticker.tick() => {}
        }

        terminal.draw(|f| ui::render(f, &state))?;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
