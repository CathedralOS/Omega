use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;

#[derive(Debug, Clone)]
pub(crate) struct ExpandedDomainReference {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) path: Vec<resolved::name::DiagnosticName>,
}

/// Expand a transparent domain alias to its atomic declared domains. This is
/// the shared pre-normalization operation for proof facts, executable
/// membership, and constrained types.
pub(crate) fn expand_domain_reference(
    program: &resolved::SymbolResolvedTrees,
    symbol: symbols::SymbolHandle,
    path: Vec<resolved::name::DiagnosticName>,
) -> Result<Vec<ExpandedDomainReference>, Diagnostic> {
    fn expand(
        program: &resolved::SymbolResolvedTrees,
        symbol: symbols::SymbolHandle,
        path: Vec<resolved::name::DiagnosticName>,
        stack: &mut Vec<symbols::SymbolHandle>,
    ) -> Result<Vec<ExpandedDomainReference>, Diagnostic> {
        let name = path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        if name == "Carry::Portable" {
            return Ok(language_semantics::CarryPermission::ALL
                .into_iter()
                .map(|permission| {
                    let [namespace, member] = permission
                        .name()
                        .split("::")
                        .collect::<Vec<_>>()
                        .try_into()
                        .expect("compiler carry permission has two path members");
                    ExpandedDomainReference {
                        symbol: symbols::SymbolHandle::invalid(),
                        path: vec![
                            resolved::name::DiagnosticName::generated_static(namespace),
                            resolved::name::DiagnosticName::generated_static(member),
                        ],
                    }
                })
                .collect());
        }
        let Some(domain) = program
            .domain_definitions
            .iter()
            .find(|domain| domain.symbol == symbol)
        else {
            // Preserve unresolved spellings for the validation layer, which
            // owns the ordinary unknown-domain diagnostic.
            return Ok(vec![ExpandedDomainReference { symbol, path }]);
        };
        let Some(alias) = domain.alias.as_ref() else {
            return Ok(vec![ExpandedDomainReference { symbol, path }]);
        };
        if let Some(cycle_start) = stack.iter().position(|candidate| *candidate == symbol) {
            let cycle = stack[cycle_start..]
                .iter()
                .copied()
                .chain(std::iter::once(symbol))
                .filter_map(|candidate| {
                    program
                        .domain_definitions
                        .iter()
                        .find(|domain| domain.symbol == candidate)
                })
                .map(|domain| domain.name.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::error(format!("domain alias cycle: {cycle}")));
        }
        if alias.constituents.is_empty() {
            return Err(Diagnostic::error(format!(
                "domain alias `{}` must name at least one constituent",
                domain.name
            )));
        }

        stack.push(symbol);
        let mut expanded = Vec::new();
        for constituent in &alias.constituents {
            let constituent_path = program.domain_path_members(constituent.domain).to_vec();
            expanded.extend(expand(
                program,
                constituent.domain_symbol,
                constituent_path,
                stack,
            )?);
        }
        stack.pop();
        Ok(expanded)
    }

    expand(program, symbol, path, &mut Vec::new())
}
