//! Open the Product dependency named by a declaration query at its issuing occurrence.

use super::super::LogicalSourceDiscovery;
use super::{ProductQueryInputs, SourceStorage};
use diagnostics::Diagnostic;
use source_admission::SourceFile;
use source_text::{SourceSpan, Span};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DeclarationLookup {
    author: SourceFile,
    span: Span,
    alias: String,
    members: Vec<String>,
}

impl ProductQueryInputs {
    pub fn with_declaration_path(mut self, author: &SourceFile, span: Span, path: &[u8]) -> Self {
        // The semantic selector diagnoses malformed paths and handles ordinary local names.
        if let Ok(name) = std::str::from_utf8(path)
            && let Some((alias, path)) = name.split_once("::")
            && !alias.is_empty()
            && path.split("::").all(|member| !member.is_empty())
        {
            self.declaration = Some(DeclarationLookup {
                author: author.clone(),
                span,
                alias: alias.to_owned(),
                members: path.split("::").map(str::to_owned).collect(),
            });
        }
        self
    }

    pub(crate) fn declaration_sources(
        &self,
        storage: &mut SourceStorage,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let Some(lookup) = &self.declaration else {
            return Ok(Vec::new());
        };
        let author = storage
            .sources
            .rejoined_source_id(&lookup.author)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Product query author lost its admitted source correspondence",
                )]
            })?;
        if let Some(alias) = lookup
            .alias
            .strip_prefix('|')
            .and_then(|name| name.strip_suffix('|'))
        {
            use super::super::bindings::{ImportOccurrence, PendingPackageImport};
            use super::super::files::{captured_file, core_relative_path};
            use super::super::package_selection::ReconciledPackageImportRequest;
            use source_compiler::SourceImportTarget;

            let import = storage.source_roots.iter().find(|(_, roots)| roots.source_id == author)
                .and_then(|(_, roots)| roots.build_imports.iter().find(|import| matches!(&import.target, SourceImportTarget::File { alias: name, .. } if name == alias)));
            let Some(import) = import else {
                return Ok(Vec::new());
            };
            let SourceImportTarget::File { root, path, .. } = &import.target else {
                unreachable!()
            };
            let capture = |root: &std::path::Path, logical: &str| {
                captured_file(root, std::path::Path::new(logical), &storage.filesystem)?
                    .map(|file| vec![file])
                    .ok_or_else(|| {
                        vec![
                            Diagnostic::error(format!("no admitted logical file `{logical}`"))
                                .with_source_span(SourceSpan::new(author, import.span)),
                        ]
                    })
            };
            if root.as_deref() == Some("omega") {
                return capture(
                    &super::super::bundled_core_root()?,
                    core_relative_path(path).map_err(|reason| vec![Diagnostic::error(reason)])?,
                );
            }
            let requester = storage
                .sources
                .get(author)
                .expect("the Build author was just rejoined");
            let package = match root.as_deref() {
                None => requester.package_identity,
                Some(alias) => storage
                    .sources
                    .product_dependency_at(SourceSpan::new(author, lookup.span), alias),
            };
            let selected = package.and_then(|owner| {
                self.graphs()
                    .chain(self.named_source.iter())
                    .find_map(|graph| graph.package_root(owner).map(|root| (owner, root, graph)))
            });
            if let Some((owner, source_root, graph)) = selected {
                let request = PendingPackageImport {
                    occurrence: ImportOccurrence::file(
                        author,
                        import.ordinal,
                        import.target.to_string(),
                        alias,
                    ),
                    scope: source_provenance::DependencyScope::Product,
                    request: ReconciledPackageImportRequest::file(
                        owner,
                        source_root,
                        path,
                        &requester.path,
                        root.is_some(),
                    ),
                };
                return request
                    .resolve_for_exact_target(graph, &mut storage.package_roots)
                    .map(|resolved| resolved.into_iter().map(|file| file.path).collect());
            }
            return if root.is_none() {
                capture(&requester.package_root, path)
            } else {
                Ok(Vec::new())
            };
        }
        let Some(package) = storage
            .sources
            .product_dependency_at(SourceSpan::new(author, lookup.span), &lookup.alias)
        else {
            return Ok(Vec::new());
        };
        let root = self
            .graphs()
            .find_map(|graph| graph.package_root(package))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Product declaration dependency has no admitted source capture",
                )]
            })?;
        let excluded = storage
            .package_roots
            .iter()
            .filter(|owner| owner.root != root && owner.root.starts_with(root))
            .map(|owner| owner.root.clone())
            .collect();
        storage
            .package_roots
            .select_declaring(root, &lookup.members, &storage.filesystem, excluded)
    }
}
