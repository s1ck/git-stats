#![allow(deprecated)]

#[macro_use]
extern crate nom;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{AppSettings, Clap};
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

            let navigators = get_navigators(commit_message);

            if navigators.is_empty() {
                let single_driver = "single_driver";
                let single_counts = match inner_map.get_mut(single_driver)  {
                    Some(inner_map) => inner_map,
                    None => {
                        inner_map.insert(String::from(single_driver), 0_u32);
                        inner_map.get_mut(single_driver).unwrap()
                    }
                };
                *single_counts += 1;
            }

            for navigator in navigators {
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
        .filter_map(|line| coauthor::get_co_author(line))
        .map(|coauthor| coauthor.name)
        .collect()
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

    named!(co_authored_by<Vec<&[u8]>>, many1!(ws!(tag_no_case!("co-authored-by:"))));
    named!(co_author<&[u8]>, preceded!(co_authored_by, map!(take_till!(|c| c == b'<'), byte_string_trim)));

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