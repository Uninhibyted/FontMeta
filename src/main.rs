mod app;
mod binary;
mod font;
mod ui;

use std::{env, io, path::PathBuf, time::Duration};

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle},
};
use ratatui::{backend::CrosstermBackend, Terminal};

const EVENT_POLL: Duration = Duration::from_millis(80);

fn main() -> Result<()> {
    let mut font_paths: Vec<PathBuf> = Vec::new();
    let mut output_dir = PathBuf::from("Export");
    let mut raw_args = env::args().skip(1);

    while let Some(arg) = raw_args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("FontMeta — terminal font metadata editor");
                println!();
                println!("Usage: fontmeta [-o <dir>] [font files...]");
                println!();
                println!("Options:");
                println!("  -o, --output <dir>   Output directory for saved fonts (default: Export)");
                println!();
                println!("Fonts can also be dragged into the terminal or pasted as paths after launch.");
                return Ok(());
            }
            "-o" | "--output" => {
                if let Some(dir) = raw_args.next() {
                    output_dir = PathBuf::from(dir);
                }
            }
            _ => font_paths.push(PathBuf::from(arg)),
        }
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original_hook(info);
    }));

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, SetTitle("FontMeta"))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, font_paths, output_dir);

    restore_terminal();

    result
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableBracketedPaste);
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, args: Vec<PathBuf>, output_dir: PathBuf) -> Result<()> {
    let mut app = App::from_args(args, output_dir);

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(EVENT_POLL)? {
            match event::read()? {
                Event::Paste(text) => app.handle_paste(text),

                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    if app.editing {
                        app.handle_edit_key(key.code);
                    } else {
                        app.handle_normal_key(key.code);

                        if app.should_quit {
                            break;
                        }
                    }
                }

                _ => {}
            }
        }
    }

    Ok(())
}
