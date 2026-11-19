//! L2 identifies declarations and uses by source span. This file's tables connect those identities to its indexed syntax while L3 checks it.

use crate::{Access, Site, TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::{DeclarationId, IndexedSource, Symbol};
use std::collections::HashMap;

pub(crate) struct ResolvedSourceFile<'a> {
    pub(crate) file: IndexedSource<'a>,
    pub(crate) names: HashMap<SourceSpan, &'a Symbol>,
    pub(crate) parameters: HashMap<SourceSpan, ValueType>,
    pub(crate) hypotheses: HashMap<SourceSpan, Site>,
    pub(crate) binding_annotations: HashMap<SourceSpan, usize>,
    pub(crate) definitions: HashMap<SourceSpan, &'a DeclarationId>,
}
impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn declaration(
        &self,
        node_index: usize,
    ) -> Result<&'a DeclarationId, TypeError> {
        self.definitions
            .get(&self.file.span(node_index))
            .copied()
            .ok_or_else(|| TypeError::at(&self.file, node_index, "missing L2 definition"))
    }
    pub(crate) fn single_child(
        &self,
        node_index: usize,
    ) -> Result<usize, TypeError> {
        match self.file.nodes[node_index].children.as_slice() {
            [child] => Ok(*child),
            _ => Err(TypeError::unsupported(&self.file, node_index)),
        }
    }
}

impl<'a> ResolvedSourceFile<'a> {
    pub fn symbol(
        &self,
        node_index: usize,
    ) -> Option<&'a Symbol> {
        let mut words = self.file.nodes[node_index].words();
        let first = words.next()?;
        let last = words.next_back().unwrap_or(first);
        self.names
            .get(&SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()))
            .copied()
    }
    pub fn qualifier(
        &self,
        node: usize,
    ) -> Option<&'a Symbol> {
        let mut words = self.file.nodes[node].words();
        words.next_back()?;
        let first = words.next()?;
        let last = words.next_back().unwrap_or(first);
        self.names
            .get(&SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()))
            .copied()
    }
    pub fn access(
        &self,
        node_index: usize,
    ) -> Access {
        match self.file.nodes[node_index].words().next().map(|w| w.text) {
            Some("mut") => Access::Mut,
            Some("write") => Access::Write,
            _ => Access::Shared,
        }
    }
}
