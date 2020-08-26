use crate::repo::{AuthorCounts, Repo, HAN_SOLO};
use itertools::Itertools;
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

pub(crate) struct App {
    current_author: Option<usize>,
    co_author_counts: AuthorCounts,
    navigator_counts: AuthorCounts,
    pub(crate) repo: Repo,
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
            current_author: None,
            co_author_counts: Default::default(),
            navigator_counts: Default::default(),
            repo,
            author_widget_width: Default::default(),
            range_filter_popup: Some(RangeFilter {
                filter: range.unwrap_or_default(),
                error: Default::default(),
            }),
        };
        app.update_counts_from_repo();
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

        self.co_author_counts = co_author_counts;
        self.navigator_counts = navigator_counts;
        self.author_widget_width = author_widget_width as u16;
    }

    pub(crate) fn current_author(&self) -> Option<usize> {
        self.current_author
    }

    pub(crate) fn set_current_author(&mut self, author: &str) {
        let active = self.repo.string_cache().lookup(author);
        self.current_author = active;
    }

    pub(crate) fn set_range_filter(&mut self, range: &str) {
        self.range_filter_popup = Some(RangeFilter {
            filter: String::from(range),
            error: Default::default(),
        })
    }

    pub(crate) fn co_author_tuples(&self, author: &usize) -> Vec<(&str, u64)> {
        self.value_tuples(self.co_author_counts.get(author))
    }

    pub(crate) fn navigator_tuples(&self, author: &usize) -> Vec<(&str, u64)> {
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

    pub(crate) fn update_counts_from_repo(&mut self) {
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

    pub(crate) fn author_widget_width(&self) -> u16 {
        self.author_widget_width
    }

    pub(crate) fn all_authors(&self) -> impl Iterator<Item = &str> {
        self.repo.string_cache().values()
    }
}
