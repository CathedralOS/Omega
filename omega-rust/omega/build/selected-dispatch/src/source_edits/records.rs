use super::{
    Diagnostic, ExpressionHandle, ExpressionNode, GraphGuard, HandleSpan, TypedTrees, rejected,
};
use typed_trees::statement::{StatementHandle, StatementNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Batch {
    pub(super) edits: Vec<ExpressionEdit>,
    pub(super) statement_edits: Vec<StatementEdit>,
    pub(super) guard: GraphGuard,
}

impl Batch {
    pub(super) fn validate(&self, program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for edit in &self.edits {
            if let Some(call) = &edit.original_call {
                call.validate(program)?;
            }
        }
        for edit in &self.statement_edits {
            if let Some(call) = &edit.original_call {
                call.validate_statement(program)?;
            }
        }
        self.guard.validate(program)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpressionEdit {
    pub(super) handle: ExpressionHandle,
    pub(super) original: ExpressionNode,
    pub(super) original_call: Option<ExpressionArguments>,
}

/// A statement-position call node replaced by selected dispatch. The whole
/// authored node is restored; the retained argument list proves the statement
/// table's operand span still names the same expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatementEdit {
    pub(super) handle: StatementHandle,
    pub(super) original: StatementNode,
    pub(super) original_call: Option<ExpressionArguments>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpressionArguments {
    pub(super) span: HandleSpan<ExpressionHandle>,
    pub(super) arguments: Vec<ExpressionHandle>,
}

impl ExpressionArguments {
    pub(super) fn validate(&self, program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        if program.expression_table.expression_handles(self.span) != self.arguments {
            return Err(rejected("original dispatch argument list changed"));
        }
        Ok(())
    }

    /// Statement-table calls keep their operand handles in the statement
    /// table's own expression-handle arena, not the expression table's.
    pub(super) fn validate_statement(&self, program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        if program.statement_table.expression_handles(self.span) != self.arguments {
            return Err(rejected("original dispatch argument list changed"));
        }
        Ok(())
    }
}
