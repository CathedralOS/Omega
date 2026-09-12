use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::StaticMachineArgument;

#[derive(Default)]
pub(crate) struct SourceEditBuilder {
    ignored: bool,
    edits: Vec<ExpressionEdit>,
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
        self.edits.push(ExpressionEdit {
            handle,
            original,
            original_call,
        });
    }

    pub(crate) fn finish(
        self,
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
