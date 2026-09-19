use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    DependencyScope, SourceFile, SourceId, SourceOrigin, SourceResolutionStratum, SourceSpan,
};
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    files: Vec<SourceFile>,
    /// Authorized product-scope dependency edges by requesting package:
    /// requester identity, then `alias -> target` identity. Only
    /// `DependencyPurpose::Product` edges may enter this roster; build-scope
    /// edges are compilation-local nameability and never become product
    /// selection authority
    /// (wiki/spec/build/scoped_execution.md, "Selecting product declarations
    /// without executing them").
    product_dependencies: BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
}

impl SourceMap {
    pub fn from_files(files: Vec<SourceFile>) -> Self {
        Self {
            files,
            product_dependencies: BTreeMap::new(),
        }
    }

    /// Record product-scope dependency edges keyed by requesting package.
    /// Aliases are already exact and per-package unique upstream.
    pub fn retain_product_dependency_scope<'a>(
        &mut self,
        edges: impl IntoIterator<Item = (PackageKeyIdentity, &'a str, PackageKeyIdentity)>,
    ) {
        for (requester, alias, target) in edges {
            self.product_dependencies
                .entry(requester)
                .or_default()
                .insert(alias.to_owned(), target);
        }
    }

    /// The product-scope dependency `alias` authorizes for `requester`, or
    /// `None` when the requester holds no such product edge. A build-scope
    /// alias, an unknown spelling, and a consumer's borrowed namespace all
    /// resolve the same way here: absent.
    pub fn product_dependency_target(
        &self,
        requester: PackageKeyIdentity,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.product_dependencies
            .get(&requester)?
            .get(alias)
            .copied()
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
        self.add_with_metadata_and_resolution_stratum(
            path,
            source,
            package_root,
            package_identity,
            origin,
            SourceResolutionStratum::Base,
        )
    }

    pub fn add_with_metadata_and_resolution_stratum(
        &mut self,
        path: PathBuf,
        source: String,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        resolution_stratum: SourceResolutionStratum,
    ) -> &SourceFile {
        self.add_checked_instance(
            path,
            source,
            package_root,
            package_identity,
            origin,
            resolution_stratum,
            DependencyScope::Product,
        )
    }

    /// Add one checked instance of a source. The same path may join once per
    /// dependency scope: the two instances share source bytes and package
    /// identity but nothing else
    /// (wiki/spec/build/scoped_execution.md, "Two checked contexts").
    pub fn add_checked_instance(
        &mut self,
        path: PathBuf,
        source: String,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        resolution_stratum: SourceResolutionStratum,
        dependency_scope: DependencyScope,
    ) -> &SourceFile {
        self.files.push(SourceFile {
            source_id: SourceId(self.files.len()),
            path,
            package_root,
            package_identity,
            dependency_scope,
            origin,
            resolution_stratum,
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

    /// Whether two declarations belong to one checked instance of a package:
    /// the same package identity AND the same dependency scope. Two checked
    /// instances of one source are the same package but never the same
    /// checked context — lexical sharing (module namespaces, package-private
    /// visibility, per-context target selection) keys off this predicate.
    pub fn same_checked_instance(&self, left: SourceSpan, right: SourceSpan) -> bool {
        match (self.file_at(left), self.file_at(right)) {
            (Some(left), Some(right)) => {
                left.dependency_scope == right.dependency_scope
                    && match (left.package_identity, right.package_identity) {
                        (Some(left), Some(right)) => left == right,
                        (None, None) => left.package_root == right.package_root,
                        _ => false,
                    }
            }
            _ => false,
        }
    }

    /// Whether a source-backed reference may resolve a declaration under the
    /// activation-local two-stratum rule.
    ///
    /// Base sources cannot observe declarations generated by the current
    /// activation. Extension sources may observe the complete retained base
    /// and every source in their extension stratum.
    pub fn reference_can_see_declaration(
        &self,
        reference: SourceSpan,
        declaration: SourceSpan,
    ) -> bool {
        if reference.span.start == reference.span.end {
            return true;
        }
        let Some(reference) = self.file_at(reference) else {
            return true;
        };
        let Some(declaration) = self.file_at(declaration) else {
            return true;
        };
        reference.resolution_stratum == SourceResolutionStratum::CurrentActivationExtension
            || declaration.resolution_stratum == SourceResolutionStratum::Base
    }

    pub fn resolution_strata_separate(&self, left: SourceSpan, right: SourceSpan) -> bool {
        match (self.file_at(left), self.file_at(right)) {
            (Some(left), Some(right)) => left.resolution_stratum != right.resolution_stratum,
            _ => false,
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Every exact source file retained by the frontend, in source-id order.
    /// Consumers that derive order-independent identity must sort their own
    /// canonical rows rather than treating load order as semantics.
    pub fn files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter()
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

    use crate::{SourceId, SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span};
    use semantic_vocabulary::PackageKeyIdentity;

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
    fn source_free_reference_remains_permissive_with_an_extension_declaration() {
        let mut sources = SourceMap::default();
        let extension = sources
            .add_with_metadata_and_resolution_stratum(
                PathBuf::from(".omega/generated/extension.omg"),
                String::from("data Extension {}"),
                PathBuf::from("."),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::CurrentActivationExtension,
            )
            .source_span(Span::new(5, 14));
        let source_free = sources
            .add(PathBuf::from("main.omg"), String::new())
            .source_span(Span::new(0, 0));

        assert!(sources.reference_can_see_declaration(SourceSpan::default(), extension));
        assert!(sources.reference_can_see_declaration(source_free, extension));
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
