//! Source-issued lexical authority beside exact Product query operations.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBuildQueryOperation {
    Entry,
    Schema,
    Provider,
    SchemaPath,
    ProviderPath,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildQueryCall {
    pub operation: semantic_vocabulary::OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub action: TerminalBuildQueryOperation,
    pub occurrence: source::SourceSpan,
}

pub(super) fn capture(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildQueryCall>, diagnostics::Diagnostic> {
    let calls =
        crate::build_protocol::capture(checked, lowered, "BuildProductOperations", |name| {
            Some(match name {
                "entry" => TerminalBuildQueryOperation::Entry,
                "schema" => TerminalBuildQueryOperation::Schema,
                "provider" => TerminalBuildQueryOperation::Provider,
                "schema_path" => TerminalBuildQueryOperation::SchemaPath,
                "provider_path" => TerminalBuildQueryOperation::ProviderPath,
                _ => return None,
            })
        })?;
    Ok(calls
        .into_iter()
        .map(
            |(operation, boundary, action, occurrence)| TerminalBuildQueryCall {
                operation,
                boundary,
                action,
                occurrence,
            },
        )
        .collect())
}
