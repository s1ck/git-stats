use crate::{PairingCounts, Repo, Result};
use author_counts_view::AuthorCountsView;
use cursive::{
    align::{HAlign, VAlign},
    event::Key,
    menu::MenuTree,
    traits::{Nameable, Resizable, Scrollable},
    views::{Dialog, DummyView, EditView, LinearLayout, SelectView, TextView},
    Cursive, CursiveExt,
};
use cursive_tree_view::{Placement, TreeView};
use std::rc::Rc;

use crate::ui::hot_paths_view::{expand_tree, TreeEntry};
use std::env;
use std::path::{Path, PathBuf};

mod author_counts_view;
mod hot_paths_view;

pub(crate) fn render_hotpaths(
    path: Option<&Path>,
    repo: Repo,
    range: Option<String>,
) -> Result<()> {
    let current_dir: PathBuf;
    let path = match path {
        Some(p) => p,
        None => {
            current_dir = env::current_dir()?;
            current_dir.as_ref()
        }
    };

    let mut tree = TreeView::<TreeEntry>::new().on_submit(|c, index| {
        let tree = c.find_name::<TreeView<TreeEntry>>("tree").unwrap();
        // let text = c.find_name::<TextView>("text").unwrap();

        if let Some(entry) = tree.borrow_item(index) {
            c.call_on_name("text", |text: &mut TextView| {
                text.set_content(format!("{:#?}", entry));
            })
            .unwrap();
        }
    });

    tree.insert_item(
        TreeEntry {
            name: path.file_name().unwrap().to_str().unwrap().to_string(),
            dir: Some(path.to_path_buf()),
        },
        Placement::After,
        0,
    );

    expand_tree(&mut tree, 0, path);

    // Lazily insert directory listings for sub nodes
    tree.set_on_collapse(|siv: &mut Cursive, row, is_collapsed, children| {
        if !is_collapsed && children == 0 {
            siv.call_on_name("tree", move |tree: &mut TreeView<TreeEntry>| {
                if let Some(dir) = tree.borrow_item(row).unwrap().dir.clone() {
                    expand_tree(tree, row, &dir);
                }
            });
        }
    });

    // Setup Cursive
    let mut siv = Cursive::default();

    add_global_callbacks(&mut siv);

    // Let's add a ResizedView to keep the list at a reasonable size
    // (it can scroll anyway).
    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                Dialog::around(
                    tree.with_name("tree")
                        .scrollable()
                        .full_height()
                        .min_width(21), // .fixed_width(usize::from(app.author_widget_width()))
                )
                .title("File View"),
            )
            .child(DummyView.fixed_width(1))
            .child(
                Dialog::around(
                    TextView::new("select a file on the left")
                        .with_name("text")
                        .scrollable()
                        .full_height()
                        .full_width(),
                )
                .title("Commits"),
            )
            // .child(
            //     Dialog::around(counts_view.with_name("co-authors").full_width()) // TextView::new("foobar").with_name("co-authors")
            //         .title("Co-authors"),
            // )
            .full_screen(),
    );

    siv.run();

    Ok(())
}

pub(crate) fn render_coauthors(repo: Repo, range: Option<String>) -> Result<()> {
    let mut counts_view = AuthorCountsView::new(repo);

    let mut select = SelectView::<Rc<PairingCounts>>::new()
        // Center the text horizontally
        .h_align(HAlign::Left)
        .v_align(VAlign::Top)
        // Use keyboard to jump to the pressed letters
        .autojump()
        // show counts view when "Enter" is pressed
        .on_submit(show_co_authors);

    // add all authors
    let counts = counts_view.counts_for_range(range)?;
    let counts = counts
        .into_resolving_iter(&counts_view.string_cache())
        .map(|(author, counts)| (author, Rc::new(counts)));
    select.add_all(counts);

    // sort by author names
    select.sort_by_label();

    let mut siv = cursive::default();

    add_global_callbacks(&mut siv);

    let _ = siv
        .menubar()
        .add_subtree(
            "Filter",
            MenuTree::new().leaf("Commit range", show_range_dialog),
        )
        .add_delimiter()
        .add_leaf("Quit", Cursive::quit);

    siv.set_autohide_menu(false);

    // Let's add a ResizedView to keep the list at a reasonable size
    // (it can scroll anyway).
    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                Dialog::around(
                    select.with_name("committers").scrollable().full_height(), // .fixed_width(usize::from(app.author_widget_width()))
                )
                .title("Committer"),
            )
            .child(DummyView.fixed_width(1))
            .child(
                Dialog::around(counts_view.with_name("co-authors").full_width()) // TextView::new("foobar").with_name("co-authors")
                    .title("Co-authors"),
            )
            .full_screen(),
    );

    siv.run();

    Ok(())
}

fn show_co_authors(siv: &mut Cursive, counts: &Rc<PairingCounts>) {
    siv.call_on_name("co-authors", |app: &mut AuthorCountsView| {
        app.set_current_counts(Rc::clone(counts));
    })
    .unwrap();
}

fn show_range_dialog(siv: &mut Cursive) {
    disable_menu_bar(siv);

    fn ok(siv: &mut Cursive) {
        let range_start = siv
            .call_on_name("range_start", |view: &mut EditView| view.get_content())
            .unwrap();
        let range_end = siv
            .call_on_name("range_end", |view: &mut EditView| view.get_content())
            .unwrap();

        // set to full range if nothing is specified
        let range = if range_start.is_empty() && range_end.is_empty() {
            None
        } else {
            Some(format!("{}..{}", range_start, range_end))
        };

        let mut app = siv.find_name::<AuthorCountsView>("co-authors").unwrap();

        match app.counts_for_range(range) {
            Ok(counts) => {
                siv.call_on_name(
                    "committers",
                    move |select: &mut SelectView<Rc<PairingCounts>>| {
                        let counts = counts
                            .into_resolving_iter(&app.string_cache())
                            .map(|(author, counts)| (author, Rc::new(counts)));

                        select.clear();
                        select.add_all(counts);
                        select.sort_by_label();
                    },
                )
                .unwrap();
                // close range dialog
                let _ = siv.pop_layer();
                enable_menu_bar(siv)
            }
            Err(err) => {
                siv.add_layer(
                    Dialog::around(TextView::new(err.to_string()))
                        .title("Error")
                        .button("Ok", |s| {
                            let _ = s.pop_layer();
                        }),
                );
            }
        }
    }

    siv.add_layer(
        Dialog::around(
            LinearLayout::horizontal()
                .child(EditView::new().with_name("range_start").fixed_width(20))
                .child(TextView::new(".."))
                .child(EditView::new().with_name("range_end").fixed_width(20)),
        )
        .title("Enter commit range")
        .button("Ok", ok)
        .with_name("range_dialog"),
    );
}

fn add_global_callbacks(siv: &mut Cursive) {
    enable_menu_bar(siv);

    siv.add_global_callback(Key::F3, |s| {
        if s.find_name::<Dialog>("range_dialog").is_none() {
            show_range_dialog(s);
        }
    });

    siv.add_global_callback(Key::F10, Cursive::quit);
}

fn enable_menu_bar(siv: &mut Cursive) {
    siv.set_global_callback(Key::Esc, |s| s.select_menubar());
}

fn disable_menu_bar(siv: &mut Cursive) {
    siv.set_global_callback(Key::Esc, |_s| ());
}
