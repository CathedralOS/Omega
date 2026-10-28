//! Source assembly is one control-flow graph. Its instruction statements use
//! ordinary source operations; labels retain the edge contracts until typed
//! elaboration can carry the enclosing values through those edges.

use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::statement::StatementHandle;
use crate::syntax_trees::paged_roster::paged_roster;
use arena::{Handle, HandleSpan};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
pub struct AssemblyControlFlow {
    /// The intersection of instruction and register-view catalog demands.
    pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
    /// Catalog-generated arithmetic and ZF operations: their selected meaning
    /// must remain builtin, independently of authored token overloads.
    pub primitives: HandleSpan<crate::expression::ExpressionHandle>,
    /// Explicit body-local register/flag identities; empty cells are absent.
    pub scratch: [crate::expression::ExpressionHandle;
        language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
    pub blocks: HandleSpan<AssemblyBlock>,
}
impl Default for AssemblyControlFlow {
    fn default() -> Self {
        Self {
            target_requirement: Default::default(),
            primitives: Default::default(),
            scratch: [crate::expression::ExpressionHandle::invalid();
                language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
            blocks: Default::default(),
        }
    }
}

#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
pub struct AssemblyBlock {
    pub label: Identifier,
    pub requires: HandleSpan<ExpressionHandle>,
    pub ensures: HandleSpan<ExpressionHandle>,
    pub instructions: HandleSpan<StatementHandle>,
    pub exit: AssemblyExit,
}

#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
pub enum AssemblyExit {
    #[default]
    Fallthrough,
    Jump(Identifier),
    /// Local destination and required ZF polarity (true for set, false for clear).
    ZeroFlag(Identifier, bool),
    /// An ordinary checked instruction transfers out of this assembly body.
    /// It is retained as the block's last instruction, never a label edge.
    EnclosingTransition,
}

paged_roster! {
    pub(crate) struct AssemblyTable => pub(crate) AssemblyTableAppendCheckpoint {
        arena graphs: AssemblyControlFlow,
        arena blocks: AssemblyBlock,
        arena instructions: StatementHandle,
        arena expressions: ExpressionHandle,
    }
}

impl AssemblyTable {
    pub(crate) fn insert_graph(
        &mut self,
        graph: AssemblyControlFlow,
    ) -> Handle<AssemblyControlFlow> {
        self.graphs.insert(graph)
    }

    pub(crate) fn graph(&self, handle: Handle<AssemblyControlFlow>) -> &AssemblyControlFlow {
        self.graphs.get(handle)
    }

    pub(crate) fn insert_blocks(
        &mut self,
        blocks: impl IntoIterator<Item = AssemblyBlock>,
    ) -> HandleSpan<AssemblyBlock> {
        self.blocks.insert_many(blocks)
    }

    pub(crate) fn blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
        self.blocks.span_or_empty(span)
    }

    pub(crate) fn insert_instructions(
        &mut self,
        instructions: impl IntoIterator<Item = StatementHandle>,
    ) -> HandleSpan<StatementHandle> {
        self.instructions.insert_many(instructions)
    }

    pub(crate) fn instructions(&self, span: HandleSpan<StatementHandle>) -> &[StatementHandle] {
        self.instructions.span_or_empty(span)
    }

    pub(crate) fn insert_expressions(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.expressions.insert_many(expressions)
    }

    pub(crate) fn expressions(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.expressions.span_or_empty(span)
    }
}
