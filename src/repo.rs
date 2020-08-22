use std::collections::BTreeMap;
use std::collections::HashMap;
use std::{borrow::Cow, path::PathBuf};

use color_eyre::Section;
use eyre::Result;
use git2::{Repository, Revwalk};
use itertools::Itertools;
use once_cell::sync::Lazy;

use crate::stringcache::StringCache;

pub type AuthorCounts = BTreeMap<usize, BTreeMap<usize, u32>>;

const HAN_SOLO: &str = "Han Solo";

pub struct Repo {
    repository: Repository,
    replacements: Replacements,
    string_cache: StringCache,
}

impl Repo {
    pub fn open(path: Option<PathBuf>, replacements: Vec<(String, String)>) -> Result<Self> {
        let repository = path
            .map_or_else(Repository::open_from_env, Repository::discover)
            .map_err(|_| Error::NotInGitRepository)
            .suggestion(Suggestions::NotInGitRepository)?;

        let mut string_cache = StringCache::new();
        string_cache.intern("Han Solo");

        Ok(Repo {
            repository,
            replacements: Replacements(replacements),
            string_cache,
        })
    }

    pub fn into_string_cache(self) -> StringCache {
        self.string_cache
    }

    pub fn extract_coauthors(&mut self) -> Result<(AuthorCounts, AuthorCounts)> {
        let repository = &self.repository;
        let replacements = &self.replacements;
        let string_cache = &mut self.string_cache;

        let mut revwalk: Revwalk = repository.revwalk()?;

        revwalk.push_head()?;

        let mut driver_counts: AuthorCounts = BTreeMap::new();

        revwalk
            .filter_map(|oid| {
                let oid = oid.ok()?;
                repository.find_commit(oid).ok()
            })
            // TODO: should be an argument option
            // Filter merge commits
            .filter(|commit| commit.parent_count() == 1)
            // Extract co-author
            .for_each(|commit| {
                let author = commit.author();
                let author_name = author.name().unwrap_or_default();
                let author_name = replacements.normalize_author_name(author_name);
                let author_name = string_cache.intern(author_name);

                let commit_message = commit.message().unwrap_or_default();
                let navigators = Self::get_navigators(commit_message);

                let inner_map = driver_counts.entry(author_name).or_default();

                if navigators.is_empty() {
                    // TODO: move outside
                    let han_solo = string_cache.intern(HAN_SOLO);
                    let single_counts = inner_map.entry(han_solo).or_insert(0_u32);
                    *single_counts += 1;
                }

                for navigator in navigators {
                    let navigator = replacements.normalize_author_name(navigator);
                    let navigator = string_cache.intern(navigator);
                    let pair_counts = inner_map.entry(navigator).or_insert(0_u32);
                    *pair_counts += 1;
                }
            });

        let groups = driver_counts
            .iter()
            .flat_map(|(driver, navigators)| {
                navigators.iter().flat_map(move |(navigator, count)| {
                    vec![
                        (*driver, (*navigator, count)),
                        (*navigator, (*driver, count)),
                    ]
                })
            })
            .into_group_map();

        let pair_counts = groups
            .into_iter()
            .map(|(key, co_committers)| {
                let co_committers = co_committers
                    .into_iter()
                    .into_group_map()
                    .into_iter()
                    .map(|(co_committer, counts)| {
                        let sum = counts.into_iter().sum::<u32>();
                        (co_committer, sum)
                    })
                    .collect::<BTreeMap<_, _>>();
                (key, co_committers)
            })
            .collect::<BTreeMap<_, _>>();

        Ok((driver_counts, pair_counts))
    }

    fn get_navigators(commit_message: &str) -> Vec<&str> {
        commit_message
            .lines()
            .filter_map(|line| co_authors::get_co_author(line))
            .map(|coauthor| coauthor.name)
            .collect()
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

    fn replace_umlauts<'a>(input: &'a str) -> Cow<'a, str> {
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

#[derive(derive_more::Display, Debug)]
enum Suggestions {
    #[display(fmt = "Try running {} from within a Git repository.", APPLICATION)]
    NotInGitRepository,
}

static APPLICATION: &'static str = env!("CARGO_PKG_NAME");
