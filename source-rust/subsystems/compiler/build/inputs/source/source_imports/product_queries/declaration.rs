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
    alias: Option<String>,
    members: Vec<String>,
}

impl ProductQueryInputs {
    pub fn with_declaration_path(mut self, author: &SourceFile, span: Span, path: &[u8]) -> Self {
        // A file-qualified operand can be resolved entirely from its actual import.
        // An unqualified operand can also name a declaration in the successor entry.
        if let Ok(name) = std::str::from_utf8(path) {
            let (alias, members) = match name.split_once("::") {
                Some((alias, members)) => (Some(alias.to_owned()), members),
                None => (None, name),
            };
            self.declarations.push(DeclarationLookup {
                author: author.clone(),
                span,
                alias,
                members: members.split("::").map(str::to_owned).collect(),
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
        let local_entry = || {
            inputs
                .successor_entry()
                .map(|entry| storage.default_package_root().join(entry.as_str()))
                .into_iter()
                .collect()
        };
        let Some(qualifier) = &self.alias else {
            return Ok(local_entry());
        };
        if let Some(alias) = qualifier
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
            .product_dependency_at(SourceSpan::new(author, self.span), qualifier)
        else {
            return Ok(local_entry());
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
