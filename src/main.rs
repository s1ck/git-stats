#[macro_use]
extern crate maplit;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{AppSettings, Clap};
use color_eyre::Section;
use eyre::Report;
use fehler::throws;
use git2::{Repository, Revwalk};
use itertools::Itertools;
use nom::lib::std::collections::HashMap;
use once_cell::sync::Lazy;

mod ui;

#[derive(Clap, Debug)]
#[clap(version, author, about, global_setting = AppSettings::ColoredHelp)]
struct Opts {
    /// Path to Git repository
    #[clap(short, long)]
    repository: Option<PathBuf>,
    /// Depth of hot path investigation
    #[clap(short, long, default_value = "4")]
    depth: usize,
    /// Replace authors based on this map. Can be specified multiple times, value are delimited by `=`
    #[clap(short = "R", long, parse(try_from_str = parse_key_val), number_of_values = 1)]
    replacement: Vec<(String, String)>,
}

/// Parse a replacement key-value pair
fn parse_key_val(s: &str) -> eyre::Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| eyre::eyre!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].into(), s[pos + 1..].into()))
}

impl Opts {
    fn normalize_author_name(&self, name: &str) -> String {
        let name = self
            .replacement
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
        replace_umlauts(name)
    }
}

// #[throws(Report)]
fn open_repo(path: Option<PathBuf>) -> eyre::Result<Repository> {
    path.map_or_else(Repository::open_from_env, Repository::discover)
        .map_err(|_| Error::NotInGitRepository)
        .suggestion(Suggestions::NotInGitRepository)
}

#[throws(Report)]
fn main() {
    color_eyre::install()?;
    let mut opts: Opts = Opts::parse();

    // Returns Owned repository and replaces the field in Opts with the Options' default (None)
    // Same as:
    // let repository = std::mem::take(&mut opts.repository);
    // Same as:
    // let repository = std::mem::replace(&mut opts.repository, None);
    let repository = opts.repository.take();
    // let hot_path_depth = opts.depth;

    let repository = open_repo(repository)?;

    let mut revwalk: Revwalk = repository.revwalk()?;
    revwalk.push_head()?;

    let mut driver_counts = BTreeMap::new();
    // let mut hot_paths = BTreeMap::new();

    revwalk
        .filter_map(|oid| {
            let oid = oid.ok()?;
            repository.find_commit(oid).ok()
        })
        // TODO: should be an argument option
        // Filter merge commits
        .filter(|commit| commit.parent_count() == 1)
        // Extract co-author
        .inspect(|commit| {
            let author = commit.author();
            let author_name = author.name().unwrap_or_default();
            let commit_message = commit.message().unwrap_or_default();
            let author_name = opts.normalize_author_name(author_name);
            let navigators = get_navigators(commit_message);

            let inner_map = driver_counts
                .entry(author_name)
                .or_insert_with(BTreeMap::new);

            if navigators.is_empty() {
                let single_counts = inner_map.entry(String::from("han_solo")).or_insert(0_u32);
                *single_counts += 1;
            }

            for navigator in navigators {
                let navigator = opts.normalize_author_name(navigator);
                let pair_counts = inner_map.entry(navigator).or_insert(0_u32);
                *pair_counts += 1;
            }
        })
        // .inspect(|commit| {
        //     let parent = commit.parent(0).unwrap();
        //     let parent_tree = parent.tree().unwrap();
        //     let current = commit.tree().unwrap();
        //     let diff = repository.diff_tree_to_tree(Some(&parent_tree), Some(&current), None).unwrap();
        //     // let diff = repository.diff_tree_to_workdir(Some(&current), None).unwrap();
        //     let author = commit.author();
        //     let author_name = replace_umlauts(author.name().unwrap_or_default());
        //
        //     let inner_map = hot_paths.entry(author_name).or_insert_with(BTreeMap::new);
        //
        //     for delta in diff.deltas() {
        //         let directory = delta.new_file().path().unwrap();
        //         for ancestor in directory.ancestors().skip(1) {
        //             let path_count = inner_map.entry(ancestor.to_path_buf()).or_insert(0_u32);
        //             *path_count += 1;
        //         }
        //     }
        // })
        .for_each(drop);

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

    ui::render_coauthors(driver_counts, pair_counts)?

    // for (author, hot_paths) in hot_paths {
    //     println!("{}", author);
    //     let mut hot_paths = hot_paths.into_iter()
    //         .filter(|(path, _)| path.iter().count() <= hot_path_depth)
    //         .collect::<Vec<_>>();
    //     hot_paths.sort_by_key(|(_, count)| u32::max_value() - count);
    //     hot_paths.into_iter().for_each(|(path, count)| println!("{} => {:?}", count, path))
    // }

    // println!("{:#?}", pair_counts);
    // println!("{:#?}", hot_paths);
    //
    // for (author, co_authors) in pair_counts {
    //     let total_commits = co_authors.values().sum::<u32>();
    //     println!("{}: {} commits", author, total_commits);
    // }
}

fn get_navigators(commit_message: &str) -> Vec<&str> {
    commit_message
        .lines()
        .filter_map(|line| coauthor::get_co_author(line))
        .map(|coauthor| coauthor.name)
        .collect()
}

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

fn replace_umlauts(input: &str) -> String {
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

static APPLICATION: &'static str = env!("CARGO_PKG_NAME");

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
