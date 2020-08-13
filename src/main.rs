#![allow(deprecated)]

#[macro_use]
extern crate maplit;
#[macro_use]
extern crate nom;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{AppSettings, Clap};
use color_eyre::Section;
use eyre::Report;
use fehler::throws;
use git2::{Repository, Revwalk};
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

    let mut pair_counts = BTreeMap::new();
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
            let author_name = replace_umlauts(author_name);
            let navigators = get_navigators(commit_message);

            let inner_map = pair_counts.entry(author_name).or_insert_with(BTreeMap::new);

            if navigators.is_empty() {
                let single_counts = inner_map.entry(String::from("han_solo")).or_insert(0_u32);
                *single_counts += 1;
            }

            for navigator in navigators {
                let navigator = replace_umlauts(navigator);
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

    ui::render_coauthors(pair_counts)?

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

mod coauthor {

    #[derive(Debug, PartialEq)]
    pub struct CoAuthor<'a> {
        pub(crate) name: &'a str,
    }

    pub fn get_co_author(line: &str) -> Option<CoAuthor> {
        let (_, co_author) = co_author(line.as_bytes()).ok()?;
        let co_author = std::str::from_utf8(co_author).ok()?;
        Some(CoAuthor { name: co_author })
    }

    // sad Ferris :*(
    fn byte_string_trim(input: &[u8]) -> &[u8] {
        &input[0..input.len() - (input.iter().rev().take_while(|c| **c == b' ').count())]
    }

    named!(
        co_authored_by<Vec<&[u8]>>,
        many1!(ws!(tag_no_case!("co-authored-by:")))
    );
    named!(
        co_author<&[u8]>,
        preceded!(
            co_authored_by,
            map!(take_till!(|c| c == b'<'), byte_string_trim)
        )
    );

    //
    // fn sp<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    //     let chars = " \t\r\n";
    //
    //     // nom combinators like `take_while` return a function. That function is the
    //     // parser,to which we can pass the input
    //     take_while(move |c| chars.contains(c))(i)
    // }
    //
    // fn co_authored_by(input: &str) -> IResult<&str, ()> {
    //
    //
    //     // let foo:() = many1!(tag_no_case("co-authored-by:"));
    //
    //     let (input, _) = many1(preceded(tag_no_case("co-authored-by:"), sp))(input)?;
    //     Ok((input, ()))
    // }

    #[cfg(test)]
    mod tests {
        use test_case::test_case;

        use super::*;

        #[test_case("co-authored-by: Alice <alice@wonderland.org>",  "Alice <alice@wonderland.org>"; "lower case")]
        #[test_case("Co-Authored-By: Alice <alice@wonderland.org>",  "Alice <alice@wonderland.org>"; "camel case")]
        #[test_case("CO-AUTHORED-BY: Alice <alice@wonderland.org>",  "Alice <alice@wonderland.org>"; "upper case")]
        #[test_case("Co-authored-by: Alice <alice@wonderland.org>",  "Alice <alice@wonderland.org>"; "mixed case")]
        #[test_case("Co-authored-by: Co-authored-by: Alice <alice@wonderland.org>",  "Alice <alice@wonderland.org>"; "florentin case")]
        fn test_co_authored_by(input: &str, expected: &str) {
            let (result, _) = co_authored_by(input.as_bytes()).unwrap();
            assert_eq!(result, expected.as_bytes())
        }

        #[test_case("co-authored-by: Alice <alice@wonderland.org>",  "Alice" ; "alice")]
        #[test_case("co-authored-by: Alice Bob <alice@wonderland.org>",  "Alice Bob" ; "alice bob")]
        fn test_co_author(input: &str, expected: &str) {
            let (_, result) = co_author(input.as_bytes()).unwrap();
            assert_eq!(result, expected.as_bytes())
        }

        #[test_case("co-authored-by: Alice <alice@wonderland.org>" => Some("Alice") ; "alice")]
        #[test_case("co-authored-by: Alice Keys <alice@wonderland.org>" => Some("Alice Keys") ; "alice keys")]
        #[test_case("Some other content" => None ; "none")]
        fn test_get_co_author(input: &str) -> Option<&str> {
            get_co_author(input).map(|co_author| co_author.name)
        }
    }
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
