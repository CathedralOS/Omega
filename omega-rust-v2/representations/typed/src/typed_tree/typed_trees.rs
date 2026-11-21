//! L3 carries L2's immutable tree storage forward. A structural node may have
//! multiple typed records. Index their exact producing nodes once so consumers
//! can walk branches in source order without reconstructing a tree from spans.
use crate::{IndirectCall, TypedCall, TypedExpression, TypedOperation, TypedStage};
use omega_resolved::{SymbolResolvedTree, SymbolResolvedTrees, TreeNodeId};

#[derive(Debug, Eq, PartialEq)]
pub struct TypedTrees<'text> {
    resolved: SymbolResolvedTrees<'text>,
    facts: TypedStage,
    by_node: Vec<(TreeNodeId, Record)>,
}

#[derive(Debug, Eq, PartialEq)]
enum Record {
    Membership(usize),
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
    Constant(usize),
}

#[derive(Clone, Copy)]
pub enum NodeSemantics<'a> {
    Membership(&'a crate::TypedMembership),
    Field(&'a omega_resolved::DeclarationId),
    Constant(&'a omega_resolved::DeclarationId),
    CollectionAccess(&'a crate::CollectionAccess),
    Expression(&'a TypedExpression),
    Landing(&'a crate::TypedNumericLanding),
    Call(&'a TypedCall),
    IndirectCall(&'a IndirectCall),
    Operation(&'a TypedOperation),
    Instruction(&'a crate::TypedInstruction),
    RankingWitness(&'a crate::RankingWitness),
    Obligation(&'a crate::TypingObligation),
    RequirementBinding(&'a crate::ConformanceRow),
}

impl<'text> TypedTrees<'text> {
    pub fn new(
        resolved: SymbolResolvedTrees<'text>,
        mut facts: TypedStage,
    ) -> Self {
        let mut by_node = Vec::with_capacity(
            facts.expressions.len()
                + facts.memberships.len()
                + facts.landings.len()
                + facts.constant_references.len()
                + facts.calls.len()
                + facts.indirect_calls.len()
                + facts.operations.len()
                + facts.instructions.len()
                + facts.ranking_witnesses.len()
                + facts.obligations.len()
                + facts.conformance_rows.len()
                + facts.collection_accesses.len(),
        );
        fn indexed(
            nodes: impl Iterator<Item = TreeNodeId>,
            record: fn(usize) -> Record,
        ) -> impl Iterator<Item = (TreeNodeId, Record)> {
            nodes.enumerate().map(move |(i, node)| (node, record(i)))
        }
        by_node.extend(indexed(facts.memberships.iter().map(|m| m.node), Record::Membership));
        by_node.extend(indexed(facts.expressions.iter().map(|e| e.node), Record::Expression));
        by_node.extend(indexed(facts.constant_references.iter().map(|(node, _)| *node), Record::Constant));
        by_node.extend(indexed(facts.landings.iter().map(|e| e.node), Record::Landing));
        by_node.extend(indexed(facts.calls.iter().map(|c| c.node), Record::Call));
        by_node.extend(indexed(facts.indirect_calls.iter().map(|c| c.node), Record::IndirectCall));
        by_node.extend(indexed(facts.operations.iter().map(|op| op.node), Record::Operation));
        by_node.extend(indexed(facts.conformance_rows.iter().map(|row| row.node), Record::RequirementBinding));
        by_node.extend(indexed(facts.collection_accesses.iter().map(|access| access.node), Record::CollectionAccess));
        by_node.extend(indexed(facts.instructions.iter().map(|i| i.node), Record::Instruction));
        by_node.extend(indexed(facts.ranking_witnesses.iter().map(|w| w.node), Record::RankingWitness));
        // Every obligation is this forest's own; an admitted library keeps its rows in its own forest.
        by_node.extend(indexed(facts.obligations.iter().map(|o| o.node), Record::Obligation));
        by_node.sort_by_key(|(node, _)| *node);
        facts.callable_origins.sort_unstable();
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

    /// Resolve an ordinary imported call to the body belonging to this typing round.
    /// Import shares the defining signature with the caller. A second library can have
    /// the same declaration, signature and source coordinates with a different body.
    /// Its own clients can use it, but it cannot supply the body selected by this client.
    pub fn imported_machine_definition(
        &self,
        machine: &omega_resolved::DeclarationId,
    ) -> Option<(TypedTree<'_>, &crate::TypedSignature)> {
        let library = self.library?;
        let selected = self
            .signatures
            .iter()
            .find(|signature| &signature.machine == machine)?;
        let definition = library
            .signatures
            .iter()
            .find(|definition| std::sync::Arc::ptr_eq(selected, definition))?;
        let tree = library.trees().nth(definition.node.file)?;
        let resolved = tree.resolved();
        if resolved.declaration_at(definition.node.node) != Some(machine) {
            return None;
        }
        resolved
            .file
            .child(definition.node.node, omega_syntax::NodeKind::Body)?;
        Some((tree, definition))
    }

    /// Find the alias RHS checked for this contract in its defining forest.
    pub fn callable_origin(
        &self,
        contract: &crate::Site,
    ) -> Option<(TypedTree<'_>, usize)> {
        for forest in std::iter::once(self).chain(self.library) {
            if let Ok(index) = forest
                .facts
                .callable_origins
                .binary_search_by(|(site, _)| site.cmp(contract))
            {
                let node = forest.facts.callable_origins[index].1;
                return Some((forest.trees().nth(node.file)?, node.node));
            }
        }
        None
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
                Record::Membership(i) => NodeSemantics::Membership(&self.facts.memberships[i]),
                Record::RequirementBinding(i) => NodeSemantics::RequirementBinding(&self.facts.conformance_rows[i]),
                Record::CollectionAccess(i) => NodeSemantics::CollectionAccess(&self.facts.collection_accesses[i]),
                Record::Landing(i) => NodeSemantics::Landing(&self.facts.landings[i]),
                Record::Constant(i) => NodeSemantics::Constant(&self.facts.constant_references[i].1),
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
    /// Declaration facts and node identities belong to this defining forest.
    pub fn forest(&self) -> &'a TypedTrees<'a> {
        self.forest
    }

    pub fn resolved(&self) -> &SymbolResolvedTree<'a> {
        &self.resolved
    }

    pub fn semantics_at(
        &self,
        node: usize,
    ) -> impl Iterator<Item = NodeSemantics<'_>> {
        self.forest.semantics_at(self.resolved.file.node_id(node))
    }

    pub fn declaration_selection(&self, node: usize) -> Option<&crate::DeclarationSelection> {
        self.forest.declaration_selections.get(&self.resolved.file.node_id(node))
    }

    pub fn construction(
        &self,
        node: usize,
    ) -> Option<&crate::TypedConstruction> {
        self.forest.constructions.get(&self.resolved.file.node_id(node))
    }

    pub fn assembly_region(
        &self,
        node: usize,
    ) -> Option<&crate::TypedAssemblyRegion> {
        self.forest
            .assembly_regions
            .get(&self.resolved.file.node_id(node))
    }

    pub fn owned_byte_literal(
        &self,
        node: usize,
    ) -> Option<&[u8]> {
        self.forest
            .owned_byte_literals
            .get(&self.resolved.file.node_id(node))
            .map(AsRef::as_ref)
    }
}
