use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use color_eyre::Section;
use eyre::Result;
use git2::{Repository, Revwalk};
use itertools::Itertools;
use once_cell::sync::Lazy;

pub type AuthorCounts = BTreeMap<String, BTreeMap<String, u32>>;

pub struct Repo {
    repository: Repository,
    replacements: Vec<(String, String)>,
}

impl Repo {
    pub fn open(path: Option<PathBuf>, replacements: Vec<(String, String)>) -> Result<Self> {
        let repository = path
            .map_or_else(Repository::open_from_env, Repository::discover)
            .map_err(|_| Error::NotInGitRepository)
            .suggestion(Suggestions::NotInGitRepository)?;

        Ok(Repo {
            repository,
            replacements,
        })
    }

    pub fn extract_coauthors(&self) -> Result<(AuthorCounts, AuthorCounts)> {
        let mut revwalk: Revwalk = self.repository.revwalk()?;

        revwalk.push_head()?;

        let mut driver_counts = BTreeMap::new();

        revwalk
            .filter_map(|oid| {
                let oid = oid.ok()?;
                self.repository.find_commit(oid).ok()
            })
            // TODO: should be an argument option
            // Filter merge commits
            .filter(|commit| commit.parent_count() == 1)
            // Extract co-author
            .for_each(|commit| {
                let author = commit.author();
                let author_name = author.name().unwrap_or_default();
                let commit_message = commit.message().unwrap_or_default();
                let author_name = self.normalize_author_name(author_name);
                let navigators = Self::get_navigators(commit_message);

                let inner_map = driver_counts
                    .entry(author_name)
                    .or_insert_with(BTreeMap::new);

                if navigators.is_empty() {
                    let single_counts = inner_map.entry(String::from("han_solo")).or_insert(0_u32);
                    *single_counts += 1;
                }

                for navigator in navigators {
                    let navigator = self.normalize_author_name(navigator);
                    let pair_counts = inner_map.entry(navigator).or_insert(0_u32);
                    *pair_counts += 1;
                }
            });

        let groups = driver_counts
            .iter()
            .flat_map(|(driver, navigators)| {
                navigators.iter().flat_map(move |(navigator, count)| {
                    vec![
                        (driver.clone(), (navigator.clone(), count)),
                        (navigator.clone(), (driver.clone(), count)),
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
                    .map(|(co_comitter, counts)| {
                        let sum = counts.into_iter().sum::<u32>();
                        (co_comitter, sum)
                    })
                    .collect::<BTreeMap<_, _>>();
                (key, co_committers)
            })
            .collect::<BTreeMap<_, _>>();

        Ok((driver_counts, pair_counts))
    }

    fn normalize_author_name(&self, name: &str) -> String {
        let name = self
            .replacements
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

    fn get_navigators(commit_message: &str) -> Vec<&str> {
        commit_message
            .lines()
            .filter_map(|line| co_authors::get_co_author(line))
            .map(|coauthor| coauthor.name)
            .collect()
    }

    fn replace_umlauts(input: &str) -> String {
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

        let mut new_string = String::new();

        for c in input.chars() {
            match replacements.get(&c) {
                Some(replacement) => new_string.push_str(replacement),
                None => new_string.push(c),
            }
        }
        new_string
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
