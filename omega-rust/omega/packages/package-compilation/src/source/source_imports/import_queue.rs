// Scope-keyed `(PathBuf, DependencyScope)` pending source import queue (`ImportQueue`).
//
// Because a single file path may join compilation once in `DependencyScope::Build`
// and once in `DependencyScope::Product` (`wiki/spec/build/scoped_execution.md`),
// deduplication is keyed on `(PathBuf, DependencyScope)` rather than `PathBuf` alone.

use std::collections::{HashMap, VecDeque, hash_map::Entry};
use std::path::{Path, PathBuf};

use source::DependencyScope;

#[derive(Debug, Default)]
pub struct ImportQueue {
    seen: HashMap<(PathBuf, DependencyScope), ()>,
    pending: VecDeque<(PathBuf, DependencyScope)>,
}

impl ImportQueue {
    pub fn seed(&mut self, path: PathBuf, scope: DependencyScope) {
        self.push(path, scope);
    }

    pub fn enqueue(
        &mut self,
        imports: Vec<(PathBuf, DependencyScope)>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        for (path, scope) in imports {
            self.push(path, scope);
        }

        Ok(())
    }

    pub fn mark_loaded(&mut self, path: &Path, scope: DependencyScope) {
        self.seen.insert((path.to_path_buf(), scope), ());
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn take_frontier(&mut self) -> Vec<(PathBuf, DependencyScope)> {
        if !self.pending.is_empty() {
            self.pending.drain(..).collect()
        } else {
            Default::default()
        }
    }

    fn push(&mut self, path: PathBuf, scope: DependencyScope) {
        if let Entry::Vacant(entry) = self.seen.entry((path, scope)) {
            self.pending.push_back(entry.key().clone());
            entry.insert(());
        }
    }
}
