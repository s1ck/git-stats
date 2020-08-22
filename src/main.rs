#[macro_use]
extern crate maplit;

use std::path::PathBuf;

use clap::{AppSettings, Clap};
use eyre::Report;
use fehler::throws;

use crate::repo::Repo;

mod repo;
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

    let Opts{ repository, replacements } = opts;

    let repo = Repo::open(repository, replacements)?;

    let (driver_counts, pair_counts) = repo.extract_coauthors()?;

    ui::render_coauthors(driver_counts, pair_counts)?
}
