//! Source assembly is one control-flow graph. Its instruction statements use
//! ordinary source operations; labels retain the edge contracts until typed
//! elaboration can carry the enclosing values through those edges.

use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::statement::StatementHandle;
use arena::{Handle, HandleSpan, PagedArenaSet as Arena};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssemblyControlFlow {
    pub blocks: HandleSpan<AssemblyBlock>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssemblyBlock {
    pub label: Identifier,
    pub requires: HandleSpan<ExpressionHandle>,
    pub ensures: HandleSpan<ExpressionHandle>,
    pub instructions: HandleSpan<StatementHandle>,
    pub exit: AssemblyExit,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AssemblyExit {
    #[default]
    Fallthrough,
    Jump(Identifier),
    /// An ordinary checked instruction transfers out of this assembly body.
    /// It is retained as the block's last instruction, never a label edge.
    EnclosingTransition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssemblyTable {
    graphs: Arena<AssemblyControlFlow>,
    blocks: Arena<AssemblyBlock>,
    instructions: Arena<StatementHandle>,
    expressions: Arena<ExpressionHandle>,
}

impl AssemblyTable {
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
