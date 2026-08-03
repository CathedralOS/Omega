use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct ImportQueue {
    seen: HashSet<PathBuf>,
    pending: VecDeque<PathBuf>,
}

impl ImportQueue {
    pub fn seed(&mut self, path: PathBuf) {
        self.push(path);
    }

    pub fn enqueue(&mut self, paths: Vec<PathBuf>) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
        for path in paths {
            self.push(path);
        }

        Ok(())
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn take_frontier(&mut self) -> Vec<PathBuf> {
        self.pop_all().unwrap_or_default()
    }

    fn push(&mut self, path: PathBuf) {
        if self.seen.insert(path.clone()) {
            self.pending.push_back(path);
        }
    }

    fn pop_all(&mut self) -> Option<Vec<PathBuf>> {
        (!self.pending.is_empty()).then(|| self.pending.drain(..).collect())
    }
}
