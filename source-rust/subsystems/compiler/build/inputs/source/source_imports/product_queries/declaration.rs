//! Open the Product dependency named by a declaration query at its issuing occurrence.

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
}

impl ProductQueryInputs {
    pub fn with_declaration_path(mut self, author: &SourceFile, span: Span, path: &[u8]) -> Self {
        // A file-qualified operand can be resolved entirely from its actual import.
        // An unqualified operand can also name a declaration in the successor entry.
        if let Ok(name) = std::str::from_utf8(path) {
            let alias = name
                .strip_prefix('|')
                .and_then(|qualified| qualified.split_once("|::"))
                .map(|(alias, _)| alias.to_owned());
            self.declarations.push(DeclarationLookup {
                author: author.clone(),
                span,
                alias,
            });
        }
        self
    }

    /// File aliases that an executed Product query selected, in its issuing source.
    pub fn queried_file_aliases(&self) -> impl Iterator<Item = (&SourceFile, &str)> {
        self.declarations.iter().filter_map(|lookup| {
            let alias = lookup.alias.as_deref()?;
            Some((&lookup.author, alias))
        })
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
        if let Some(alias) = &self.alias {
            return super::super::files::product_file_sources(
                storage,
                SourceSpan::new(author, self.span),
                alias,
                inputs.graphs().chain(inputs.named_source.iter()),
            );
        }
        Ok(inputs
            .successor_entry()
            .map(|entry| storage.default_package_root().join(entry.as_str()))
            .into_iter()
            .collect())
    }
}
