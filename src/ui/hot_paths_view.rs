use core::fmt;
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::path::{PathBuf, Path};
use std::{fs, io};

use cursive_tree_view::{Placement, TreeView};
use ignore::{Walk, WalkBuilder};

#[derive(Debug)]
pub(crate) struct TreeEntry {
    pub(crate) name: String,
    pub(crate) dir: Option<PathBuf>,
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

            if path.is_dir() {
                entries.push(TreeEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    dir: Some(entry.into_path()),
                });
            } else if path.is_file() {
                entries.push(TreeEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    dir: None,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn expand_tree(tree: &mut TreeView<TreeEntry>, parent_row: usize, dir: &Path) {
    let mut entries = Vec::new();
    collect_entries(dir, &mut entries).ok();

    for i in entries {
        if i.dir.is_some() {
            tree.insert_container_item(i, Placement::LastChild, parent_row);
        } else {
            tree.insert_item(i, Placement::LastChild, parent_row);
        }
    }
}
