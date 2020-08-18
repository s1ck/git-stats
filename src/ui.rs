use std::{io, thread};
use std::collections::BTreeMap;
use std::sync::mpsc;

use itertools::Itertools;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use termion::input::TermRead;
use tui::{Frame, Terminal};
use tui::backend::{Backend, TermionBackend};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{BarChart, Block, Borders, List, ListItem};
use tui::widgets::ListState;
use unicode_width::UnicodeWidthStr;

pub fn render_coauthors(navigator_counts: BTreeMap<String, BTreeMap<String, u32>>, co_author_counts: BTreeMap<String, BTreeMap<String, u32>>) -> eyre::Result<()> {
    let events = Events::with_config(Config::default());

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new("Git stats", navigator_counts, co_author_counts);

    loop {
        terminal.draw(|frame| { draw(frame, &mut app) })?;

        match events.next()? {
            Event::Input(key) => match key {
                Key::Char(c) => {
                    app.on_key(c);
                }
                Key::Up => {
                    app.on_up();
                }
                Key::Down => {
                    app.on_down();
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw<B: Backend>(frame: &mut Frame<B>, app: &mut App) {
    let bar_gap = 5_u16;

    let author_widget_width = app.authors.items.iter()
        .map(|author| author.width())
        .max()
        .unwrap_or_default() + ">>".width();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(author_widget_width as u16), Constraint::Min(0)].as_ref())
        .split(frame.size());

    let authors = app
        .authors
        .items
        .iter()
        .map(|author| ListItem::new(author.as_str()))
        .collect::<Vec<_>>();

    let list = List::new(authors)
        .block(Block::default().title("Authors").borders(Borders::ALL))
        .style(Style::default().fg(Color::Green))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    frame.render_stateful_widget(list, chunks[0], &mut app.authors.state);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    let author = app.authors.current().unwrap();

    let co_author_tuples = app.co_author_tuples(author);
    let bar_width_co_author = usize::from(inner_chunks[0].width) / co_author_tuples.len() - bar_gap as usize;
    let co_authors_barchart = BarChart::default()
        .block(Block::default().title("Co-Authors").borders(Borders::ALL))
        .data(&co_author_tuples[..])
        .bar_gap(bar_gap)
        .bar_width(bar_width_co_author as u16)
        .bar_style(Style::default().fg(Color::Red))
        .value_style(Style::default().fg(Color::Black).bg(Color::Red));

    frame.render_widget(co_authors_barchart, inner_chunks[0]);

    let navigator_tuples = app.navigator_tuples(author);
    let bar_width_navigator = usize::from(inner_chunks[1].width) / navigator_tuples.len() - bar_gap as usize;
    let navigators_barchart = BarChart::default()
        .block(Block::default().title("Navigators").borders(Borders::ALL))
        .data(&navigator_tuples[..])
        .bar_gap(bar_gap)
        .bar_width(bar_width_navigator as u16)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    frame.render_widget(navigators_barchart, inner_chunks[1]);
}

pub struct App<'a> {
    pub title: &'a str,
    pub should_quit: bool,
    pub authors: StatefulList<String>,
    pub co_author_counts: BTreeMap<String, BTreeMap<String, u32>>,
    pub navigator_counts: BTreeMap<String, BTreeMap<String, u32>>,
}

impl<'a> App<'a> {
    pub fn new(
        title: &'a str,
        navigator_counts: BTreeMap<String, BTreeMap<String, u32>>,
        co_author_counts: BTreeMap<String, BTreeMap<String, u32>>) -> App<'a>
    {
        let authors = navigator_counts.keys().map(|s| {s.clone()}).collect_vec();

        App {
            title,
            should_quit: false,
            authors: StatefulList::with_items(authors),
            co_author_counts,
            navigator_counts,
        }
    }

    pub fn co_author_tuples(&self, author: &str) -> Vec<(&str, u64)> {
        match self.co_author_counts.get(author) {
            Some(co_authors) => {
                co_authors
                    .iter()
                    .map(|(navigator, count)| (navigator.as_str(), (*count as u64)))
                    .collect::<Vec<_>>()
            },
            None => vec![]
        }
    }

    pub fn navigator_tuples(&self, author: &str) -> Vec<(&str, u64)> {
        match self.navigator_counts.get(author) {
            Some(co_authors) => {
                co_authors
                    .iter()
                    .map(|(navigator, count)| (navigator.as_str(), (*count as u64)))
                    .collect::<Vec<_>>()
            },
            None => vec![]
        }
    }

    pub fn on_up(&mut self) {
        self.authors.previous();
    }

    pub fn on_down(&mut self) {
        self.authors.next();
    }

    pub fn on_key(&mut self, c: char) {
        match c {
            'q' => {
                self.should_quit = true;
            }
            _ => {}
        }
    }
}

pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> StatefulList<T> {

    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        StatefulList {
            state,
            items,
        }
    }

    pub fn current(&self) -> Option<&T> {
        self.items.get(self.state.selected().unwrap_or(0))
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
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
            exit_key: Key::Char('q'),
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
