use std::collections::BTreeMap;
use std::sync::mpsc;
use std::{io, thread};

use itertools::Itertools;
use str_utils::StartsWithIgnoreAsciiCase;
use termion::input::TermRead;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::backend::{Backend, TermionBackend};
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::ListState;
use tui::widgets::{
    Block, Borders, List, ListItem, Paragraph, StackableBarChart, ValuePlacement, Wrap,
};
use tui::{Frame, Terminal};
use unicode_width::UnicodeWidthStr;

pub fn render_coauthors(
    navigator_counts: BTreeMap<String, BTreeMap<String, u32>>,
    co_author_counts: BTreeMap<String, BTreeMap<String, u32>>,
) -> eyre::Result<()> {
    let mut app = App::new("Git stats", navigator_counts, co_author_counts);

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
                Key::Char(c) => app.on_key(c),
                Key::Up => app.on_up(),
                Key::Down => app.on_down(),
                Key::Esc => app.on_escape(),
                Key::Backspace => app.on_backspace(),
                _ => (),
            },
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw<B: Backend>(frame: &mut Frame<B>, app: &mut App) {
    let bar_gap = 3_u16;

    let author_widget_width = app
        .authors
        .items
        .iter()
        .map(|author| author.width())
        .max()
        .unwrap_or_default()
        + ">>".width();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(author_widget_width as u16),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(frame.size());

    let filtered_authors = app
        .authors
        .items
        .iter()
        .filter(|s| s.starts_with_ignore_ascii_case(&app.search_filter))
        .cloned()
        .collect_vec();
    app.authors.filter_down(filtered_authors);

    let authors = app
        .authors
        .current_items
        .iter()
        .map(|author| ListItem::new(author.as_str()))
        .collect_vec();

    let list = List::new(authors)
        .block(Block::default().title("Authors").borders(Borders::ALL))
        .style(Style::default().fg(Color::Green))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    let list_area = if !app.search_filter.is_empty() {
        let list_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Max(3)].as_ref())
            .split(chunks[0]);

        let filter = Paragraph::new(app.search_filter.as_str())
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
}

pub struct App<'a> {
    pub title: &'a str,
    pub should_quit: bool,
    pub authors: StatefulList<String>,
    pub co_author_counts: BTreeMap<String, BTreeMap<String, u32>>,
    pub navigator_counts: BTreeMap<String, BTreeMap<String, u32>>,
    search_filter: String,
}

impl<'a> App<'a> {
    pub fn new(
        title: &'a str,
        mut navigator_counts: BTreeMap<String, BTreeMap<String, u32>>,
        mut co_author_counts: BTreeMap<String, BTreeMap<String, u32>>,
    ) -> App<'a> {
        let all_authors = navigator_counts
            .keys()
            .chain(co_author_counts.keys())
            .unique()
            .cloned()
            .collect_vec();

        for author in &all_authors {
            let inner_navigators = navigator_counts.get_mut(author);
            let inner_co_authors = co_author_counts.get_mut(author);

            match (inner_navigators, inner_co_authors) {
                // key doesn't exist on either side (should never really happen)
                (None, None) => continue,
                // don't propagate navigators-only into the driver counts
                (None, Some(_)) => continue,
                // driver counts only, add zero value entries as navigators
                (Some(inner_navigators), None) => {
                    let inner_co_authors = co_author_counts.entry(author.clone()).or_default();

                    for key in inner_navigators.keys() {
                        inner_co_authors.insert(key.clone(), 0);
                    }
                }
                // (None, Some(inner_co_authors)) => {
                //     let inner_navigators = navigator_counts.entry(author.clone()).or_default();

                //     for key in inner_co_authors.keys() {
                //         inner_navigators.insert(key.clone(), 0);
                //     }
                // }
                // merge driver counts with navigator counts
                (Some(inner_navigators), Some(inner_co_authors)) => {
                    for key in inner_co_authors.keys() {
                        inner_navigators.entry(key.clone()).or_default();
                    }
                    for key in inner_navigators.keys() {
                        inner_co_authors.entry(key.clone()).or_default();
                    }
                }
            }
        }

        let authors = navigator_counts
            .iter()
            .filter(|(_, inner)| !inner.is_empty())
            .map(|(author, _)| author.clone())
            .collect_vec();

        App {
            title,
            should_quit: false,
            authors: StatefulList::with_items(authors),
            co_author_counts,
            navigator_counts,
            search_filter: String::from(""),
        }
    }

    pub fn co_author_tuples(&self, author: &str) -> Vec<(&str, u64)> {
        match self.co_author_counts.get(author) {
            Some(co_authors) => co_authors
                .iter()
                .map(|(navigator, count)| (navigator.as_str(), (*count as u64)))
                .collect::<Vec<_>>(),
            None => vec![],
        }
    }

    pub fn navigator_tuples(&self, author: &str) -> Vec<(&str, u64)> {
        match self.navigator_counts.get(author) {
            Some(co_authors) => co_authors
                .iter()
                .map(|(navigator, count)| (navigator.as_str(), (*count as u64)))
                .collect::<Vec<_>>(),
            None => vec![],
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
            'Q' => self.should_quit = true,
            c if c.is_lowercase() || c.is_whitespace() => self.search_filter.push(c),
            _ => (),
        }
    }

    pub fn on_escape(&mut self) {
        self.search_filter.truncate(0);
    }

    pub fn on_backspace(&mut self) {
        let _ = self.search_filter.pop();
    }
}

pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
    pub current_items: Vec<T>,
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        let current_items = Vec::new();
        StatefulList {
            state,
            items,
            current_items,
        }
    }

    pub fn current(&self) -> Option<&T> {
        self.current_items.get(self.state.selected().unwrap_or(0))
    }

    pub fn filter_down(&mut self, current_items: Vec<T>) {
        if let Some(i) = &mut self.state.selected() {
            if *i >= current_items.len() {
                *i = current_items.len().saturating_sub(1);
            }
        }
        self.current_items = current_items;
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.current_items.len().saturating_sub(1) {
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
                    self.current_items.len().saturating_sub(1)
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
