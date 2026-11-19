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
    Landing(usize),
    Call(usize),
    IndirectCall(usize),
    Operation(usize),
    Instruction(usize),
    RankingWitness(usize),
    Obligation(usize),
    RequirementBinding(usize),
    CollectionAccess(usize),
}

pub enum NodeSemantics<'a> {
    Field(&'a omega_resolver::DeclarationId),
    CollectionAccess(&'a crate::CollectionAccess),
    Expression(&'a TypedExpression),
    /// The destination carrier of an exact expression at this node.
    Landing(&'a TypedExpression),
    Call(&'a TypedCall),
    IndirectCall(&'a IndirectCall),
    Operation(&'a TypedOperation),
    Instruction(&'a crate::TypedInstruction),
    RankingWitness(&'a crate::RankingWitness),
    Obligation(&'a crate::TypingObligation),
    RequirementBinding(&'a crate::ConformanceRow),
}

impl<'text> TypedTrees<'text> {
    pub(crate) fn new(
        resolved: SymbolResolvedTrees<'text>,
        facts: TypedStage,
        first_local_obligation: usize,
    ) -> Self {
        let mut by_node = Vec::with_capacity(
            facts.expressions.len()
                + facts.landings.len()
                + facts.calls.len()
                + facts.indirect_calls.len()
                + facts.operations.len()
                + facts.instructions.len()
                + facts.ranking_witnesses.len()
                + (facts.obligations.len() - first_local_obligation)
                + facts.conformance_rows.len()
                + facts.collection_accesses.len(),
        );
        fn indexed(
            nodes: impl Iterator<Item = TreeNodeId>,
            record: fn(usize) -> Record,
        ) -> impl Iterator<Item = (TreeNodeId, Record)> {
            nodes.enumerate().map(move |(i, node)| (node, record(i)))
        }
        by_node.extend(indexed(facts.expressions.iter().map(|e| e.node), Record::Expression));
        by_node.extend(indexed(facts.landings.iter().map(|e| e.node), Record::Landing));
        by_node.extend(indexed(facts.calls.iter().map(|c| c.node), Record::Call));
        by_node.extend(indexed(facts.indirect_calls.iter().map(|c| c.node), Record::IndirectCall));
        by_node.extend(indexed(facts.operations.iter().map(|op| op.node), Record::Operation));
        by_node.extend(indexed(facts.conformance_rows.iter().map(|row| row.node), Record::RequirementBinding));
        by_node.extend(indexed(facts.collection_accesses.iter().map(|access| access.node), Record::CollectionAccess));
        by_node.extend(indexed(facts.instructions.iter().map(|i| i.node), Record::Instruction));
        by_node.extend(indexed(facts.ranking_witnesses.iter().map(|w| w.node), Record::RankingWitness));
        // Imported obligations are already indexed in the defining library's forest.
        by_node.extend(
            facts
                .obligations
                .iter()
                .enumerate()
                .skip(first_local_obligation)
                .map(|(i, obligation)| (obligation.node, Record::Obligation(i))),
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
                Record::RequirementBinding(i) => NodeSemantics::RequirementBinding(&self.facts.conformance_rows[i]),
                Record::CollectionAccess(i) => NodeSemantics::CollectionAccess(&self.facts.collection_accesses[i]),
                Record::Landing(i) => NodeSemantics::Landing(&self.facts.landings[i]),
                Record::Expression(i) => NodeSemantics::Expression(&self.facts.expressions[i]),
                Record::Call(i) => NodeSemantics::Call(&self.facts.calls[i]),
                Record::IndirectCall(i) => NodeSemantics::IndirectCall(&self.facts.indirect_calls[i]),
                Record::Instruction(i) => NodeSemantics::Instruction(&self.facts.instructions[i]),
                Record::RankingWitness(i) => NodeSemantics::RankingWitness(&self.facts.ranking_witnesses[i]),
                Record::Obligation(i) => NodeSemantics::Obligation(&self.facts.obligations[i]),
                Record::Operation(i) => NodeSemantics::Operation(&self.facts.operations[i]),
            })
            .chain(
                self.facts
                    .place_steps
                    .get(&node)
                    .and_then(|step| match &step.projection {
                        Some(crate::Projection::Field(id)) => Some(NodeSemantics::Field(id)),
                        _ => None,
                    }),
            )
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
