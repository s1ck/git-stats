use crate::StringCache;
use fxhash::FxHashMap;
use std::{collections::HashMap, ops::Index};

#[derive(Debug, Default)]
pub struct AuthorCounts(FxHashMap<usize, PairingCounts>);

impl AuthorCounts {
    pub(crate) fn add_pair(&mut self, driver: usize, navigator: usize) {
        if driver != navigator {
            self.author(driver).paired_with(navigator).inc_driver();
            self.author(navigator).paired_with(driver).inc_navigator();
        }
    }

    fn author(&mut self, author: usize) -> &mut PairingCounts {
        self.0.entry(author).or_default()
    }

    pub(crate) fn into_resolving_iter<'a>(
        self,
        string_cache: &'a StringCache,
    ) -> impl Iterator<Item = (&'a str, PairingCounts)> {
        ResolvingAuthorCountsIter {
            string_cache,
            inner: self.into_iter(),
        }
    }
}

impl Index<usize> for AuthorCounts {
    type Output = PairingCounts;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[&index]
    }
}

impl IntoIterator for AuthorCounts {
    type Item = (usize, PairingCounts);

    type IntoIter = <HashMap<usize, PairingCounts> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

struct ResolvingAuthorCountsIter<'a> {
    string_cache: &'a StringCache,
    inner: <HashMap<usize, PairingCounts> as IntoIterator>::IntoIter,
}

impl<'a> Iterator for ResolvingAuthorCountsIter<'a> {
    type Item = (&'a str, PairingCounts);

    fn next(&mut self) -> Option<Self::Item> {
        let (author, counts) = self.inner.next()?;
        let author = &self.string_cache[author];
        Some((author, counts))
    }
}

#[derive(Debug, Default, Clone)]
pub struct PairingCounts(FxHashMap<usize, PairedWith>);

impl PairingCounts {
    pub(crate) fn paired_with(&mut self, author: usize) -> &mut PairedWith {
        self.0.entry(author).or_default()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn max_value(&self) -> u32 {
        self.0.values().map(|c| c.total).max().unwrap_or_default()
    }

    pub(crate) fn resolving_iter<'counts, 'name: 'counts>(
        &'counts self,
        string_cache: &'name StringCache,
    ) -> impl Iterator<Item = (&'name str, PairedWith)> + 'counts {
        ResolvingPairingCountsIter::new(string_cache, self.0.iter())
    }
}

impl Index<usize> for PairingCounts {
    type Output = PairedWith;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[&index]
    }
}

struct ResolvingPairingCountsIter<'name, T> {
    string_cache: &'name StringCache,
    inner: T,
}

impl<'name, T> ResolvingPairingCountsIter<'name, T> {
    fn new(string_cache: &'name StringCache, inner: T) -> Self {
        Self {
            string_cache,
            inner,
        }
    }
}

impl<'name, 'counts, T> Iterator for ResolvingPairingCountsIter<'name, T>
where
    T: Iterator<Item = (&'counts usize, &'counts PairedWith)>,
{
    type Item = (&'name str, PairedWith);

    fn next(&mut self) -> Option<Self::Item> {
        let (author, counts) = self.inner.next()?;
        let author = &self.string_cache[*author];
        Some((author, *counts))
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct PairedWith {
    pub as_driver: u32,
    pub total: u32,
}

impl PairedWith {
    fn inc_driver(&mut self) {
        self.as_driver += 1;
        self.total += 1;
    }

    fn inc_navigator(&mut self) {
        self.total += 1;
    }
}
