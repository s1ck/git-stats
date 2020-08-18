use std::collections::BTreeMap;
use std::io;

use termion::input::TermRead;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{BarChart, Block, Borders, List, ListItem};
use tui::Terminal;

pub fn render_coauthors(driver_count: BTreeMap<String, BTreeMap<String, u32>>, pair_count: BTreeMap<String, BTreeMap<String, u32>>) -> eyre::Result<()> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let author = "Paul Horn";
    let counts = &driver_count[author];
    let navigator_tuples = counts
        .iter()
        .map(|(navigator, count)| (navigator.as_str(), (*count as u64)))
        .collect::<Vec<_>>();

    let pairs = &pair_count[author];
    let co_author_tuples = pairs
        .iter()
        .map(|(co_author, count)| (co_author.as_str(), (*count as u64)))
        .collect::<Vec<_>>();

    let bar_gap = 5_u16;

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(15), Constraint::Min(0)].as_ref())
                .split(frame.size());

            let authors = [ListItem::new(author)];
            let list = List::new(authors)
                .block(Block::default().title("Authors").borders(Borders::ALL))
                .style(Style::default().fg(Color::Green))
                .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
                .highlight_symbol(">>");

            let inner_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[1]);

            let bar_width_co_author = usize::from(inner_chunks[0].width) / co_author_tuples.len() - bar_gap as usize;
            let bar_width_navigator = usize::from(inner_chunks[1].width) / navigator_tuples.len() - bar_gap as usize;

            let co_authors_barchart = BarChart::default()
                .block(Block::default().title("Co-Authors").borders(Borders::ALL))
                .data(&co_author_tuples[..])
                .bar_gap(bar_gap)
                .bar_width(bar_width_co_author as u16)
                .bar_style(Style::default().fg(Color::Red))
                .value_style(Style::default().fg(Color::Black).bg(Color::Red));

            let navigators_barchart = BarChart::default()
                .block(Block::default().title("Navigators").borders(Borders::ALL))
                .data(&navigator_tuples[..])
                .bar_gap(bar_gap)
                .bar_width(bar_width_navigator as u16)
                .bar_style(Style::default().fg(Color::Yellow))
                .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

            frame.render_widget(list, chunks[0]);
            frame.render_widget(co_authors_barchart, inner_chunks[0]);
            frame.render_widget(navigators_barchart, inner_chunks[1]);
        });

        let keys = io::stdin().keys().next();
        if let Some(Ok(Key::Char('q'))) = keys {
            break;
        }
    }
    Ok(())
}
