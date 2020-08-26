#[macro_use]
extern crate maplit;

#[macro_use]
extern crate log;

use std::convert::TryFrom;
use std::fs::File;
use std::path::PathBuf;

use app::App;
use clap::{AppSettings, Clap};
use cursive::align::{HAlign, VAlign};
use cursive::event::EventResult;
use cursive::traits::*;
use cursive::views::{Dialog, DummyView, LinearLayout, OnEventView, SelectView, TextView, CircularFocus, EditView};
use cursive::{
    direction::Orientation,
    theme::{BaseColor, Color, ColorStyle},
    Cursive,
};
use eyre::Report;
use fehler::throws;
use repo::Repo;
use simplelog::*;
use std::rc::Rc;

mod app;
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

    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create(&format!("{}.log", repo::APPLICATION)).unwrap(),
    )?;

    let Opts {
        repository,
        replacements,
        range,
    } = opts;

    let repo = Repo::open(repository, replacements)?;
    // ui::render_coauthors(repo, range)?

    let app = App::new(repo, range);

    let mut select = SelectView::new()
        // Center the text horizontally
        .h_align(HAlign::Left)
        .v_align(VAlign::Top);
    // Use keyboard to jump to the pressed letters
    // .autojump();

    // add authors
    select.add_all_str(app.all_authors());
    select.sort();

    // Sets the callback for when "Enter" is pressed.
    select.set_on_submit(show_co_authors);

    // Let's override the `j` and `k` keys for navigation
    let select = OnEventView::new(select)
        .on_pre_event_inner('k', |s, _| {
            s.select_up(1);
            Some(EventResult::Consumed(None))
        })
        .on_pre_event_inner('j', |s, _| {
            s.select_down(1);
            Some(EventResult::Consumed(None))
        });

    let mut siv = cursive::default();

    siv.set_global_callback('Q', Cursive::quit);
    siv.set_global_callback('R', show_range_dialog);

    // Let's add a ResizedView to keep the list at a reasonable size
    // (it can scroll anyway).
    siv.add_layer(
        LinearLayout::horizontal()
            .child(
                Dialog::around(
                    select
                        .with_name("committers")
                        .scrollable()
                        .full_height()
                        .fixed_width(15),
                )
                .title("Committer"),
            )
            .child(DummyView.fixed_width(1))
            .child(
                Dialog::around(app.with_name("co-authors").full_width()) // TextView::new("foobar").with_name("co-authors")
                    .title("Co-authors"),
            )
            .full_screen(),
    );

    siv.run();
}

fn show_co_authors(siv: &mut Cursive, committer: &str) {
    let mut app = siv.find_name::<App>("co-authors").unwrap();
    app.set_current_author(committer);
}

fn show_range_dialog(siv: &mut Cursive) {
    fn ok(siv: &mut Cursive) {
        let range_start = siv.call_on_name("range_start", |view: &mut EditView| {
            view.get_content()
        }).unwrap();
        let range_end = siv.call_on_name("range_end", |view: &mut EditView| {
            view.get_content()
        }).unwrap();
        let range = format!("{}..{}", range_start, range_end);

        let mut app = siv.find_name::<App>("co-authors").unwrap();
        app.set_range_filter(range.as_str());
        app.on_enter();
        siv.pop_layer();
    }

    siv.add_layer(
        Dialog::around(
        LinearLayout::horizontal()
            .child(
                EditView::new()
                    .with_name("range_start")
                    .fixed_width(20),
            )
            .child(TextView::new(".."))
            .child(
                EditView::new()
                    .with_name("range_end")
                    .fixed_width(20),
            ),
        )
            .title("Enter commit range")
            .button("Ok", ok)
    );
}

impl cursive::view::View for App {
    fn draw(&self, printer: &cursive::Printer) {
        let author = self.current_author();
        let author = match author {
            Some(author) => author,
            None => return,
        };

        let max_co_author_count = self.draw_author_counts(
            author,
            self.co_author_tuples(&author),
            None,
            ColorStyle::title_secondary(),
            printer,
        );

        self.draw_author_counts(
            author,
            self.navigator_tuples(&author),
            Some(max_co_author_count),
            ColorStyle::title_primary(),
            printer,
        );
    }
}

impl App {
    fn draw_author_counts(
        &self,
        author: usize,
        author_counts: Vec<(&str, u64)>,
        max_count: Option<u64>,
        color: ColorStyle,
        printer: &cursive::Printer,
    ) -> u64 {
        let max_count = max_count
            .or_else(|| author_counts.iter().map(|(_, commits)| *commits).max())
            .unwrap_or_default()
            .max(1);

        info!(
            "max_count = {:?}  author = {:?}",
            max_count,
            self.repo.string_cache().get(author).unwrap_or("???")
        );
        info!(
            "printer.size = {:?}  printer.offset = {:?}  printer.output_size = {:?}  printer.content_offset = {:?}",
            printer.size, printer.offset, printer.output_size, printer.content_offset,
        );

        for (index, (co_author, commits)) in author_counts.into_iter().enumerate() {
            let top =
                usize::try_from((printer.size.y - printer.offset.y) as u64 * commits / max_count)
                    .unwrap();
            info!(
                "index = {}, top = {}, commits = {}, co_autor = {}",
                index, top, commits, co_author
            );

            let bar_width = 3;
            let bar_gap = 2;

            printer.with_color(color, |p| {
                for x in 1..=bar_width {
                    let x = index * (bar_width + bar_gap) + bar_gap + (x - 1);
                    p.print_vline((x, printer.size.y - top), top, tui::symbols::bar::FULL)
                }
            })
        }

        max_count
    }
}
