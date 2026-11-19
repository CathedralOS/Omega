//! L3 borrows L2's resolved tree and keeps parameter types and logical hypotheses for this file.

use crate::{Access, Site, TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::{DeclarationId, SymbolResolvedTree};
use std::collections::HashMap;

pub(crate) struct ResolvedSourceFile<'a> {
    pub(crate) tree: SymbolResolvedTree<'a>,
    pub(crate) parameters: HashMap<SourceSpan, ValueType>,
    pub(crate) hypotheses: HashMap<SourceSpan, Site>,
}
impl<'a> std::ops::Deref for ResolvedSourceFile<'a> {
    type Target = SymbolResolvedTree<'a>;
    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn declaration(
        &self,
        node_index: usize,
    ) -> Result<&'a DeclarationId, TypeError> {
        self.declaration_at(node_index)
            .ok_or_else(|| TypeError::at(self.file, node_index, "missing L2 definition"))
    }
    pub(crate) fn single_child(
        &self,
        node_index: usize,
    ) -> Result<usize, TypeError> {
        match self.file.nodes[node_index].children.as_slice() {
            [child] => Ok(*child),
            _ => Err(TypeError::unsupported(self.file, node_index)),
        }
    }
}

impl<'a> ResolvedSourceFile<'a> {
    pub(crate) fn parameter_annotation(
        &self,
        binder_node: usize,
    ) -> Option<usize> {
        let binder = &self.file.nodes[binder_node];
        if binder.kind != omega_parser::NodeKind::Param {
            return None;
        }
        binder.children.first().copied()
    }
    pub(crate) fn operator_token(
        &self,
        node: usize,
    ) -> Result<&'a str, TypeError> {
        self.operator(node)
            .map(|op| op.token.as_str())
            .ok_or_else(|| TypeError::at(self.file, node, "missing resolved operator"))
    }
    pub(crate) fn member_name(
        &self,
        node: usize,
    ) -> Result<&'a str, TypeError> {
        self.member(node)
            .ok_or_else(|| TypeError::at(self.file, node, "missing resolved member"))
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
