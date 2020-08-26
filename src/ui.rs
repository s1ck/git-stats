use itertools::Itertools;
use std::sync::mpsc;
use std::{io, thread};
use str_utils::StartsWithIgnoreAsciiCase;
use termion::input::TermRead;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::backend::{Backend, TermionBackend};
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, Paragraph, StackableBarChart,
    ValuePlacement, Wrap,
};
use tui::{Frame, Terminal};
use unicode_width::UnicodeWidthStr;

use crate::{app::App, repo::Repo};

pub fn render_coauthors(repo: Repo, range: Option<String>) -> eyre::Result<()> {
    let mut app = App::new(repo, range);

    let events = Events::with_config(Config::default());

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| draw(frame, &mut app))?;

        match events.next()? {
            Event::Input(key) => match key {
                Key::Char('\n') => app.on_enter(),
                Key::Char(c) => app.on_key(c),
                Key::Up => app.on_up(),
                Key::Down => app.on_down(),
                Key::Esc => app.on_escape(),
                Key::Backspace => app.on_backspace(),
                _ => (),
            },
        }

        if app.should_quit() {
            break;
        }
    }
    Ok(())
}

fn draw<B: Backend>(frame: &mut Frame<B>, app: &mut App) {
    let bar_gap = 3_u16;
    let string_cache = app.repo.string_cache();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(app.author_widget_width()),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(frame.size());

    let filtered_authors = app
        .authors
        .items
        .iter()
        .filter(|s| {
            string_cache
                .get(**s)
                .filter(|s| s.starts_with_ignore_ascii_case(&app.search_filter()))
                .is_some()
        })
        .copied()
        .collect_vec();

    app.authors.filter_down(filtered_authors);

    let authors = app
        .authors
        .current_items
        .iter()
        .flat_map(|author| string_cache.get(*author))
        .map(ListItem::new)
        .collect_vec();

    let list = List::new(authors)
        .block(Block::default().title("Authors").borders(Borders::ALL))
        .style(Style::default().fg(Color::Green))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    let list_area = if !app.search_filter().is_empty() {
        let list_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Max(3)].as_ref())
            .split(chunks[0]);

        let filter = Paragraph::new(app.search_filter())
            .block(Block::default().title("Filter").borders(Borders::ALL))
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        frame.render_widget(filter, list_chunks[1]);
        list_chunks[0]
    } else {
        chunks[0]
    };

    frame.render_stateful_widget(list, list_area, &mut app.authors.state);

    let author = app.authors.current();
    let author = match author {
        Some(author) => author,
        None => return,
    };

    let co_authors_area = chunks[1];
    let navigators_area = Layout::default()
        .margin(1)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(co_authors_area)[0];

    let co_author_tuples = app.co_author_tuples(author);
    let max_co_author_commits = co_author_tuples
        .iter()
        .map(|ca| ca.1)
        .max()
        .unwrap_or_default();

    // need to use the navigators_area as base for the bar width
    // as the co_authors_area also contains the block
    let width_per_co_author = usize::from(navigators_area.width) / co_author_tuples.len().max(1);
    let bar_width_co_author = width_per_co_author
        .saturating_sub(usize::from(bar_gap))
        .max(1);
    let co_authors_barchart = StackableBarChart::default()
        .block(Block::default().title("Co-Authors").borders(Borders::ALL))
        .data(&co_author_tuples[..])
        .bar_gap(bar_gap)
        .bar_width(bar_width_co_author as u16)
        .bar_style(Style::default().fg(Color::Red))
        .value_style(Style::default().fg(Color::Black).bg(Color::Red))
        .value_placement(ValuePlacement::Top);

    frame.render_widget(co_authors_barchart, co_authors_area);

    let navigator_tuples = app.navigator_tuples(author);
    let width_per_navigator = usize::from(navigators_area.width) / navigator_tuples.len().max(1);
    let bar_width_navigator = width_per_navigator
        .saturating_sub(usize::from(bar_gap))
        .max(1);
    let navigators_barchart = StackableBarChart::default()
        .data(&navigator_tuples[..])
        .max(max_co_author_commits)
        .bar_gap(bar_gap)
        .bar_width(bar_width_navigator as u16)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    frame.render_widget(navigators_barchart, navigators_area);

    if let Some(range_filter) = app.range_filter_popup() {
        let popup = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(50),
                    Constraint::Percentage(25),
                ]
                .as_ref(),
            )
            .split(frame.size())[1];

        let popup = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(45),
                    Constraint::Percentage(10),
                    Constraint::Percentage(45),
                ]
                .as_ref(),
            )
            .split(popup)[1];

        let filter = Block::default()
            .title("Filter for Commit range")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightGreen));

        frame.render_widget(Clear, popup);
        frame.render_widget(filter, popup);

        let filter = Paragraph::new(range_filter.filter.as_str())
            .style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        let filter_text = Layout::default()
            .direction(Direction::Vertical)
            .horizontal_margin(2)
            .constraints(
                [
                    Constraint::Max(popup.height.saturating_sub(1) / 2),
                    Constraint::Length(1),
                    Constraint::Max(popup.height.saturating_sub(1) / 2),
                ]
                .as_ref(),
            )
            .split(popup);
        let error_text = filter_text[2];
        let filter_text = filter_text[1];

        frame.render_widget(filter, filter_text);
        frame.set_cursor(
            // Put cursor past the end of the input text
            filter_text.x + range_filter.filter.width() as u16,
            // Move one line down, from the border to the input line
            filter_text.y,
        );

        if !range_filter.error.is_empty() {
            let error = Paragraph::new(range_filter.error.as_str())
                .block(
                    Block::default()
                        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                        .border_type(BorderType::Double),
                )
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });

            frame.render_widget(error, error_text);
        }
    }
}

pub enum Event<I> {
    Input(I),
}

pub struct Events {
    rx: mpsc::Receiver<Event<Key>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub exit_key: Key,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            exit_key: Key::Char('Q'),
        }
    }
}

impl Events {
    pub fn with_config(config: Config) -> Events {
        let (tx, rx) = mpsc::channel();
        let tx = tx.clone();

        thread::spawn(move || {
            let stdin = io::stdin();
            for evt in stdin.keys() {
                if let Ok(key) = evt {
                    if let Err(err) = tx.send(Event::Input(key)) {
                        eprintln!("{}", err);
                        return;
                    }
                    if key == config.exit_key {
                        return;
                    }
                }
            }
        });

        Events { rx }
    }

    pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
        self.rx.recv()
    }
}
