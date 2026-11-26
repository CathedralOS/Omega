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
            self.declarations.push(DeclarationLookup {
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
        let mut paths = Vec::new();
        for lookup in &self.declarations {
            paths.extend(lookup.sources(self, storage)?);
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub fn with_successor_entry(mut self, entry: build_declarations::BuildEntryPath) -> Self {
        self.successor = Some(entry);
        self
    }

    pub(crate) fn successor_entry(&self) -> Option<&build_declarations::BuildEntryPath> {
        self.successor.as_ref()
    }
}

impl DeclarationLookup {
    fn sources(
        &self,
        inputs: &ProductQueryInputs,
        storage: &mut SourceStorage,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let author = storage
            .sources
            .rejoined_source_id(&self.author)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Product query author lost its admitted source correspondence",
                )]
            })?;
        if let Some(alias) = self
            .alias
            .strip_prefix('|')
            .and_then(|name| name.strip_suffix('|'))
        {
            return super::super::files::product_file_sources(
                storage,
                SourceSpan::new(author, self.span),
                alias,
                inputs.graphs().chain(inputs.named_source.iter()),
            );
        }
        let Some(package) = storage
            .sources
            .product_dependency_at(SourceSpan::new(author, self.span), &self.alias)
        else {
            return Ok(Vec::new());
        };
        let root = inputs
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
            .select_declaring(root, &self.members, &storage.filesystem, excluded)
    }
}
