use core::fmt;
use std::{fs, io};
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread::current;

use cursive::{Cursive, Printer, View};
use cursive::event::{Callback, Event, EventResult, Key};
use cursive_tree_view::{Placement, TreeView};
use ignore::{Walk, WalkBuilder};
use log::info;

use crate::author_modifications::AuthorModifications;
use crate::repo::Repo;

pub(crate) struct AuthorModificationsView {
    current_counts: Option<AuthorModifications>,
    repo: Repo,
    range: Option<String>
}

impl AuthorModificationsView {
    pub fn new(repo: Repo, range: Option<String>) -> AuthorModificationsView {
        AuthorModificationsView {
            current_counts: Some(AuthorModifications::default()),
            repo,
            range
        }
    }

    pub(crate) fn update_counts(&mut self, absolute_path: &Path) {
        let workdir = self.repo.workdir().unwrap();
        let relative_path = absolute_path.strip_prefix(workdir).unwrap();
        let option = self.range.as_ref();
        let counts = self.repo.extract_author_modifications(relative_path, option).unwrap();
        self.current_counts = Some(counts);
    }
}

impl View for AuthorModificationsView {
    fn draw(&self, printer: &Printer<'_, '_>) {
        printer.print((0, 0), format!("{:?}", self.current_counts).as_str())
    }
}

#[derive(Debug)]
pub(crate) struct TreeEntry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
}

impl TreeEntry {
    pub fn new(path: &Path) -> TreeEntry {
        TreeEntry {
            name: path.file_name().unwrap().to_str().unwrap().to_string(),
            path: path.to_path_buf(),
            is_dir: path.is_dir(),
        }
    }
}

impl fmt::Display for TreeEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub(crate) fn collect_entries(dir: &Path, entries: &mut Vec<TreeEntry>) -> eyre::Result<()> {
    if dir.is_dir() {
        let walk = WalkBuilder::new(dir)
            .max_depth(Some(1))
            .sort_by_file_path(|p1, p2| match (p1.is_dir(), p2.is_dir()) {
                (true, true) | (false, false) => p1.cmp(&p2),
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
            })
            .build();

        for entry in walk {
            let entry = entry?;
            let path = entry.path();

            if path == dir {
                continue;
            }

            entries.push(TreeEntry::new(entry.into_path().as_ref()));
        }
    }
    Ok(())
}

pub(crate) fn expand_tree(tree: &mut TreeView<TreeEntry>, parent_row: usize, dir: &Path) {
    let mut entries = Vec::new();
    collect_entries(dir, &mut entries).ok();


    for i in entries {
        if i.is_dir {
            tree.insert_container_item(i, Placement::LastChild, parent_row);
        } else {
            tree.insert_item(i, Placement::LastChild, parent_row);
        }
    }
}

pub(crate) struct CustomTreeView(pub TreeView<TreeEntry>, pub Rc<dyn Fn(&mut Cursive, usize)>);

impl Deref for CustomTreeView {
    type Target = TreeView<TreeEntry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CustomTreeView {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl View for CustomTreeView {
    fn draw(&self, printer: &cursive::Printer) {
        View::draw(&self.0, printer)
    }

    fn layout(&mut self, v: cursive::Vec2) {
        View::layout(&mut self.0, v)
    }

    fn needs_relayout(&self) -> bool {
        View::needs_relayout(&self.0)
    }

    fn required_size(&mut self, constraint: cursive::Vec2) -> cursive::Vec2 {
        View::required_size(&mut self.0, constraint)
    }

    fn on_event(&mut self, event: cursive::event::Event) -> cursive::event::EventResult {
        if !self.is_enabled() {
            return EventResult::Ignored;
        }

        match event {
            Event::Char(' ') | Event::Key(Key::Left) | Event::Key(Key::Right) =>
                View::on_event(&mut self.0, Event::Key(Key::Enter)),
            Event::Key(Key::Enter) => {
                if !self.is_empty() {
                    let row = self.row().unwrap_or_default();
                    let cb = Rc::clone(&self.1);
                    EventResult::Consumed(Some(Callback::from_fn(move |s| cb(s, row))))
                } else {
                    EventResult::Ignored
                }
            }
            e => View::on_event(&mut self.0, e),
        }
    }

    fn call_on_any<'a>(
        &mut self,
        sel: &cursive::view::Selector<'_>,
        cb: cursive::event::AnyCb<'a>,
    ) {
        View::call_on_any(&mut self.0, sel, cb)
    }

    fn focus_view(&mut self, sel: &cursive::view::Selector<'_>) -> Result<(), ()> {
        View::focus_view(&mut self.0, sel)
    }

    fn take_focus(&mut self, source: cursive::direction::Direction) -> bool {
        View::take_focus(&mut self.0, source)
    }

    fn important_area(&self, view_size: cursive::Vec2) -> cursive::Rect {
        View::important_area(&self.0, view_size)
    }

    fn type_name(&self) -> &'static str {
        View::type_name(&self.0)
    }
}
