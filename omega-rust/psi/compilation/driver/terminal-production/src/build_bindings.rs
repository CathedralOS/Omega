//! Admitted designated operands beside their exact emitted calls.
//!
//! Publication erases source joins. Capture them here, while the checked source
//! and optimized operations share custody; consumers never parse debug names or
//! reopen trees. Portable artifact bytes alone do not grant Build authority.

use language_semantics::declaration_selection::RootBindingRequest;
use semantic_vocabulary::OperationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBuildBindingOperation {
    Prepare,
    Bind,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: RootBindingRequest,
}

/// Source-issued context bound to one canonical semantic artifact. Its private
/// construction is not reproduced by loading arbitrary external bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_psi::SemanticFingerprint,
    calls: Vec<TerminalBuildBindingCall>,
}

impl TerminalBuildBindings {
    pub fn calls_for(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Option<&[TerminalBuildBindingCall]> {
        (self.identity == artifact.manifest().semantic().program_fingerprint).then_some(&self.calls)
    }

    pub(super) fn new(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        calls: Vec<TerminalBuildBindingCall>,
    ) -> Self {
        Self {
            identity: artifact.manifest().semantic().program_fingerprint,
            calls,
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
        if !call.root_binding.is_valid() {
            continue;
        }
        let request = checked
            .expression_table
            .root_binding_request(call.root_binding)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "Build call lost its designated operands before publication",
                )
            })?;
        let semantic_vocabulary::BoundaryInvocationSite::Operation(operation) =
            occurrence.terminal_site
        else {
            return Err(diagnostics::Diagnostic::error(
                "Build binding lost its ordinary result-producing call",
            ));
        };
        let action = match call.target.as_str() {
            "prepare" => TerminalBuildBindingOperation::Prepare,
            "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
            _ => {
                return Err(diagnostics::Diagnostic::error(
                    "checked Build binding has no admitted operation",
                ));
            }
        };
        let emitted = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find(|emitted| emitted.id == operation)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error("Build binding lost its emitted operation")
            })?;
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &emitted.kind else {
            return Err(diagnostics::Diagnostic::error(
                "Build binding no longer names a boundary call",
            ));
        };
        calls.push(TerminalBuildBindingCall {
            operation,
            boundary: *boundary,
            action,
            request: request.clone(),
        });
    }
    Ok(calls)
}
