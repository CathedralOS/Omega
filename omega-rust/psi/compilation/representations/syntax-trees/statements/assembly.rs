use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::paged_roster::paged_roster;
use crate::statement::StatementHandle;
use arena::{Handle, HandleSpan};

// The parser preserves instruction spelling, operand expressions and labeled blocks.
#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct AuthoredAssembly {
    pub source_span: source_text::SourceSpan,
    pub requires: Box<[ExpressionHandle]>,
    pub ensures: Box<[ExpressionHandle]>,
    pub clobbers: Option<Box<[Identifier]>>,
    pub blocks: Box<[AuthoredAssemblyBlock]>,
}

impl AuthoredAssembly {
    pub fn visit_operands(
        &self,
        trees: &crate::SyntaxTrees,
        mut visit: impl FnMut(ExpressionHandle),
    ) {
        for &operand in self.requires.iter().chain(&self.ensures) {
            visit(operand);
        }
        for block in &self.blocks {
            for &operand in block.requires.iter().chain(&block.ensures) {
                visit(operand);
            }
            for instruction in &block.instructions {
                match &instruction.operands {
                    AssemblyOperands::Expressions(values) => {
                        for &operand in values {
                            visit(operand);
                        }
                    }
                    AssemblyOperands::Transition(target) => {
                        use crate::statement::TransitionTargetNode;
                        match trees.statements.transition_target(*target) {
                            TransitionTargetNode::Value(value) => visit(*value),
                            TransitionTargetNode::Named { arguments, .. } => {
                                for &operand in trees.statements.expression_handles(*arguments) {
                                    visit(operand);
                                }
                            }
                            TransitionTargetNode::Terminal => {}
                        }
                    }
                }
            }
        }
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct AuthoredAssemblyBlock {
    pub label: Option<Identifier>,
    pub requires: Box<[ExpressionHandle]>,
    pub ensures: Box<[ExpressionHandle]>,
    pub instructions: Box<[AssemblyInstruction]>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct AssemblyInstruction {
    pub mnemonic: Identifier,
    pub operands: AssemblyOperands,
    pub source_span: source_text::SourceSpan,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum AssemblyOperands {
    Expressions(Box<[ExpressionHandle]>),
    Transition(crate::statement::TransitionTargetHandle),
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct AssemblyControlFlow {
    pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
    pub primitives: HandleSpan<ExpressionHandle>,
    pub scratch: [ExpressionHandle; language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
    pub blocks: HandleSpan<AssemblyBlock>,
}
impl Default for AssemblyControlFlow {
    fn default() -> Self {
        Self {
            target_requirement: Default::default(),
            primitives: Default::default(),
            scratch: [ExpressionHandle::invalid();
                language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
            blocks: Default::default(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct AssemblyBlock {
    pub label: Identifier,
    pub requires: HandleSpan<ExpressionHandle>,
    pub ensures: HandleSpan<ExpressionHandle>,
    pub instructions: HandleSpan<StatementHandle>,
    pub exit: AssemblyExit,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum AssemblyExit {
    #[default]
    Fallthrough,
    Jump(Identifier),
    ZeroFlag(Identifier, bool),
    EnclosingTransition,
}

paged_roster! {
    pub(crate) struct AssemblyTable => pub(crate) AssemblyTableAppendCheckpoint {
        arena authored: AuthoredAssembly,
        arena graphs: AssemblyControlFlow,
        arena blocks: AssemblyBlock,
        arena instructions: StatementHandle,
        arena expressions: ExpressionHandle,
    }
}

impl AssemblyTable {
    pub(crate) fn insert_authored(&mut self, value: AuthoredAssembly) -> Handle<AuthoredAssembly> {
        self.authored.insert(value)
    }
    pub(crate) fn authored(&self, handle: Handle<AuthoredAssembly>) -> &AuthoredAssembly {
        self.authored.get(handle)
    }
    pub(crate) fn has_authored(&self) -> bool {
        !self.authored.is_empty()
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
