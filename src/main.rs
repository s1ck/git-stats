use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Clap, AppSettings};
use color_eyre::Section;
use eyre::Report;
use fehler::throws;
use git2::{Repository, Revwalk};

#[derive(Clap, Debug)]
#[clap(version, author, about, global_setting = AppSettings::ColoredHelp)]
struct Opts {
    /// Path to Git repository
    #[clap(short, long)]
    repository: Option<PathBuf>,
}

#[throws(Report)]
fn open_repo(path: Option<PathBuf>) -> Repository {
    path.map_or_else(Repository::open_from_env, Repository::discover)
        .map_err(|_| Error::NotInGitRepository)
        .suggestion(Suggestions::NotInGitRepository)?
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

    let repository = open_repo(repository)?;

    let mut revwalk: Revwalk = repository.revwalk()?;
    revwalk.push_head()?;

    let mut pair_counts = BTreeMap::new();

    revwalk
        .filter_map(|oid| {
            let oid = oid.ok()?;
            repository.find_commit(oid).ok()
        })
        .for_each(|commit| {
            let author = commit.author();
            let author_name = author.name().unwrap_or_default();
            let commit_message = commit.message().unwrap_or_default();

            let inner_map = match pair_counts.get_mut(author_name) {
                Some(inner_map) => inner_map,
                None => {
                    pair_counts.insert(String::from(author_name), BTreeMap::new());
                    pair_counts.get_mut(author_name).unwrap()
                }
            };

            for navigator in get_navigators(commit_message) {
               let pair_counts =  match inner_map.get_mut(navigator) {
                    Some(inner_map) => inner_map,
                    None => {
                        inner_map.insert(String::from(navigator), 0_u32);
                        inner_map.get_mut(navigator).unwrap()
                    }
                };
                *pair_counts += 1;
            }
        });

    println!("{:#?}", pair_counts);
}

fn get_navigators(commit_message: &str) -> Vec<&str> {
    commit_message.lines()
        .filter(|line| line.to_lowercase().starts_with("co-authored-by:"))
        .filter_map(|line| line.splitn(2, ":").nth(1))
        .filter_map(|line| line.split("<").next())
        .map(|split| split.trim())
        .collect()
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