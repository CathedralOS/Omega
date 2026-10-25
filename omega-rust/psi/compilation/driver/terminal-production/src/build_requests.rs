//! Exact toolchain Build request declarations beside their emitted operations.
//!
//! The host reads evaluated arguments, never syntax or debug names. This
//! companion authenticates which ordinary boundary calls belong to the
//! compiler-owned protocol; loading an artifact alone cannot recreate it.

use semantic_vocabulary::{BoundaryMachineId, OperationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBuildRequestOperation {
    Package,
    Application,
    Member,
    ArtifactOnly,
    PathDependency,
    GitDependency,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildRequestCall {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub action: TerminalBuildRequestOperation,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildRequests {
    identity: terminal_psi::SemanticFingerprint,
    calls: Vec<TerminalBuildRequestCall>,
}

impl TerminalBuildRequests {
    pub fn calls_for(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Option<&[TerminalBuildRequestCall]> {
        (self.identity == artifact.manifest().semantic().program_fingerprint).then_some(&self.calls)
    }

    pub(super) fn new(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        calls: Vec<TerminalBuildRequestCall>,
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
) -> Result<Vec<TerminalBuildRequestCall>, diagnostics::Diagnostic> {
    let mut declarations = Vec::new();
    for definition in checked.traits() {
        if !definition.is_boundary
            || !language_semantics::build_requests::is_project_request_protocol(
                &checked.symbols,
                definition.symbol,
            )
        {
            continue;
        }
        for signature in checked.trait_machine_signatures(definition) {
            use TerminalBuildRequestOperation as Request;
            let action = match signature.name.as_str() {
                "package" => Request::Package,
                "application" => Request::Application,
                "member" => Request::Member,
                "artifact_only" => Request::ArtifactOnly,
                "depend_path" => Request::PathDependency,
                "depend_git" => Request::GitDependency,
                _ => {
                    return Err(diagnostics::Diagnostic::error(
                        "unknown toolchain Build request operation",
                    ));
                }
            };
            declarations.push((signature.symbol, action));
        }
    }
    let mut calls = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        use checked_trees::NominalMachineUseSite as Site;
        let target = match occurrence.source_site {
            Some(Site::Statement(statement)) => {
                match checked.statement_table.statement(statement) {
                    typed_trees::statement::StatementNode::Call(call) => call.target_symbol,
                    _ => continue,
                }
            }
            Some(Site::Expression(expression)) => {
                match checked.expression_table.expression(expression) {
                    typed_trees::expression::ExpressionNode::Call(call) => call.target_symbol,
                    _ => continue,
                }
            }
            None => continue,
        };
        let Some((_, action)) = declarations.iter().find(|(symbol, _)| *symbol == target) else {
            continue;
        };
        let semantic_vocabulary::BoundaryInvocationSite::Operation(operation) =
            occurrence.terminal_site
        else {
            return Err(diagnostics::Diagnostic::error(
                "Build request lost its ordinary boundary operation",
            ));
        };
        let boundary = crate::build_bindings::emitted_boundary(lowered, operation)?;
        calls.push(TerminalBuildRequestCall {
            operation,
            boundary,
            action: *action,
        });
    }
    Ok(calls)
}
