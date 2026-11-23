//! A source already supplied by a dependency's Build has no file to read.
//! The same path can be supplied separately to Build and Product.

use std::collections::{HashMap, hash_map::Entry};
use std::path::{Path, PathBuf};

use source_provenance::DependencyScope;

#[derive(Debug)]
enum ImportSource {
    CapturedFile,
    AlreadyLoaded,
}

#[derive(Debug, Default)]
pub struct ImportQueue {
    sources: HashMap<(PathBuf, DependencyScope), ImportSource>,
    pending: Vec<(PathBuf, DependencyScope)>,
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
        self.sources
            .insert((path.to_path_buf(), scope), ImportSource::AlreadyLoaded);
    }

    pub fn take_frontier(&mut self) -> Option<Vec<(PathBuf, DependencyScope)>> {
        let mut pending = std::mem::take(&mut self.pending);
        pending.retain(|file| matches!(self.sources.get(file), Some(ImportSource::CapturedFile)));
        (!pending.is_empty()).then_some(pending)
    }

    fn push(&mut self, path: PathBuf, scope: DependencyScope) {
        if let Entry::Vacant(entry) = self.sources.entry((path, scope)) {
            self.pending.push(entry.key().clone());
            entry.insert(ImportSource::CapturedFile);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DependencyScope::{Build, Product};

    #[test]
    fn supplying_a_queued_import_cancels_only_its_own_file_read() {
        let mut imports = ImportQueue::default();
        let source = PathBuf::from("generated.omg");
        let authored = PathBuf::from("authored.omg");
        imports.seed(source.clone(), Build);
        imports.seed(authored.clone(), Build);
        imports.seed(source.clone(), Product);
        imports.mark_loaded(&source, Build);
        imports.seed(source.clone(), Build);

        assert_eq!(
            imports.take_frontier(),
            Some(vec![(authored, Build), (source, Product)])
        );
        assert_eq!(imports.take_frontier(), None);
    }
}
