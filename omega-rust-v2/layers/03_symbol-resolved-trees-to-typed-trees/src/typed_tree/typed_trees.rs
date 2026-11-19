//! L3 carries L2's immutable tree storage forward. A structural node may have
//! multiple typed records. Index their exact producing nodes once so consumers
//! can walk branches in source order without reconstructing a tree from spans.
use crate::{IndirectCall, TypedCall, TypedExpression, TypedOperation, TypedStage};
use omega_resolver::{SymbolResolvedTree, SymbolResolvedTrees, TreeNodeId};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedTrees<'text> {
    resolved: SymbolResolvedTrees<'text>,
    facts: TypedStage,
    by_node: Vec<(TreeNodeId, Record)>,
}

#[derive(Debug, Eq, PartialEq)]
enum Record {
    Expression(usize),
    Call(usize),
    IndirectCall(usize),
    Operation(usize),
}

pub enum NodeSemantics<'a> {
    Expression(&'a TypedExpression),
    Call(&'a TypedCall),
    IndirectCall(&'a IndirectCall),
    Operation(&'a TypedOperation),
}

impl<'text> TypedTrees<'text> {
    pub(crate) fn new(
        resolved: SymbolResolvedTrees<'text>,
        facts: TypedStage,
    ) -> Self {
        let mut by_node = Vec::with_capacity(facts.expressions.len() + facts.calls.len() + facts.indirect_calls.len() + facts.operations.len());
        by_node.extend(
            facts
                .expressions
                .iter()
                .enumerate()
                .map(|(i, e)| (e.node, Record::Expression(i))),
        );
        by_node.extend(
            facts
                .calls
                .iter()
                .enumerate()
                .map(|(i, c)| (c.node, Record::Call(i))),
        );
        by_node.extend(
            facts
                .indirect_calls
                .iter()
                .enumerate()
                .map(|(i, c)| (c.node, Record::IndirectCall(i))),
        );
        by_node.extend(
            facts
                .operations
                .iter()
                .enumerate()
                .map(|(i, op)| (op.node, Record::Operation(i))),
        );
        by_node.sort_by_key(|(node, _)| *node);
        Self { resolved, facts, by_node }
    }

    pub fn resolved(&self) -> &SymbolResolvedTrees<'text> {
        &self.resolved
    }

    pub fn trees(&self) -> impl ExactSizeIterator<Item = TypedTree<'_>> {
        self.resolved
            .trees()
            .map(|resolved| TypedTree { resolved, forest: self })
    }

    fn semantics_at(
        &self,
        node: TreeNodeId,
    ) -> impl Iterator<Item = NodeSemantics<'_>> {
        let start = self.by_node.partition_point(|(site, _)| *site < node);
        self.by_node[start..]
            .iter()
            .take_while(move |(site, _)| *site == node)
            .map(|(_, record)| match *record {
                Record::Expression(i) => NodeSemantics::Expression(&self.facts.expressions[i]),
                Record::Call(i) => NodeSemantics::Call(&self.facts.calls[i]),
                Record::IndirectCall(i) => NodeSemantics::IndirectCall(&self.facts.indirect_calls[i]),
                Record::Operation(i) => NodeSemantics::Operation(&self.facts.operations[i]),
            })
    }

    pub fn into_facts(self) -> TypedStage {
        self.facts
    }
}

impl std::ops::Deref for TypedTrees<'_> {
    type Target = TypedStage;
    fn deref(&self) -> &Self::Target {
        &self.facts
    }
}

/// Node indices are local to the tree being walked. The forest supplies the
/// file identity, so equal node numbers in two files select different records.
pub struct TypedTree<'a> {
    resolved: SymbolResolvedTree<'a>,
    forest: &'a TypedTrees<'a>,
}
impl<'a> TypedTree<'a> {
    pub fn resolved(&self) -> &SymbolResolvedTree<'a> {
        &self.resolved
    }

    pub fn semantics_at(
        &self,
        node: usize,
    ) -> impl Iterator<Item = NodeSemantics<'_>> {
        self.forest.semantics_at(self.resolved.file.node_id(node))
    }
}
