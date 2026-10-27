//! Admitted designated operands beside their exact emitted calls.
//!
//! Publication erases source joins. Capture them here, while the checked source
//! and optimized operations share custody; consumers never parse debug names or
//! reopen trees. Portable artifact bytes alone do not grant Build authority.

use language_semantics::declaration_selection::{BuildSelectionRequest, ProductDeclarationRequest};
use semantic_vocabulary::OperationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBuildBindingOperation {
    Prepare,
    Bind,
    Select,
    SelectMode,
    Exclude,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: BuildSelectionRequest,
    pub selecting_machine: Option<source::SourceSpan>,
}

/// Source-issued context bound to exact Terminal semantics and, for private
/// code, its live invocation owner. Loading bytes cannot recreate this context.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Vec<TerminalBuildBindingCall>,
    declarations: Vec<std::sync::Arc<ProductDeclarationRequest>>,
}

impl TerminalBuildBindings {
    pub fn calls_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&[TerminalBuildBindingCall]> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub fn declarations_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&[std::sync::Arc<ProductDeclarationRequest>]> {
        (self.identity == artifact.custody_identity()).then_some(&self.declarations)
    }

    pub(super) fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        calls: Vec<TerminalBuildBindingCall>,
        declarations: Vec<std::sync::Arc<ProductDeclarationRequest>>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls,
            declarations,
        }
    }
}

pub(super) fn capture(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildBindingCall>, diagnostics::Diagnostic> {
    let mut calls = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
            occurrence.source_site
        else {
            continue;
        };
        let typed_trees::expression::ExpressionNode::Call(call) =
            checked.expression_table.expression(expression)
        else {
            continue;
        };
        if !call.build_selection.is_valid() {
            continue;
        }
        let request = checked
            .expression_table
            .build_selection_request(call.build_selection)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "Build call lost its designated operands before publication",
                )
            })?;
        let semantic_vocabulary::InvocationSite::Operation(operation) = occurrence.terminal_site
        else {
            return Err(diagnostics::Diagnostic::error(
                "Build binding lost its ordinary result-producing call",
            ));
        };
        let action = match call.target.as_str() {
            "prepare" => TerminalBuildBindingOperation::Prepare,
            "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
            "select" => TerminalBuildBindingOperation::Select,
            "select_mode" => TerminalBuildBindingOperation::SelectMode,
            "exclude" => TerminalBuildBindingOperation::Exclude,
            _ => {
                return Err(diagnostics::Diagnostic::error(
                    "checked Build binding has no admitted operation",
                ));
            }
        };
        let boundary = emitted_boundary(lowered, operation)?;
        let selecting_machine = if request.requires_lexical_subject() {
            Some(
                checked
                    .machines()
                    .iter()
                    .find(|machine| {
                        checked
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == occurrence.source_state)
                    })
                    .and_then(|machine| checked.symbols.symbol_source_span(machine.symbol))
                    .ok_or_else(|| {
                        diagnostics::Diagnostic::error(
                            "Build selection lost its authored machine coordinate",
                        )
                    })?,
            )
        } else {
            None
        };
        calls.push(TerminalBuildBindingCall {
            operation,
            boundary,
            action,
            request: request.clone(),
            selecting_machine,
        });
    }
    Ok(calls)
}

pub(super) fn emitted_boundary(
    lowered: &lowered_psi::LoweredPsi,
    operation: OperationId,
) -> Result<semantic_vocabulary::BoundaryMachineId, diagnostics::Diagnostic> {
    let emitted = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|emitted| emitted.id == operation)
        .ok_or_else(|| diagnostics::Diagnostic::error("Build operation lost its emitted call"))?;
    let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &emitted.kind else {
        return Err(diagnostics::Diagnostic::error(
            "Build operation no longer names a boundary call",
        ));
    };
    Ok(*boundary)
}

/// Formation sees every checked source request, including choices in helpers
/// and branches that optimization or execution never reaches. Copies of one
/// authored occurrence do not create another formation obligation.
pub(super) fn capture_declarations(
    checked: &checked_trees::CheckedTrees,
) -> Vec<std::sync::Arc<ProductDeclarationRequest>> {
    let mut declarations = Vec::new();
    for (_, node) in checked.expression_table.iter_expressions() {
        let typed_trees::expression::ExpressionNode::Call(call) = node else {
            continue;
        };
        let Some(BuildSelectionRequest::Declaration(request)) = checked
            .expression_table
            .build_selection_request(call.build_selection)
        else {
            continue;
        };
        if !declarations.contains(request) {
            declarations.push(request.clone());
        }
    }
    declarations
}
