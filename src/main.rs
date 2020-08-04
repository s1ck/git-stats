use std::collections::HashMap;

use color_eyre::Section;
use eyre::Report;
use fehler::throws;
use git2::{Repository, Revwalk};

#[throws(Report)]
fn open_repo() -> Repository {
    Repository::open_from_env()
        .map_err(|_| Error::NotInGitRepository)
        .suggestion(Suggestions::NotInGitRepository)?
}

#[throws(Report)]
fn main() {
    color_eyre::install()?;

    let repository: Repository = open_repo()?;

    let mut revwalk: Revwalk = repository.revwalk()?;
    revwalk.push_head()?;

    let mut pair_counts = HashMap::new();

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
                    pair_counts.insert(String::from(author_name), HashMap::new());
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
    vec![commit_message]
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