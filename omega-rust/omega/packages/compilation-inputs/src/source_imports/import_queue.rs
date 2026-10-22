use std::collections::{HashMap, VecDeque, hash_map::Entry};
use std::path::{Path, PathBuf};

use source::DependencyScope;

/// Pending source loads keyed by checked instance: one path may join once
/// per dependency scope (wiki/spec/build/scoped_execution.md, "Two checked
/// contexts"). Dedupe is therefore `(path, scope)`, not path alone — a
/// source both purposes select loads a second time under its other scope.
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

    /// Mark `path` as having joined the compilation under `scope` without a
    /// physical load — generated and injected sources are retained by their
    /// producer, not read from disk.
    pub fn mark_loaded(&mut self, path: &Path, scope: DependencyScope) {
        self.seen.insert((path.to_path_buf(), scope), ());
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn take_frontier(&mut self) -> Vec<(PathBuf, DependencyScope)> {
        (!self.pending.is_empty())
            .then(|| self.pending.drain(..).collect())
            .unwrap_or_default()
    }

    fn push(&mut self, path: PathBuf, scope: DependencyScope) {
        // Move the lookup key first; only a new checked instance needs a
        // second owned path for its pending load. Entry avoids hashing twice.
        if let Entry::Vacant(entry) = self.seen.entry((path, scope)) {
            self.pending.push_back(entry.key().clone());
            entry.insert(());
        }
    }
}
