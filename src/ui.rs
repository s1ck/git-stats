use std::collections::BTreeMap;
use std::io;

use termion::input::TermRead;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{BarChart, Block, Borders, List, ListItem};
use tui::Terminal;

pub fn render_coauthors(pair_counts: BTreeMap<String, BTreeMap<String, u32>>) -> eyre::Result<()> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let author = "Paul Horn";
    let counts = &pair_counts[author];
    let co_author_tuples = counts
        .iter()
        .map(|(co_author, count)| (co_author.as_str(), *count as u64))
        .collect::<Vec<_>>();

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

            let bar_width = usize::from(chunks[1].width) / co_author_tuples.len();

            let barchart = BarChart::default()
                .block(Block::default().title("Co-authors").borders(Borders::ALL))
                .data(&co_author_tuples[..])
                .bar_width(bar_width as u16)
                .bar_style(Style::default().fg(Color::Yellow))
                .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

            frame.render_widget(list, chunks[0]);

            frame.render_widget(barchart, chunks[1]);
        });

        let keys = io::stdin().keys().next();
        if let Some(Ok(Key::Char('q'))) = keys {
            break;
        }
    }
    Ok(())
}
