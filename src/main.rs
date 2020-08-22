#[macro_use]
extern crate maplit;

use std::path::PathBuf;

use clap::{AppSettings, Clap};
use eyre::Report;
use fehler::throws;

use crate::repo::Repo;

mod repo;
mod stringcache;
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
    /// Stop execution after parsing, don't show any UI
    #[clap(long)]
    stop: bool,
}

/// Parse a replacement key-value pair
fn parse_key_val(s: &str) -> eyre::Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| eyre::eyre!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].into(), s[pos + 1..].into()))
}

#[throws(Report)]
fn main() {
    color_eyre::install()?;
    let opts: Opts = Opts::parse();

    let Opts {
        repository,
        replacements,
        stop,
        range,
    } = opts;

    let mut repo = Repo::open(repository, replacements)?;

    let (driver_counts, pair_counts) = repo.extract_coauthors(range)?;

    if !stop {
        ui::render_coauthors(driver_counts, pair_counts, repo.into_string_cache())?
    }
}
