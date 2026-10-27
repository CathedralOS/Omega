//! Source provenance and emitted-operation custody shared by Build protocols.

pub(super) fn capture<Action: Copy>(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    protocol: &str,
    action: impl Fn(&str) -> Option<Action>,
) -> Result<
    Vec<(
        semantic_vocabulary::OperationId,
        semantic_vocabulary::BoundaryMachineId,
        Action,
    )>,
    diagnostics::Diagnostic,
> {
    let mut declarations = Vec::new();
    for definition in checked.traits() {
        if !definition.is_boundary
            || !language_semantics::build_requests::is_build_protocol(
                &checked.symbols,
                definition.symbol,
                protocol,
            )
        {
            continue;
        }
        for signature in checked.trait_machine_signatures(definition) {
            let selected = action(signature.name.as_str()).ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
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
        let semantic_vocabulary::InvocationSite::Operation(operation) = occurrence.terminal_site
        else {
            return Err(diagnostics::Diagnostic::error(format!(
                "{protocol} lost its ordinary boundary operation"
            )));
        };
        calls.push((
            operation,
            crate::build_bindings::emitted_boundary(lowered, operation)?,
            *action,
        ));
    }
    Ok(calls)
}
