//! Source assembly is one control-flow graph. Its instruction statements use
//! ordinary source operations; labels retain the edge contracts until typed
//! elaboration can carry the enclosing values through those edges.

use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::statement::StatementHandle;
use arena::{Handle, HandleSpan, PagedArenaSet as Arena};

#[derive(Debug, Clone, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
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

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub struct AssemblyBlock {
    pub label: Identifier,
    pub requires: HandleSpan<ExpressionHandle>,
    pub ensures: HandleSpan<ExpressionHandle>,
    pub instructions: HandleSpan<StatementHandle>,
    pub exit: AssemblyExit,
}

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
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

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub(crate) struct AssemblyTable {
    graphs: Arena<AssemblyControlFlow>,
    blocks: Arena<AssemblyBlock>,
    instructions: Arena<StatementHandle>,
    expressions: Arena<ExpressionHandle>,
}

pub(crate) struct AssemblyTableAppendCheckpoint {
    graphs: arena::PagedArenaFrontier<AssemblyControlFlow>,
    blocks: arena::PagedArenaFrontier<AssemblyBlock>,
    instructions: arena::PagedArenaFrontier<StatementHandle>,
    expressions: arena::PagedArenaFrontier<ExpressionHandle>,
}

impl AssemblyTable {
    pub(crate) fn append_checkpoint(&self) -> AssemblyTableAppendCheckpoint {
        AssemblyTableAppendCheckpoint {
            graphs: self.graphs.frontier(),
            blocks: self.blocks.frontier(),
            instructions: self.instructions.frontier(),
            expressions: self.expressions.frontier(),
        }
    }

    pub(crate) fn rewind_appended(&mut self, checkpoint: AssemblyTableAppendCheckpoint) {
        self.graphs.rewind_appended(checkpoint.graphs);
        self.blocks.rewind_appended(checkpoint.blocks);
        self.instructions.rewind_appended(checkpoint.instructions);
        self.expressions.rewind_appended(checkpoint.expressions);
    }

    pub(crate) fn share_pages(&mut self) {
        self.graphs.share_pages();
        self.blocks.share_pages();
        self.instructions.share_pages();
        self.expressions.share_pages();
    }

    pub(crate) fn new(generation: u32) -> Self {
        Self {
            graphs: Arena::with_generation(generation),
            blocks: Arena::with_generation(generation),
            instructions: Arena::with_generation(generation),
            expressions: Arena::with_generation(generation),
        }
    }

    pub(crate) fn adopt(&mut self, other: Self) {
        self.graphs.adopt(other.graphs);
        self.blocks.adopt(other.blocks);
        self.instructions.adopt(other.instructions);
        self.expressions.adopt(other.expressions);
    }

    pub(crate) fn begin_page(&mut self, generation: u32) {
        self.graphs.begin_page(generation);
        self.blocks.begin_page(generation);
        self.instructions.begin_page(generation);
        self.expressions.begin_page(generation);
    }

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
