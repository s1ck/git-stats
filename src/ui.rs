use crate::{app::App, repo::Repo};

use cursive::align::{HAlign, VAlign};
use cursive::event::Key;
use cursive::traits::{Nameable, Resizable, Scrollable};
use cursive::views::{Dialog, DummyView, EditView, LinearLayout, SelectView, TextView};
use cursive::{
    theme::{ColorStyle, PaletteColor},
    view::View,
    Cursive,
};

pub fn render_coauthors(repo: Repo, range: Option<String>) -> eyre::Result<()> {
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
    siv.set_global_callback(Key::F3, show_range_dialog);

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

    Ok(())
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
        app.update_counts_from_repo();
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

        let mut bar_gap = 2;

        let width_per_author =
            usize::from(printer.size.x - printer.offset.x) / author_counts.len().max(1);

        let mut bar_width = width_per_author.saturating_sub(usize::from(bar_gap)).max(1);
        if bar_width % 2 != 0 {
            bar_width += 1;
            bar_gap -= 1;
        }

        let maxest_y = printer.size.y - printer.offset.y;
        let max_y = maxest_y.saturating_sub(1);
        let max_top_y = maxest_y as u64;

        for (index, (co_author, commits)) in author_counts.into_iter().enumerate() {
            let top = (max_top_y * commits / max_count) as usize;

            let scaled = 8 * max_top_y * commits / max_count;
            let last_block = scaled - (top as u64 * 8);

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
                    p.print((x, maxest_y), &format!("{:^1$.1$}", co_author, bar_width));
                })
            }
        }

        max_count
    }
}

impl View for App {
    fn draw(&self, printer: &cursive::Printer) {
        let author = self.current_author();
        let author = match author {
            Some(author) => author,
            None => return,
        };

        let max_co_author_count = self.draw_author_counts(
            self.co_author_tuples(&author),
            None,
            ColorStyle::title_secondary(),
            ColorStyle::new(PaletteColor::Primary, PaletteColor::HighlightInactive),
            Some(ColorStyle::primary()),
            BarPlacement::Right,
            printer,
        );

        self.draw_author_counts(
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
