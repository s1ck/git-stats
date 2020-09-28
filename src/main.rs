#[macro_use]
extern crate eyre;
#[macro_use]
extern crate maplit;

use std::fs::File;
use std::path::PathBuf;

use clap::{AppSettings, Clap};
use eyre::Result;
use log::LevelFilter;
use simplelog::{CombinedLogger, Config, WriteLogger};

use crate::{
    author_counts::{AuthorCounts, PairingCounts},
    repo::{Repo, HAN_SOLO},
    stringcache::StringCache,
};

mod author_counts;
mod repo;
mod stringcache;
mod author_path_counts;
mod ui;

#[derive(Clap, Debug)]
#[clap(version, author, about, global_setting = AppSettings::ColoredHelp)]
struct Opts {
    /// Path to Git repository
    #[clap(short, long)]
    repository: Option<PathBuf>,
    /// Replace authors based on this map. Can be specified multiple times, value are delimited by `=`
    #[clap(short = "R", long="replacement", parse(try_from_str = parse_key_val), number_of_values = 1)]
    replacements: Vec<(String, String)>,
    /// Commit range to scan. Default is to go from HEAD to the very beginning.
    ///
    /// This accepts the form of `<commit-1>..<commit-2>` and will start scanning at `commit-2` and stop at `commit-1`.
    /// The default can be seen as if it was defined as `..HEAD`.
    #[clap(long)]
    range: Option<String>,
}

/// Parse a replacement key-value pair
fn parse_key_val(s: &str) -> Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| eyre::eyre!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].into(), s[pos + 1..].into()))
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let opts: Opts = Opts::parse();

    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("git-stats.log")?,
    )?;

    let Opts {
        repository,
        replacements,
        range,
    } = opts;

    let repo = Repo::open(repository.as_ref(), replacements)?;
    // ui::render_coauthors(repo, range)
    ui::render_path_counts(repo, range)
}
