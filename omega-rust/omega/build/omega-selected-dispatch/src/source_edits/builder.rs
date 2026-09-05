use super::*;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::StaticMachineArgument;

#[derive(Default)]
pub(crate) struct SourceEditBuilder {
    ignored: bool,
    edits: Vec<Edit>,
    roots: Vec<ExpressionHandle>,
    symbols: Vec<SymbolHandle>,
    static_arguments: Vec<StaticMachineArgument>,
    failure: Option<Vec<Diagnostic>>,
    unsupported: bool,
}

impl SourceEditBuilder {
    pub(crate) fn ignored() -> Self {
        Self {
            ignored: true,
            ..Self::default()
        }
    }

    pub(crate) fn expression(&mut self, program: &TypedTrees, handle: ExpressionHandle) {
        if self.ignored || self.failure.is_some() {
            return;
        }
        let original = program.expression_table.expression(handle);
        if let ExpressionNode::Call(call) = original {
            if let Err(diagnostics) = guard::validate_call_static_arguments(call) {
                self.failure = Some(diagnostics);
                return;
            }
            self.static_arguments
                .extend_from_slice(&call.machine_arguments);
            if let Some(dispatch) = &call.static_requirement_dispatch {
                self.symbols.extend([
                    dispatch.declaring_trait,
                    dispatch.requirement,
                    dispatch.realization_machine,
                    dispatch.realization_state,
                ]);
            }
            if let Some(request) = &call.quotient_operation {
                self.static_arguments
                    .push(request.representative_operation.clone());
                self.static_arguments.extend(
                    request
                        .theorem_evidence
                        .iter()
                        .map(|theorem| theorem.application.clone()),
                );
            }
            if let Some(request) = &call.private_layout_operation {
                self.static_arguments.push(request.selected_slot.clone());
            }
        }
        let original = original.clone();
        let original_call = match &original {
            ExpressionNode::Call(call) => {
                self.symbols.push(call.target_symbol);
                self.roots.push(call.receiver);
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec();
                self.roots.extend_from_slice(&arguments);
                Some(ExpressionArguments {
                    span: call.arguments,
                    arguments,
                })
            }
            ExpressionNode::Binary(binary) => {
                self.roots.extend([binary.left, binary.right]);
                None
            }
            ExpressionNode::Unary(unary) => {
                self.roots.push(unary.operand);
                None
            }
            ExpressionNode::Indexed(indexed) => {
                self.roots.extend([indexed.collection, indexed.index]);
                None
            }
            _ => {
                self.unsupported = true;
                None
            }
        };
        self.roots.push(handle);
        self.edits.push(Edit::Expression {
            handle,
            original,
            original_call,
        });
    }

    pub(crate) fn statement(&mut self, program: &TypedTrees, handle: Handle<StatementNode>) {
        if self.ignored || self.failure.is_some() {
            return;
        }
        let StatementNode::Call(original) = program.statement_table.statement(handle) else {
            self.unsupported = true;
            return;
        };
        if let Err(diagnostics) = guard::validate_static_arguments(&original.machine_arguments) {
            self.failure = Some(diagnostics);
            return;
        }
        self.static_arguments
            .extend_from_slice(&original.machine_arguments);
        let original_storage = StatementStorage::capture(program, original);
        self.roots.extend_from_slice(&original_storage.arguments);
        self.symbols.extend([
            original.receiver_root_symbol,
            original.receiver_symbol,
            original.target_symbol,
        ]);
        if let Some(dispatch) = &original.static_requirement_dispatch {
            self.symbols.extend([
                dispatch.declaring_trait,
                dispatch.requirement,
                dispatch.realization_machine,
                dispatch.realization_state,
            ]);
        }
        self.edits.push(Edit::Statement {
            handle,
            original: original.clone(),
            original_storage: original_storage.clone(),
            settled: original.clone(),
            settled_storage: original_storage,
        });
    }

    pub(crate) fn finish(
        mut self,
        program: &TypedTrees,
    ) -> Result<SelectedDispatchSourceEdits, Vec<Diagnostic>> {
        if let Some(diagnostics) = self.failure {
            return Err(diagnostics);
        }
        if self.unsupported {
            return Err(rejected("unsupported original dispatch edit shape"));
        }
        if self.edits.is_empty() {
            return Ok(SelectedDispatchSourceEdits::default());
        }
        for edit in &mut self.edits {
            if let Edit::Statement {
                handle,
                settled,
                settled_storage,
                ..
            } = edit
            {
                let StatementNode::Call(current) = program.statement_table.statement(*handle)
                else {
                    return Err(rejected(
                        "settlement replaced a statement call with another statement kind",
                    ));
                };
                guard::validate_static_arguments(&current.machine_arguments)?;
                self.static_arguments
                    .extend_from_slice(&current.machine_arguments);
                *settled = current.clone();
                *settled_storage = StatementStorage::capture(program, current);
                self.roots.extend_from_slice(&settled_storage.arguments);
                self.symbols.extend([
                    current.receiver_root_symbol,
                    current.receiver_symbol,
                    current.target_symbol,
                ]);
                if let Some(dispatch) = &current.static_requirement_dispatch {
                    self.symbols.extend([
                        dispatch.declaring_trait,
                        dispatch.requirement,
                        dispatch.realization_machine,
                        dispatch.realization_state,
                    ]);
                }
            }
        }
        let guard = GraphGuard::capture(
            program,
            &self.roots,
            &[],
            &self.symbols,
            &self.static_arguments,
        )?;
        Ok(SelectedDispatchSourceEdits {
            batches: vec![Batch {
                edits: self.edits,
                guard,
            }],
        })
    }
}
