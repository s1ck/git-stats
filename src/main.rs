#[macro_use]
extern crate maplit;

#[macro_use]
extern crate log;

use std::fs::File;
use std::path::PathBuf;

use app::App;
use clap::{AppSettings, Clap};
use cursive::align::{HAlign, VAlign};
use cursive::traits::{Nameable, Resizable, Scrollable};
use cursive::views::{Dialog, DummyView, EditView, LinearLayout, SelectView, TextView};
use cursive::{
    theme::{ColorStyle, PaletteColor},
    Cursive,
};
use eyre::Report;
use fehler::throws;
use repo::Repo;
use simplelog::*;

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
        .v_align(VAlign::Top)
        // Use keyboard to jump to the pressed letters
        .autojump();

    // add authors
    select.add_all_str(app.all_authors());
    select.sort();

    // Sets the callback for when "Enter" is pressed.
    select.set_on_submit(show_co_authors);

    let mut siv = cursive::default();

    siv.set_global_callback('Q', Cursive::quit);
    siv.set_global_callback('R', show_range_dialog);

    // Let's add a ResizedView to keep the list at a reasonable size
    // (it can scroll anyway).
    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                Dialog::around(
                    select
                        .with_name("committers")
                        .scrollable()
                        .full_height()
                        .fixed_width(usize::from(app.author_widget_width())),
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
        let range_start = siv
            .call_on_name("range_start", |view: &mut EditView| view.get_content())
            .unwrap();
        let range_end = siv
            .call_on_name("range_end", |view: &mut EditView| view.get_content())
            .unwrap();
        let range = format!("{}..{}", range_start, range_end);

        let mut app = siv.find_name::<App>("co-authors").unwrap();
        app.set_range_filter(range.as_str());
        app.on_enter();
        siv.pop_layer();
    }

    siv.add_layer(
        Dialog::around(
            LinearLayout::horizontal()
                .child(EditView::new().with_name("range_start").fixed_width(20))
                .child(TextView::new(".."))
                .child(EditView::new().with_name("range_end").fixed_width(20)),
        )
        .title("Enter commit range")
        .button("Ok", ok),
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BarPlacement {
    Left,
    Right,
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
            ColorStyle::new(PaletteColor::Primary, PaletteColor::HighlightInactive),
            Some(ColorStyle::primary()),
            BarPlacement::Right,
            printer,
        );

        self.draw_author_counts(
            author,
            self.navigator_tuples(&author),
            Some(max_co_author_count),
            ColorStyle::title_primary(),
            ColorStyle::new(PaletteColor::Primary, PaletteColor::Highlight),
            Some(ColorStyle::primary()),
            BarPlacement::Left,
            printer,
        );
    }
}

const FULL: &str = "█";
const SEVEN_EIGHTHS: &str = "▇";
const THREE_QUARTERS: &str = "▆";
const FIVE_EIGHTHS: &str = "▅";
const HALF: &str = "▄";
const THREE_EIGHTHS: &str = "▃";
const ONE_QUARTER: &str = "▂";
const ONE_EIGHTH: &str = "▁";
const EMPTY: &str = " ";

impl App {
    fn draw_author_counts(
        &self,
        author: usize,
        author_counts: Vec<(&str, u64)>,
        max_count: Option<u64>,
        color: ColorStyle,
        value_color: ColorStyle,
        text_color: Option<ColorStyle>,
        bar_placement: BarPlacement,
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

        let mut bar_gap = 2;

        let width_per_author =
            usize::from(printer.size.x - printer.offset.x) / author_counts.len().max(1);

        let mut bar_width = width_per_author.saturating_sub(usize::from(bar_gap)).max(1);
        if bar_width % 2 != 0 {
            bar_width += 1;
            bar_gap -= 1;
        }

        let max_y = printer.size.y - printer.offset.y;

        let max_top_y = printer.size.y.saturating_sub(1) as u64;

        for (index, (co_author, commits)) in author_counts.into_iter().enumerate() {
            let top = (max_top_y * commits / max_count) as usize;

            let scaled = 8 * max_top_y * commits / max_count;
            let last_block = scaled - (top as u64 * 8);

            info!(
                "index = {}, top = {}, last_block = {}, commits = {}, co_autor = {}",
                index, top, last_block, commits, co_author
            );

            let y = max_y.saturating_sub(top);
            let range = match bar_placement {
                BarPlacement::Left => 1..=(bar_width / 2),
                BarPlacement::Right => ((bar_width / 2) + 1)..=bar_width,
            };

            printer.with_color(color, |p| {
                for x in range {
                    let x = index * (bar_width + bar_gap) + bar_gap + (x - 1);
                    if top > 0 {
                        p.print_vline((x, y + 1), top - 1, FULL);
                        let symbol = match last_block {
                            0 => EMPTY,
                            1 => ONE_EIGHTH,
                            2 => ONE_QUARTER,
                            3 => THREE_EIGHTHS,
                            4 => HALF,
                            5 => FIVE_EIGHTHS,
                            6 => THREE_QUARTERS,
                            7 => SEVEN_EIGHTHS,
                            _ => FULL,
                        };
                        p.print((x, y), symbol);
                    }
                }
            });

            let x = index * (bar_width + bar_gap) + bar_gap;
            info!(
                "index = {}, x = {}, co_autor = {} commits = {}",
                index, x, co_author, commits
            );
            printer.with_color(value_color, |p| {
                let text_y = max_y;
                let text_x = match bar_placement {
                    BarPlacement::Left => x,
                    BarPlacement::Right => x + (bar_width / 2),
                };
                p.print((text_x, text_y), &format!("{:^1$}", commits, bar_width / 2));
            });
            if let Some(text_color) = text_color {
                printer.with_color(text_color, |p| {
                    p.print((x, max_y + 1), &format!("{:^1$.1$}", co_author, bar_width));
                })
            }
        }

        max_count
    }
}
