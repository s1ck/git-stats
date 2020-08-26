use crate::repo::{AuthorCounts, Repo, HAN_SOLO};
use itertools::Itertools;
use std::collections::BTreeMap;
use tui::widgets::ListState;
use unicode_width::UnicodeWidthStr;

pub(crate) struct App {
    should_quit: bool,
    current_author: Option<usize>,
    pub(crate) authors: StatefulList<usize>,
    co_author_counts: AuthorCounts,
    navigator_counts: AuthorCounts,
    pub(crate) repo: Repo,
    search_filter: String,
    author_widget_width: u16,
    range_filter_popup: Option<RangeFilter>,
}

#[derive(Debug, Default)]
pub(crate) struct RangeFilter {
    pub(crate) filter: String,
    pub(crate) error: String,
}

impl App {
    pub(crate) fn new(repo: Repo, range: Option<String>) -> App {
        let mut app = App {
            should_quit: false,
            current_author: None,
            authors: StatefulList::with_items(Default::default()),
            co_author_counts: Default::default(),
            navigator_counts: Default::default(),
            repo,
            search_filter: String::from(""),
            author_widget_width: Default::default(),
            range_filter_popup: Some(RangeFilter {
                filter: range.unwrap_or_default(),
                error: Default::default(),
            }),
        };
        app.on_enter();
        app
    }

    fn apply_authors(
        &mut self,
        mut navigator_counts: AuthorCounts,
        mut co_author_counts: AuthorCounts,
    ) {
        let all_authors = navigator_counts
            .keys()
            .chain(co_author_counts.keys())
            .copied()
            .unique()
            .collect_vec();

        for author in all_authors {
            let inner_navigators = navigator_counts.get_mut(&author);
            let inner_co_authors = co_author_counts.get_mut(&author);

            match (inner_navigators, inner_co_authors) {
                // key doesn't exist on either side (should never really happen)
                (None, None) => continue,
                // don't propagate navigators-only into the driver counts
                (None, Some(_)) => continue,
                // driver counts only, add zero value entries as navigators
                (Some(inner_navigators), None) => {
                    let inner_co_authors = co_author_counts.entry(author).or_default();

                    for key in inner_navigators.keys() {
                        inner_co_authors.insert(*key, 0);
                    }
                }
                // merge driver counts with navigator counts
                (Some(inner_navigators), Some(inner_co_authors)) => {
                    for key in inner_co_authors.keys() {
                        inner_navigators.entry(*key).or_default();
                    }
                    for key in inner_navigators.keys() {
                        inner_co_authors.entry(*key).or_default();
                    }
                }
            }
        }

        let mut authors = navigator_counts
            .iter()
            .filter(|(_, inner)| !inner.is_empty())
            .map(|(author, _)| *author)
            .collect_vec();

        authors.sort_by_key(|k| self.repo.string_cache().get(*k).unwrap_or_default());

        let author_widget_width = authors
            .iter()
            .flat_map(|author| self.repo.string_cache().get(*author))
            .map(|author| author.width())
            .max()
            .unwrap_or_default()
            + ">>".width();

        self.authors = StatefulList::with_items(authors);
        self.co_author_counts = co_author_counts;
        self.navigator_counts = navigator_counts;
        self.author_widget_width = author_widget_width as u16;
    }

    pub fn current_author(&self) -> Option<usize> {
        self.current_author
    }

    pub fn set_current_author(&mut self, author: &str) {
        let active = self.repo.string_cache().lookup(author);
        self.current_author = active;
    }

    pub fn co_author_tuples(&self, author: &usize) -> Vec<(&str, u64)> {
        self.value_tuples(self.co_author_counts.get(author))
    }

    pub fn navigator_tuples(&self, author: &usize) -> Vec<(&str, u64)> {
        self.value_tuples(self.navigator_counts.get(author))
    }

    fn value_tuples(&self, author_counts: Option<&BTreeMap<usize, u32>>) -> Vec<(&str, u64)> {
        match author_counts {
            Some(co_authors) => {
                let mut co_authors = co_authors
                    .iter()
                    .map(|(navigator, count)| {
                        (&self.repo.string_cache()[*navigator], (*count as u64))
                    })
                    .collect_vec();
                co_authors.sort_by_key(|(k, _)| if *k == HAN_SOLO { "~" } else { *k });
                co_authors
            }
            None => vec![],
        }
    }

    pub fn on_up(&mut self) {
        if let None = &self.range_filter_popup {
            self.authors.previous();
        }
    }

    pub fn on_down(&mut self) {
        if let None = &self.range_filter_popup {
            self.authors.next();
        }
    }

    pub fn on_key(&mut self, c: char) {
        if let Some(range_filter) = &mut self.range_filter_popup {
            range_filter.filter.push(c);
        } else {
            match c {
                'Q' => self.should_quit = true,
                'R' => self.range_filter_popup = Some(RangeFilter::default()),
                c if c.is_lowercase() || c.is_whitespace() => self.search_filter.push(c),
                _ => (),
            }
        }
    }

    pub fn on_enter(&mut self) {
        if let Some(RangeFilter { filter, error }) = self.range_filter_popup.take() {
            if filter.is_empty() && !error.is_empty() {
                self.range_filter_popup = Some(RangeFilter { filter, error });
                return;
            }
            let range_filter = Some(filter).filter(|r| !r.is_empty());
            match self.repo.extract_coauthors(range_filter) {
                Ok((navigator_counts, co_author_counts)) => {
                    self.apply_authors(navigator_counts, co_author_counts)
                }
                Err(e) => {
                    self.range_filter_popup = Some(RangeFilter {
                        filter: Default::default(),
                        error: e.to_string(),
                    })
                }
            }
        }
    }

    pub fn on_escape(&mut self) {
        if let None = self.range_filter_popup.take() {
            self.search_filter.truncate(0);
        }
    }

    pub fn on_backspace(&mut self) {
        if let Some(range_filter) = &mut self.range_filter_popup {
            let _ = range_filter.filter.pop();
        } else {
            let _ = self.search_filter.pop();
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn author_widget_width(&self) -> u16 {
        self.author_widget_width
    }

    pub fn authors(&mut self) -> &mut StatefulList<usize> {
        &mut self.authors
    }

    pub fn all_authors(&self) -> impl Iterator<Item = &str> {
        self.repo.string_cache().values()
    }

    pub fn search_filter(&self) -> &str {
        &self.search_filter
    }

    pub fn range_filter_popup(&self) -> Option<&RangeFilter> {
        self.range_filter_popup.as_ref()
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
