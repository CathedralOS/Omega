//! Source-issued filesystem protocol calls bound to one Terminal invocation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBuildFilesystemOperation {
    ResolveSource,
    ResolveOutput,
    NamedInput,
    Open,
    Read,
    CloseSource,
    Create,
    Write,
    CloseOutput,
    IncludeSource,
    Require,
    CanComplete,
    Complete,
    Fail,
    RequiredPath,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildFilesystemCall {
    pub operation: semantic_vocabulary::OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub action: TerminalBuildFilesystemOperation,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildFilesystem {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Vec<TerminalBuildFilesystemCall>,
}

impl TerminalBuildFilesystem {
    pub fn calls_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&[TerminalBuildFilesystemCall]> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub(super) fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        calls: Vec<TerminalBuildFilesystemCall>,
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
) -> Result<Vec<TerminalBuildFilesystemCall>, diagnostics::Diagnostic> {
    let calls =
        crate::build_protocol::capture(checked, lowered, "BuildFilesystemOperations", |name| {
            use TerminalBuildFilesystemOperation as Operation;
            Some(match name {
                "resolve_source" => Operation::ResolveSource,
                "resolve_output" => Operation::ResolveOutput,
                "named_input" => Operation::NamedInput,
                "open" => Operation::Open,
                "read" => Operation::Read,
                "close_source" => Operation::CloseSource,
                "create" => Operation::Create,
                "write" => Operation::Write,
                "close_output" => Operation::CloseOutput,
                "include_source" => Operation::IncludeSource,
                "require" => Operation::Require,
                "can_complete" => Operation::CanComplete,
                "complete" => Operation::Complete,
                "fail" => Operation::Fail,
                "required_path" => Operation::RequiredPath,
                _ => return None,
            })
        })?;
    Ok(calls
        .into_iter()
        .map(
            |(operation, boundary, action)| TerminalBuildFilesystemCall {
                operation,
                boundary,
                action,
            },
        )
        .collect())
}
