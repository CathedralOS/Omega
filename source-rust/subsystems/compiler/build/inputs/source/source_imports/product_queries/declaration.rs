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
            return super::super::files::product_file_sources(
                storage,
                SourceSpan::new(author, lookup.span),
                alias,
                self.graphs().chain(self.named_source.iter()),
            );
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
