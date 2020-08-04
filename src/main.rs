use color_eyre::Section;
use eyre::Report;
use fehler::throws;
use git2::{Repository, Revwalk, Commit};

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

    let pairs = revwalk
        .filter_map(|oid| {
            let oid = oid.ok()?;
            repository.find_commit(oid).ok()
        })
        .flat_map(|commit| {
            let author = commit.author();
            let author_name = author.name().unwrap_or_default();
            let commit_message = commit.message().unwrap_or_default();

            let driver = String::from(author_name);
            let navigators = get_navigator(commit_message);

            navigators.into_iter().map(move |navigator| (driver.clone(), String::from(navigator))).collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    println!("{:#?}", pairs);
}

fn get_navigator(commit_message: &str) -> Vec<&str> {
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