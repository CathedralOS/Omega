//! L2 retains its indexed syntax and binds each file's occurrences to its
//! declarations or candidates. Only text and file identities borrow from the caller.
use crate::{DeclarationId, IndexedSource, NameRole, NameUse, OperatorUse, ResolvedStage, Symbol};
use omega_lexer::SourceSpan;
use std::{ops::Deref, sync::Arc};

#[derive(Debug, Eq, PartialEq)]
struct BoundFile<'text> {
    syntax: IndexedSource<'text>,
    names: Vec<usize>,
    definitions: Vec<(usize, usize)>,
    applications: Vec<usize>,
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
        definitions: Vec<Vec<(usize, usize)>>,
        resolution: ResolvedStage,
    ) -> Self {
        let mut files: Vec<_> = files
            .into_iter()
            .zip(definitions)
            .map(|(syntax, mut definitions)| {
                definitions.sort_unstable_by_key(|&(node, _)| node);
                BoundFile {
                    syntax,
                    definitions,
                    names: Vec::new(),
                    applications: Vec::new(),
                }
            })
            .collect();
        for (index, name) in resolution.uses.iter().enumerate() {
            let file = &mut files[name.node.file];
            file.names.push(index);
        }
        for (index, application) in resolution.applications.iter().enumerate() {
            files[application.section.file].applications.push(index);
        }
        for file in &mut files {
            file.names.sort_by_key(|&i| resolution.uses[i].node.node);
            file.applications
                .sort_unstable_by_key(|&i| resolution.applications[i].section.node);
        }
        Self(Arc::new(Forest { files, resolution }))
    }

    pub fn trees(&self) -> impl ExactSizeIterator<Item = SymbolResolvedTree<'_>> {
        self.0.files.iter().map(|file| SymbolResolvedTree {
            file: &file.syntax,
            bindings: file,
            resolution: &self.0.resolution,
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
    bindings: &'a BoundFile<'a>,
    resolution: &'a ResolvedStage,
}

impl<'a> SymbolResolvedTree<'a> {
    /// A field shorthand has both a member occurrence and a value reference.
    pub fn names(
        &self,
        node: usize,
    ) -> impl Iterator<Item = &'a NameUse> {
        let uses = &self.resolution.uses;
        let indices = &self.bindings.names;
        let start = indices.partition_point(|&i| uses[i].node.node < node);
        indices[start..]
            .iter()
            .map(|&i| &uses[i])
            .take_while(move |name| name.node.node == node)
    }
    pub fn symbol(
        &self,
        node: usize,
    ) -> Option<&'a Symbol> {
        self.names(node)
            .find(|name| name.role == NameRole::Reference)
            .map(|name| &name.target)
    }
    pub fn qualifier(
        &self,
        node: usize,
    ) -> Option<&'a Symbol> {
        self.names(node)
            .filter(|name| name.role == NameRole::Qualifier)
            .last()
            .map(|name| &name.target)
    }
    pub fn member(
        &self,
        node: usize,
    ) -> Option<&'a str> {
        self.names(node).find_map(|name| match &name.target {
            Symbol::Member(member) => Some(member.as_str()),
            _ => None,
        })
    }
    pub fn declaration_at(
        &self,
        node: usize,
    ) -> Option<&'a DeclarationId> {
        let defs = &self.bindings.definitions;
        let i = defs.binary_search_by_key(&node, |&(n, _)| n).ok()?;
        Some(&self.resolution.declarations[defs[i].1])
    }
    pub fn definitions(&self) -> impl Iterator<Item = (SourceSpan, &'a DeclarationId)> + '_ {
        self.bindings
            .definitions
            .iter()
            .map(|&(node, i)| (self.file.span(node), &self.resolution.declarations[i]))
    }
    pub fn operator(
        &self,
        node: usize,
    ) -> Option<&'a OperatorUse> {
        // The resolver emits operators in sorted file order and tree preorder.
        let id = self.file.node_id(node);
        let ops = &self.resolution.operators;
        ops.binary_search_by_key(&id, |op| op.node)
            .ok()
            .map(|i| &ops[i])
    }
    pub fn application_name(
        &self,
        section: usize,
    ) -> Option<&'a NameUse> {
        let indices = &self.bindings.applications;
        let apps = &self.resolution.applications;
        let i = indices
            .binary_search_by_key(&section, |&i| apps[i].section.node)
            .ok()?;
        Some(&self.resolution.uses[apps[indices[i]].name_use])
    }
}
