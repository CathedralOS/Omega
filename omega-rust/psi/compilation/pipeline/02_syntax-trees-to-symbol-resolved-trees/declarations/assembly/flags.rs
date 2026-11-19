//! Realize observable hardware flags across instructions in one assembly block graph.
use super::{
    PreparedBlock, generated_statements,
    instructions::{Arithmetic, PreparedInstruction},
};
use language_core::inline_assembly::AsmInstructionShape;
use syntax_trees::{
    SyntaxTrees,
    expression::{ExpressionHandle, ExpressionNode},
    identifier::Identifier,
    statement::{StatementHandle, StatementNode},
};

#[derive(Default)]
pub(super) struct HardwareFlags {
    arithmetic_updates: Vec<(StatementHandle, Arithmetic)>,
    snapshots: Vec<StatementHandle>,
    pub(super) restores: bool,
}

impl HardwareFlags {
    pub(super) fn record(&mut self, instruction: &PreparedInstruction) {
        if let Some(arithmetic) = instruction.arithmetic {
            self.arithmetic_updates
                .push((instruction.statement, arithmetic));
        }
        match instruction.contract.shape {
            AsmInstructionShape::FlagsRestore => self.restores = true,
            AsmInstructionShape::FlagsSnapshot => self.snapshots.push(instruction.statement),
            _ => {}
        }
    }

    pub(super) fn observes(&self) -> bool {
        self.restores || !self.snapshots.is_empty()
    }

    pub(super) fn materialize(
        self,
        syntax_trees: &mut SyntaxTrees,
        blocks: &mut Vec<PreparedBlock>,
        body_requires: Vec<ExpressionHandle>,
        primitives: &mut Vec<ExpressionHandle>,
    ) {
        if self.snapshots.is_empty() || self.restores {
            return;
        }
        let Self {
            arithmetic_updates,
            snapshots: flag_snapshots,
            ..
        } = self;
        for (statement, arithmetic) in arithmetic_updates {
            let generated_start = syntax_trees.expressions.expression_count();
            let saved = generated_statements::preserve_sub_operands(
                syntax_trees,
                arithmetic.left,
                arithmetic.right,
            );
            let flags = generated_statements::arithmetic_flags(
                syntax_trees,
                arithmetic.result,
                arithmetic.producer,
            );
            let mask = generated_statements::replace_arithmetic_flags(syntax_trees);
            collect_generated_primitives(syntax_trees, generated_start, primitives);
            for block in blocks.iter_mut() {
                if let Some(index) = block
                    .instructions
                    .iter()
                    .position(|item| *item == statement)
                {
                    let count = saved.len();
                    block.instructions.splice(index..index, saved);
                    block
                        .instructions
                        .splice(index + count + 2..index + count + 2, [flags, mask]);
                    break;
                }
            }
        }
        let prefix = generated_statements::initialize_flags(syntax_trees);
        blocks[0].requires.drain(..body_requires.len());
        blocks.insert(
            0,
            PreparedBlock {
                label: Identifier::generated("asm#flags_entry"),
                requires: body_requires,
                instructions: prefix,
                ..Default::default()
            },
        );
        for snapshot in flag_snapshots {
            let StatementNode::Assignment(mut assignment) =
                syntax_trees.statements.statement(snapshot).clone()
            else {
                unreachable!()
            };
            let generated_start = syntax_trees.expressions.expression_count();
            let raw = generated_statements::observe_environment(syntax_trees, assignment.value);
            let mut observation = vec![raw];
            observation.extend(generated_statements::snapshot(syntax_trees));
            assignment.value = generated_statements::name(syntax_trees, "asm#flags_snapshot");
            syntax_trees
                .statements
                .replace_statement(snapshot, StatementNode::Assignment(assignment));
            let flag = generated_statements::observed_zero_flag(syntax_trees);
            collect_generated_primitives(syntax_trees, generated_start, primitives);
            for block in blocks.iter_mut() {
                if let Some(index) = block
                    .instructions
                    .iter()
                    .position(|candidate| *candidate == snapshot)
                {
                    observation.push(flag);
                    block.instructions.splice(index..index, observation);

                    break;
                }
            }
        }
    }
}

fn collect_generated_primitives(
    trees: &SyntaxTrees,
    first: usize,
    primitives: &mut Vec<ExpressionHandle>,
) {
    primitives.extend(
        trees
            .expressions
            .iter_expressions()
            .skip(first)
            .filter_map(|(handle, node)| {
                matches!(node, ExpressionNode::Binary(_)).then_some(handle)
            }),
    );
}
