use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

/// Keeps track of the garbage collection roots.
///
/// The internal HashSet contains all the paths still in use. These paths
/// are used to find all **unused** paths and delete them.
#[derive(Debug)]
pub struct Roots(HashSet<PathBuf>);

impl Roots {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    /// Extend the garbage collection roots.
    ///
    /// Not only the file paths of roots themselves, but also all parent directories that should
    /// not be garbage collected need to be **explicitly** added to the roots. For example, if you
    /// have a path: `rootdir/example/file.txt`, the three paths: `rootdir`, `rootdir/example`, and
    /// `rootdir/example/file.txt` need to be added for the right files to be garbage collected.
    pub fn extend<'a>(&mut self, other: impl IntoIterator<Item = &'a PathBuf>) {
        self.0.extend(other.into_iter().cloned());
    }

    fn in_use(&self, entry: Option<&DirEntry>) -> bool {
        match entry {
            Some(e) => self.0.contains(e.path()),
            None => false,
        }
    }

    pub fn collect_garbage(&self, directory: impl AsRef<Path>) -> Result<()> {
        self.collect_garbage_with_filter(directory, |_| true)
    }

    /// Collect garbage with an additional filter.
    ///
    /// The filter function takes a &Path and returns a bool. The paths for which the filter
    /// function returns true are considered for garbage collection. This means that _only_ files
    /// that are unused AND for which the filter function returns true are deleted.
    pub fn collect_garbage_with_filter<P>(
        &self,
        directory: impl AsRef<Path>,
        mut predicate: P,
    ) -> Result<()>
    where
        P: FnMut(&Path) -> bool,
    {
        // Find all the paths not used anymore.
        let entries_not_in_use = WalkDir::new(directory.as_ref())
            .into_iter()
            .filter(|e| !self.in_use(e.as_ref().ok()))
            .filter(|e| match e.as_ref().ok() {
                Some(e) => predicate(e.path()),
                None => false,
            });

        // Remove all entries not in use.
        for e in entries_not_in_use {
            let entry = e?;
            let path = entry.path();
            log::debug!("Garbage collecting {path:?}...");

            if path.is_dir() {
                // If a directory is marked as unused all its children can be deleted too.
                fs::remove_dir_all(path)
                    .with_context(|| format!("Failed to remove directory: {:?}", path))?;
            } else {
                // Ignore failing to remove path because the parent directory might have been removed before.
                fs::remove_file(path).ok();
            };
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn keep_used_file() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let rootdir = create_dir(tmpdir.path().join("root"))?;

        let used_file = create_file(rootdir.join("root_file"))?;

        let mut roots = Roots::new();
        roots.extend(vec![&rootdir, &used_file]);
        roots.collect_garbage(&rootdir)?;

        assert!(used_file.exists());
        Ok(())
    }

    #[test]
    fn delete_unused_file() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let rootdir = create_dir(tmpdir.path().join("root"))?;

        let unused_file = create_file(rootdir.join("unused_file"))?;

        let mut roots = Roots::new();
        roots.extend(vec![&rootdir]);
        roots.collect_garbage(&rootdir)?;

        assert!(!unused_file.exists());
        Ok(())
    }

    #[test]
    fn delete_empty_unused_directory() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let rootdir = create_dir(tmpdir.path().join("root"))?;

        let unused_directory = create_dir(rootdir.join("unused_directory"))?;

        let mut roots = Roots::new();
        roots.extend(vec![&rootdir]);
        roots.collect_garbage(&rootdir)?;

        assert!(!unused_directory.exists());
        Ok(())
    }

    #[test]
    fn delete_unused_directory_with_unused_file_inside() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let rootdir = create_dir(tmpdir.path().join("root"))?;

        let unused_directory = create_dir(rootdir.join("unused_directory"))?;
        let unused_file_in_directory =
            create_file(unused_directory.join("unused_file_in_directory"))?;

        let mut roots = Roots::new();
        roots.extend(vec![&rootdir]);
        roots.collect_garbage(&rootdir)?;

        assert!(!unused_directory.exists());
        assert!(!unused_file_in_directory.exists());
        Ok(())
    }

    #[test]
    fn keep_used_directory_with_used_and_unused_file() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let rootdir = create_dir(tmpdir.path().join("root"))?;

        let used_directory = create_dir(rootdir.join("used_directory"))?;
        let used_file_in_directory = create_file(used_directory.join("used_file_in_directory"))?;
        let unused_file_in_directory =
            create_file(used_directory.join("unused_file_in_directory"))?;

        let mut roots = Roots::new();
        roots.extend(vec![&rootdir, &used_directory, &used_file_in_directory]);
        roots.collect_garbage(&rootdir)?;

        assert!(used_directory.exists());
        assert!(used_file_in_directory.exists());
        assert!(!unused_file_in_directory.exists());
        Ok(())
    }

    #[test]
    fn only_delete_filtered_unused_files() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let rootdir = create_dir(tmpdir.path().join("root"))?;

        let unused_file = create_file(rootdir.join("unused_file"))?;
        let unused_file_with_prefix = create_file(rootdir.join("prefix_unused_file"))?;

        let mut roots = Roots::new();
        roots.extend(vec![&rootdir]);
        roots.collect_garbage_with_filter(&rootdir, |p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with("prefix_"))
        })?;

        assert!(unused_file.exists());
        assert!(!unused_file_with_prefix.exists());
        Ok(())
    }

    fn create_file(path: PathBuf) -> Result<PathBuf> {
        fs::File::create(&path)?;
        Ok(path)
    }

    fn create_dir(path: PathBuf) -> Result<PathBuf> {
        fs::create_dir(&path)?;
        Ok(path)
    }
}
