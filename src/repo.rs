use std::{borrow::Cow, collections::HashMap, path::PathBuf};
use std::iter::once;
use std::path::Path;

use color_eyre::Section;
use cursive::With;
use git2::{Commit, Delta, Diff, DiffDelta, DiffFormat, DiffOptions, Patch, Pathspec, Repository};
use itertools::Itertools;
use once_cell::sync::Lazy;

use crate::{AuthorCounts, Result, StringCache};
use crate::author_path_counts::AuthorPathCounts;

pub const HAN_SOLO: &str = "Han Solo";

pub struct Repo {
    repository: Repository,
    replacements: Replacements,
    string_cache: StringCache,
}

impl Repo {
    pub(crate) fn open(
        path: Option<&PathBuf>,
        replacements: Vec<(String, String)>,
    ) -> Result<Self> {
        let repository = path
            .map_or_else(Repository::open_from_env, Repository::discover)
            .map_err(|_| Error::NotInGitRepository)
            .suggestion(Suggestions::NotInGitRepository)?;

        let mut string_cache = StringCache::new();
        let _ = string_cache.intern(HAN_SOLO);

        Ok(Repo {
            repository,
            replacements: Replacements(replacements),
            string_cache,
        })
    }

    pub(crate) fn workdir(&self) -> Option<PathBuf> {
        self.repository.workdir().map(|p| p.to_path_buf())
    }

    pub(crate) fn string_cache(&self) -> &StringCache {
        &self.string_cache
    }

    pub(crate) fn extract_author_counts(&mut self, range: Option<String>) -> Result<AuthorCounts> {
        let repository = &self.repository;
        let replacements = &self.replacements;
        let string_cache = &mut self.string_cache;

        let mut revwalk = repository.revwalk()?;
        match range {
            Some(range) => revwalk
                .push_range(range.as_str())
                .map_err(|err| eyre!("Invalid range: `{}`. Git error: {}", range, err.message()))?,
            None => revwalk
                .push_head()
                .map_err(|err| eyre!("Git error: {}", err.message()))?,
        };

        let author_counts = revwalk
            .filter_map(|oid| repository.find_commit(oid.ok()?).ok())
            // Filter merge commits
            // TODO: This should be an argument option
            .filter(|commit| commit.parent_count() == 1)
            .fold(AuthorCounts::default(), |counts, commit| {
                Self::find_and_add_navigators(replacements, string_cache, counts, commit)
            });

        Ok(author_counts)
    }

    pub(crate) fn extract_author_path_counts(&mut self, path_spec: &Path, range: Option<&String>) -> Result<AuthorPathCounts> {
        let repository = &self.repository;
        let replacements = &self.replacements;
        let string_cache = &mut self.string_cache;

        let mut revwalk = repository.revwalk()?;

        let mut diff_options = DiffOptions::new();
        diff_options.pathspec(path_spec);

        match range {
            Some(range) => revwalk
                .push_range(range.as_str())
                .map_err(|err| eyre!("Invalid range: `{}`. Git error: {}", range, err.message()))?,
            None => revwalk
                .push_head()
                .map_err(|err| eyre!("Git error: {}", err.message()))?,
        };

        let author_path_counts = revwalk
            .filter_map(|oid| repository.find_commit(oid.ok()?).ok())
            // Filter merge commits
            // TODO: This should be an argument option
            .filter(|commit| commit.parent_count() == 1)
            .flat_map(|commit| commit.parents()
                .filter_map(|parent| {
                    let author_name = commit.author();
                    let author_name = author_name.name()?;
                    let author_name = Self::author_id(replacements, string_cache, author_name);
                    let diff = Self::diff(&repository, &commit, &parent, &mut diff_options).ok()?;

                    if diff.deltas().len() == 0 {
                        return None;
                    }

                    let stats = diff.stats().unwrap();
                    Some((author_name, stats.insertions(), stats.deletions()))
                })
                .collect::<Vec<_>>()
            )
            .fold(AuthorPathCounts::default(), |mut counts, (author, additions, deletions)| {
                counts.add_additions(author, additions as u32);
                counts.add_deletions(author, deletions as u32);
                counts
            });

        Ok(author_path_counts)
    }

    fn diff<'a>(
        repo: &'a Repository,
        commit: &Commit,
        parent: &Commit,
        opts: &mut DiffOptions,
    ) -> eyre::Result<Diff<'a>> {
        let a = parent.tree()?;
        let b = commit.tree()?;
        let diff = repo.diff_tree_to_tree(Some(&a), Some(&b), Some(opts))?;
        Ok(diff)
    }

    fn find_and_add_navigators(
        replacements: &Replacements,
        string_cache: &mut StringCache,
        mut author_counts: AuthorCounts,
        commit: Commit<'_>,
    ) -> AuthorCounts {
        Self::try_find_and_add_navigators(replacements, string_cache, &mut author_counts, commit)
            .unwrap_or_default();
        author_counts
    }

    fn try_find_and_add_navigators(
        replacements: &Replacements,
        string_cache: &mut StringCache,
        author_counts: &mut AuthorCounts,
        commit: Commit<'_>,
    ) -> Option<()> {
        let commit_message = commit.message()?;
        let author_name = commit.author();
        let author_name = author_name.name()?;
        let author_name = Self::author_id(replacements, string_cache, author_name);

        let navigators = Self::get_navigators(commit_message);
        for navigator in navigators {
            let navigator = Self::author_id(replacements, string_cache, navigator);
            author_counts.add_pair(author_name, navigator);
        }

        Some(())
    }

    fn get_navigators(commit_message: &str) -> impl Iterator<Item = &str> {
        commit_message
            .lines()
            .filter_map(|line| co_authors::get_co_author(line))
            .map(|coauthor| coauthor.name)
            .pad_using(1, |_| HAN_SOLO)
    }

    fn author_id(replacements: &Replacements, string_cache: &mut StringCache, name: &str) -> usize {
        let name = replacements.normalize_author_name(name);
        string_cache.intern(name)
    }
}

struct Replacements(Vec<(String, String)>);

impl Replacements {
    fn normalize_author_name<'a>(&'a self, name: &'a str) -> Cow<'a, str> {
        let name = self
            .0
            .iter()
            .filter_map(|(replacing, replacements)| {
                if name == replacing.as_str() {
                    Some(replacements.as_str())
                } else {
                    None
                }
            })
            .next()
            .unwrap_or(name);

        Self::replace_umlauts(name)
    }

    fn replace_umlauts(input: &str) -> Cow<'_, str> {
        static REPLACEMENTS: Lazy<HashMap<char, &str>> = Lazy::new(|| {
            hashmap! {
                'Ä' => "Ae",
                'ä' => "ae",
                'Ö' => "Oe",
                'ö' => "oe",
                'Ü' => "Ue",
                'ü' => "ue",
                'ß' => "ss",
            }
        });

        let replacements = &*REPLACEMENTS;
        for c in input.chars() {
            if replacements.contains_key(&c) {
                let mut new_string = String::new();
                for c in input.chars() {
                    match replacements.get(&c) {
                        Some(replacement) => new_string.push_str(replacement),
                        None => new_string.push(c),
                    }
                }

                return new_string.into();
            }
        }

        input.into()
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Not in a Git repository.")]
    NotInGitRepository,
}

#[derive(thiserror::Error, Debug)]
enum Suggestions {
    #[error("Try running {} from within a Git repository.", APPLICATION)]
    NotInGitRepository,
}

pub(crate) static APPLICATION: &str = env!("CARGO_PKG_NAME");
