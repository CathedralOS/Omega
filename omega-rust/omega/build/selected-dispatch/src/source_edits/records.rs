use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Batch {
    pub(super) edits: Vec<ExpressionEdit>,
    pub(super) guard: GraphGuard,
}

impl Batch {
    pub(super) fn validate(&self, program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for edit in &self.edits {
            if let Some(call) = &edit.original_call {
                call.validate(program)?;
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
}
