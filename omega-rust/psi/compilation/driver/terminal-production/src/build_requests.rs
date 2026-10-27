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
    LogLine,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildRequestCall {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub action: TerminalBuildRequestOperation,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildRequests {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Vec<TerminalBuildRequestCall>,
}

impl TerminalBuildRequests {
    pub fn calls_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&[TerminalBuildRequestCall]> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub(super) fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        calls: Vec<TerminalBuildRequestCall>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls,
        }
    }
}

pub(super) fn capture(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildRequestCall>, diagnostics::Diagnostic> {
    let calls =
        crate::build_protocol::capture(checked, lowered, "BuildRequestOperations", |name| {
            use TerminalBuildRequestOperation as Request;
            Some(match name {
                "package" => Request::Package,
                "application" => Request::Application,
                "member" => Request::Member,
                "artifact_only" => Request::ArtifactOnly,
                "depend_path" => Request::PathDependency,
                "depend_git" => Request::GitDependency,
                "write_log" => Request::LogLine,
                _ => return None,
            })
        })?;
    Ok(calls
        .into_iter()
        .map(
            |(operation, boundary, action, _)| TerminalBuildRequestCall {
                operation,
                boundary,
                action,
            },
        )
        .collect())
}
