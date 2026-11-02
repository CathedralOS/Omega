//! Source provenance and emitted-operation custody shared by Build protocols.

use crate::build_bindings::emitted_boundary;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::build_requests::is_build_protocol;
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{BoundaryMachineId, InvocationSite, OperationId};
use source::SourceSpan;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

pub(super) fn capture<Action: Copy>(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    protocol: &str,
    action: impl Fn(&str) -> Option<Action>,
) -> Result<Vec<(OperationId, BoundaryMachineId, Action, SourceSpan)>, Diagnostic> {
    let mut declarations = Vec::new();
    for definition in checked.traits() {
        if !definition.is_boundary
            || !is_build_protocol(&checked.symbols, definition.symbol, protocol)
        {
            continue;
        }
        for signature in checked.trait_machine_signatures(definition) {
            let selected = action(signature.name.as_str()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "unknown toolchain {protocol} operation `{}`",
                    signature.name
                ))
            })?;
            declarations.push((signature.symbol, selected));
        }
    }
    let mut calls = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        use checked_trees::NominalMachineUseSite as Site;
        let (target, source_span) = match occurrence.source_site {
            Some(Site::Statement(statement)) => {
                match checked.statement_table.statement(statement) {
                    StatementNode::Call(call) => (call.target_symbol, call.source_span),
                    _ => continue,
                }
            }
            Some(Site::Expression(expression)) => {
                match checked.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => (
                        call.target_symbol,
                        checked.expression_table.source_span(expression),
                    ),
                    _ => continue,
                }
            }
            None => continue,
        };
        let Some((_, action)) = declarations.iter().find(|(symbol, _)| *symbol == target) else {
            continue;
        };
        let InvocationSite::Operation(operation) = occurrence.terminal_site else {
            return Err(Diagnostic::error(format!(
                "{protocol} lost its ordinary boundary operation"
            )));
        };
        calls.push((
            operation,
            emitted_boundary(lowered, operation)?,
            *action,
            source_span,
        ));
    }
    Ok(calls)
}
