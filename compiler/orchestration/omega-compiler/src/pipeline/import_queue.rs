use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct ImportQueue {
    seen: HashSet<PathBuf>,
    pending: VecDeque<PathBuf>,
}

impl ImportQueue {
    pub fn push(&mut self, path: PathBuf) {
        if self.seen.insert(path.clone()) {
            self.pending.push_back(path);
        }
    }

    pub fn pop_all(&mut self) -> Option<Vec<PathBuf>> {
        (!self.pending.is_empty()).then(|| self.pending.drain(..).collect())
    }
}
