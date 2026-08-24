use std::path::PathBuf;
use std::sync::Arc;

use crate::{SourceFile, SourceId, SourceOrigin, SourceSpan};
use psi_core::PackageKeyIdentity;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn from_files(files: Vec<SourceFile>) -> Self {
        Self { files }
    }

    pub fn add(&mut self, path: PathBuf, source: String) -> &SourceFile {
        let package_root = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        self.add_with_metadata(path, source, package_root, None, SourceOrigin::User)
    }

    pub fn add_with_metadata(
        &mut self,
        path: PathBuf,
        source: String,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
    ) -> &SourceFile {
        self.files.push(SourceFile {
            source_id: SourceId(self.files.len()),
            path,
            package_root,
            package_identity,
            origin,
            source: Arc::from(source),
        });

        self.files
            .last()
            .expect("source map should contain added file")
    }

    pub fn get(&self, source_id: SourceId) -> Option<&SourceFile> {
        self.files.get(source_id.0)
    }

    pub fn file_at(&self, source_span: SourceSpan) -> Option<&SourceFile> {
        self.get(source_span.source_id)
    }

    pub fn same_package(&self, left: SourceSpan, right: SourceSpan) -> bool {
        match (self.file_at(left), self.file_at(right)) {
            (Some(left), Some(right)) => match (left.package_identity, right.package_identity) {
                (Some(left), Some(right)) => left == right,
                (None, None) => left.package_root == right.package_root,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn text_at(&self, source_span: SourceSpan) -> &str {
        self.get(source_span.source_id)
            .map(|file| file.text_at(source_span.span))
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{SourceId, SourceMap, SourceSpan, Span};
    use psi_core::PackageKeyIdentity;

    #[test]
    fn resolves_source_span_text() {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add(PathBuf::from("main.omg"), String::from("machine main {}"))
            .source_id;
        let source_span = SourceSpan::new(source_id, Span::new(8, 12));

        assert_eq!(source_id, SourceId(0));
        assert_eq!(sources.text_at(source_span), "main");
    }

    #[test]
    fn invalid_source_span_resolves_to_empty_text() {
        let sources = SourceMap::default();
        let source_span = SourceSpan::new(SourceId(99), Span::new(0, 4));

        assert_eq!(sources.text_at(source_span), "");
    }

    #[test]
    fn reconciled_package_identity_supersedes_source_root_spelling() {
        let first_identity = PackageKeyIdentity::from_digest([1; 32]).expect("nonzero identity");
        let second_identity = PackageKeyIdentity::from_digest([2; 32]).expect("nonzero identity");
        let mut sources = SourceMap::default();
        let first = sources
            .add_with_metadata(
                PathBuf::from("cache/a.omg"),
                String::new(),
                PathBuf::from("cache"),
                Some(first_identity),
                crate::SourceOrigin::User,
            )
            .source_id;
        let second = sources
            .add_with_metadata(
                PathBuf::from("cache/b.omg"),
                String::new(),
                PathBuf::from("cache"),
                Some(second_identity),
                crate::SourceOrigin::User,
            )
            .source_id;
        let relocated = sources
            .add_with_metadata(
                PathBuf::from("other/c.omg"),
                String::new(),
                PathBuf::from("other"),
                Some(first_identity),
                crate::SourceOrigin::User,
            )
            .source_id;

        let span = |source_id| SourceSpan::new(source_id, Span::new(0, 0));
        assert!(!sources.same_package(span(first), span(second)));
        assert!(sources.same_package(span(first), span(relocated)));
    }
}
