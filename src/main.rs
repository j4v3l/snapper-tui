mod app;
mod limine;
mod snapper;
mod state;
mod system;
mod theme; // Declare the theme module
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

fn main() -> Result<()> {
    // Ensure terminal is restored even on panic
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        prev_hook(info);
    }));

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = app::App::new();

    use std::fs::OpenOptions;
    use std::io::Write;
    let mut log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("frame_times.log")?;

    let mut needs_redraw = true;
    loop {
        // poll input with a short timeout
        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') => break,
                    _ => {
                        app.on_key(key);
                        needs_redraw = true;
                    }
                },
                Event::Mouse(me) => {
                    app.on_mouse(me);
                    needs_redraw = true;
                },
                Event::Resize(_, _) => {
                    needs_redraw = true;
                },
                _ => {}
            }
        } else {
            // no input -> tick
            app.on_tick();
            // Always redraw for animation
            needs_redraw = true;
        }

        if needs_redraw {
            let draw_start = std::time::Instant::now();
            terminal.draw(|f| ui::draw(f, &mut app))?;
            let frame_time = draw_start.elapsed();
            let log_line = format!("[snapper-tui] Frame time: {:?}\n", frame_time);
            let _ = log_file.write_all(log_line.as_bytes());
            needs_redraw = false;
        }
    }
    Ok(())
}
