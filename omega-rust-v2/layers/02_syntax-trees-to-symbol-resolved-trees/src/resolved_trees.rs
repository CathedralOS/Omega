//! L2 retains its indexed syntax and binds each file's occurrences to its
//! declarations or candidates. Only text and file identities borrow from the caller.
use crate::{DeclarationId, IndexedSource, NameUse, ResolvedStage, Symbol};
use omega_lexer::SourceSpan;
use std::{collections::HashMap, ops::Deref, sync::Arc};

#[derive(Debug, Eq, PartialEq)]
struct BoundFile<'text> {
    syntax: IndexedSource<'text>,
    names: HashMap<SourceSpan, usize>,
    definitions: HashMap<SourceSpan, usize>,
}

#[derive(Debug, Eq, PartialEq)]
struct Forest<'text> {
    files: Vec<BoundFile<'text>>,
    resolution: ResolvedStage,
}

/// A clone shares the immutable trees when typing under another environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolResolvedTrees<'text>(Arc<Forest<'text>>);

impl<'text> SymbolResolvedTrees<'text> {
    pub(crate) fn new(
        files: Vec<IndexedSource<'text>>,
        names: Vec<HashMap<SourceSpan, usize>>,
        definitions: Vec<HashMap<SourceSpan, usize>>,
        resolution: ResolvedStage,
    ) -> Self {
        let files = files
            .into_iter()
            .zip(names)
            .zip(definitions)
            .map(|((syntax, names), definitions)| BoundFile { syntax, names, definitions })
            .collect();
        Self(Arc::new(Forest { files, resolution }))
    }

    pub fn trees(&self) -> impl ExactSizeIterator<Item = SymbolResolvedTree<'_>> {
        self.0.files.iter().map(|file| SymbolResolvedTree {
            file: &file.syntax,
            names: &file.names,
            definitions: &file.definitions,
            uses: &self.uses,
            declarations: &self.declarations,
        })
    }
}

impl Deref for SymbolResolvedTrees<'_> {
    type Target = ResolvedStage;
    fn deref(&self) -> &Self::Target {
        &self.0.resolution
    }
}

/// Each lookup resolves an occurrence within this tree's file.
#[derive(Clone, Copy)]
pub struct SymbolResolvedTree<'a> {
    pub file: &'a IndexedSource<'a>,
    names: &'a HashMap<SourceSpan, usize>,
    definitions: &'a HashMap<SourceSpan, usize>,
    uses: &'a [NameUse],
    declarations: &'a [DeclarationId],
}

impl<'a> SymbolResolvedTree<'a> {
    pub fn symbol_at(
        &self,
        span: SourceSpan,
    ) -> Option<&'a Symbol> {
        self.names.get(&span).map(|&i| &self.uses[i].target)
    }
    pub fn declaration_at(
        &self,
        span: SourceSpan,
    ) -> Option<&'a DeclarationId> {
        self.definitions.get(&span).map(|&i| &self.declarations[i])
    }
    pub fn definitions(&self) -> impl Iterator<Item = (SourceSpan, &'a DeclarationId)> + '_ {
        self.definitions
            .iter()
            .map(|(&span, &i)| (span, &self.declarations[i]))
    }
}
