use std::collections::HashMap;
use std::ops::Index;

use fxhash::FxHashMap;

use crate::stringcache::StringCache;

#[derive(Debug, Default, Clone)]
pub struct AuthorPathCounts(FxHashMap<usize, Modifications>);

impl AuthorPathCounts {
    pub(crate) fn add_additions(&mut self, author: usize, additions: u32) {
        self.author(author).add_additions(additions);
    }

    pub(crate) fn add_deletions(&mut self, author: usize, deletions: u32) {
        self.author(author).add_deletions(deletions);
    }

    fn author(&mut self, author: usize) -> &mut Modifications {
        self.0.entry(author).or_default()
    }

    pub(crate) fn into_resolving_iter(
        self,
        string_cache: &StringCache,
    ) -> impl Iterator<Item = (&str, Modifications)> {
        ResolvingAuthorPathCountsIter {
            string_cache,
            inner: self.into_iter(),
        }
    }
}

impl Index<usize> for AuthorPathCounts {
    type Output = Modifications;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[&index]
    }
}

impl IntoIterator for AuthorPathCounts {
    type Item = (usize, Modifications);

    type IntoIter = <HashMap<usize, Modifications> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

struct ResolvingAuthorPathCountsIter<'a> {
    string_cache: &'a StringCache,
    inner: <HashMap<usize, Modifications> as IntoIterator>::IntoIter,
}

impl<'a> Iterator for ResolvingAuthorPathCountsIter<'a> {
    type Item = (&'a str, Modifications);

    fn next(&mut self) -> Option<Self::Item> {
        let (author, counts) = self.inner.next()?;
        let author = &self.string_cache[author];
        Some((author, counts))
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Modifications {
    additions: u32,
    deletions: u32,
}

impl Modifications {
    fn add_additions(&mut self, additions: u32) {
        self.additions += additions
    }

    fn add_deletions(&mut self, deletions: u32) {
        self.deletions += deletions
    }
}

#[cfg(test)]
mod tests {
    use cursive::With;

    use super::*;

    #[test]
    fn add_additions() {
        let mut author_path_counts: AuthorPathCounts = Default::default();
        author_path_counts.add_additions(0, 42);
        author_path_counts.add_additions(1, 84);
        assert_eq!(author_path_counts[0].additions, 42);
        assert_eq!(author_path_counts[1].additions, 84);
    }

    #[test]
    fn add_deletions() {
        let mut author_path_counts: AuthorPathCounts = Default::default();
        author_path_counts.add_deletions(0, 42);
        author_path_counts.add_deletions(1, 84);
        assert_eq!(author_path_counts[0].deletions, 42);
        assert_eq!(author_path_counts[1].deletions, 84);
    }

    #[test]
    fn resolving_iterator() {
        let mut string_cache = StringCache::new();
        let alice_idx = string_cache.intern("Alice");
        let bob_idx = string_cache.intern("Bob");

        let mut author_path_counts: AuthorPathCounts = Default::default();
        author_path_counts.add_additions(alice_idx, 42);
        author_path_counts.add_deletions(alice_idx, 23);
        author_path_counts.add_additions(bob_idx, 84);
        author_path_counts.add_additions(bob_idx, 32);

        for (author, modifications) in author_path_counts.into_resolving_iter(&string_cache) {
            if author == "Alice" {
                assert_eq!(modifications.additions, 42);
                assert_eq!(modifications.deletions, 23);
            }
            if author == "Bob" {
                assert_eq!(modifications.additions, 84);
                assert_eq!(modifications.deletions, 32);
            }
        }
    }
}