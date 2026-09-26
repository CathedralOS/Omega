use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use symbols::SymbolKind;
use typed_trees as typed;

pub(crate) fn lower_authored_invocations(
    program: &resolved::SymbolResolvedTrees,
    declarations: &[resolved::name::DiagnosticName],
    parameters: &[resolved::signature::StateParameter],
    owner: &str,
) -> Result<Vec<typed::signature::AuthoredInvocation>, Diagnostic> {
    let mut lowered = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let target = if let Some((ordinal, parameter)) = parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.name.as_str() == declaration.as_str())
        {
            typed::signature::AuthoredInvocationTarget::Parameter {
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(format!(
                        "callable `{owner}` has too many invocation parameters for portable identity"
                    ))
                })?,
                symbol: parameter.symbol,
            }
        } else {
            // Invocation ceilings belong to the declaring source's namespace,
            // not the first same-spelled trait in the assembled forest.
            let service_symbol = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    declaration.as_str(),
                    &[SymbolKind::Trait],
                    declaration.source_span(),
                );
            service_symbol.map_or(
                typed::signature::AuthoredInvocationTarget::Unresolved,
                typed::signature::AuthoredInvocationTarget::Service,
            )
        };
        lowered.push(typed::signature::AuthoredInvocation {
            name: crate::lowerer::name::lower_name(declaration),
            source_span: declaration.source_span(),
            target,
        });
    }
    Ok(lowered)
}
