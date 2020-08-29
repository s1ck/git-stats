use crate::{AuthorCounts, PairingCounts, Repo, Result, StringCache};
use cursive::{
    theme::{ColorStyle, PaletteColor},
    View,
};
use std::rc::Rc;

pub(crate) struct AuthorCountsView {
    current_counts: Option<Rc<PairingCounts>>,
    repo: Repo,
}

impl AuthorCountsView {
    pub(crate) fn new(repo: Repo) -> AuthorCountsView {
        AuthorCountsView {
            current_counts: Default::default(),
            repo,
        }
    }

    pub(crate) fn string_cache(&self) -> &StringCache {
        self.repo.string_cache()
    }

    pub(crate) fn set_current_counts(&mut self, counts: Rc<PairingCounts>) {
        let _ = self.current_counts.replace(counts);
    }

    pub(crate) fn counts_for_range(&mut self, range: Option<String>) -> Result<AuthorCounts> {
        self.repo.extract_coauthors(range)
    }

    fn current_counts(&self) -> Option<&PairingCounts> {
        self.current_counts.as_ref().map(|c| &**c)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BarPlacement {
    Left,
    Right,
    Full,
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

const BAR_GAP: usize = 2;

impl View for AuthorCountsView {
    fn draw(&self, printer: &cursive::Printer<'_, '_>) {
        let counts = match self.current_counts() {
            Some(counts) => counts,
            None => return,
        };

        // calculate bar width and gap
        // the width is guaranteed to be an even number
        // as we want to split the bar into 2
        let max_x = usize::from(printer.size.x - printer.offset.x);
        let data_points = counts.len().max(1);
        let width_per_author = max_x / data_points;
        let mut bar_gap = BAR_GAP;
        let mut bar_width = width_per_author.saturating_sub(usize::from(bar_gap)).max(1);
        if bar_width % 2 != 0 {
            bar_width += 1;
            bar_gap -= 1;
        }
        if data_points > 1 {
            while ((data_points * bar_width) + ((data_points - 1) * (bar_gap + 1))) <= max_x {
                bar_gap += 1;
            }
        }

        // get to available height of the current screen segment
        let max_view_y = printer.size.y - printer.offset.y + 1;
        let max_y = max_view_y.saturating_sub(1) as u32;

        let max_count = counts.max_value().max(1);
        let count_iter = counts.resolving_iter(self.string_cache());

        // colors
        let driver_bar_color = ColorStyle::title_primary();
        let driver_value_color = ColorStyle::new(PaletteColor::Primary, PaletteColor::TitlePrimary);
        let all_bar_color = ColorStyle::title_secondary();
        let all_value_color = ColorStyle::new(PaletteColor::Primary, PaletteColor::TitleSecondary);
        let name_color = ColorStyle::primary();

        let draw_author_bar_inner = |index: usize,
                                     count: u32,
                                     color: ColorStyle,
                                     value_color: ColorStyle,
                                     bar_placement: BarPlacement|
         -> usize {
            let top = max_y * count / max_count;
            let scaled = 8 * max_y * count / max_count;
            let last_block = scaled - (top * 8);

            let y = max_y.saturating_sub(top) as usize;
            let top = top as usize;

            let x_range = match bar_placement {
                BarPlacement::Left => 1..=(bar_width / 2),
                BarPlacement::Right => ((bar_width / 2) + 1)..=bar_width,
                BarPlacement::Full => 1..=bar_width,
            };

            printer.with_color(color, |p| {
                for x in x_range {
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
                let text_y = max_y as usize;
                let text_x = match bar_placement {
                    BarPlacement::Right => x + (bar_width / 2),
                    BarPlacement::Left | BarPlacement::Full => x,
                };
                let text_width = match bar_placement {
                    BarPlacement::Full => bar_width,
                    BarPlacement::Left | BarPlacement::Right => bar_width / 2,
                };
                p.print((text_x, text_y), &format!("{:^1$}", count, text_width));
            });

            x
        };

        for (index, (co_author, commits)) in count_iter.into_iter().enumerate() {
            let name_pos = if commits.as_driver == 0 {
                draw_author_bar_inner(
                    index,
                    commits.total,
                    all_bar_color,
                    all_value_color,
                    BarPlacement::Full,
                )
            } else if commits.as_driver == commits.total {
                draw_author_bar_inner(
                    index,
                    commits.as_driver,
                    driver_bar_color,
                    driver_value_color,
                    BarPlacement::Full,
                )
            } else {
                let _ = draw_author_bar_inner(
                    index,
                    commits.as_driver,
                    driver_bar_color,
                    driver_value_color,
                    BarPlacement::Left,
                );
                draw_author_bar_inner(
                    index,
                    commits.total,
                    all_bar_color,
                    all_value_color,
                    BarPlacement::Right,
                )
            };

            printer.with_color(name_color, |p| {
                p.print(
                    (name_pos, max_view_y),
                    &format!("{:^1$.1$}", co_author, bar_width),
                );
            });
        }
    }
}
