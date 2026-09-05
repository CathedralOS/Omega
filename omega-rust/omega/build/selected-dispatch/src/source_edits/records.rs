use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Batch {
    pub(super) edits: Vec<Edit>,
    pub(super) guard: GraphGuard,
}

impl Batch {
    pub(super) fn validate(&self, program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for edit in &self.edits {
            match edit {
                Edit::Expression { original_call, .. } => {
                    if let Some(call) = original_call {
                        call.validate(program)?;
                    }
                }
                Edit::Statement {
                    handle,
                    original,
                    original_storage,
                    settled,
                    settled_storage,
                } => {
                    let StatementNode::Call(current) = program.statement_table.statement(*handle)
                    else {
                        return Err(rejected("settled statement is no longer a call"));
                    };
                    if current != settled {
                        return Err(rejected("settled statement call changed"));
                    }
                    original_storage.validate(program, original)?;
                    settled_storage.validate(program, settled)?;
                }
            }
        }
        self.guard.validate(program)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Edit {
    Expression {
        handle: ExpressionHandle,
        original: ExpressionNode,
        original_call: Option<ExpressionArguments>,
    },
    Statement {
        handle: Handle<StatementNode>,
        original: TableCall,
        original_storage: StatementStorage,
        settled: TableCall,
        settled_storage: StatementStorage,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatementStorage {
    pub(super) arguments: Vec<ExpressionHandle>,
    pub(super) receiver: Vec<Identifier>,
}

impl StatementStorage {
    pub(super) fn capture(program: &TypedTrees, call: &TableCall) -> Self {
        Self {
            arguments: program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec(),
            receiver: program
                .statement_table
                .name_path_members(call.receiver)
                .to_vec(),
        }
    }

    pub(super) fn validate(
        &self,
        program: &TypedTrees,
        call: &TableCall,
    ) -> Result<(), Vec<Diagnostic>> {
        if self != &Self::capture(program, call) {
            return Err(rejected(
                "dispatch statement operand or receiver list changed",
            ));
        }
        Ok(())
    }
}
