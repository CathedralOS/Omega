//! Resolve domain attachments in the machine declaration's own source scope.

use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::name::DiagnosticName;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Keep a domain only when its authored name resolves to one declaration
/// across ordinary membership lookup and exposed domains.
pub(crate) fn domain_named(
    program: &SymbolResolvedTrees,
    name: &DiagnosticName,
) -> Option<SymbolHandle> {
    match crate::resolution::names::membership_domain_lookup(
        &program.symbols,
        name.as_str(),
        name.source_span(),
    ) {
        crate::resolution::names::DomainMembership::Unique(symbol) => Some(symbol),
        crate::resolution::names::DomainMembership::NotFound
        | crate::resolution::names::DomainMembership::Ambiguous { .. }
        | crate::resolution::names::DomainMembership::Contested => None,
    }
}

/// A token binding and a resource projection both name their domain in the
/// machine's own source scope. The core Content declaration also restricts
/// the projection to a domain owned by the declaring package.
pub(crate) fn resolve_machine_domain_homes(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut homes = Vec::new();
    let mut diagnostics = Vec::new();
    for (position, machine) in program.machines.iter().enumerate() {
        if !program.symbols.symbol_source_is_selected(machine.symbol) {
            continue;
        }
        let projection = program
            .machine_trait_conformances(machine.satisfies)
            .iter()
            .any(|edge| {
                program.symbols.core_vocabulary(edge.symbol)
                    == Some(language_core::core_vocabulary::CoreVocabulary::Content)
                    && edge
                        .requirement
                        .as_ref()
                        .is_some_and(|name| name.as_str() == "project")
            });
        if machine.spelling.is_none() && !projection {
            continue;
        }
        let home = machine
            .attached_data
            .as_ref()
            .filter(|_| !machine.attached_data_symbol.is_valid())
            .and_then(|name| domain_named(program, name));
        if projection
            && !home.is_some_and(|domain| {
                program
                    .symbols
                    .same_symbol_source_package(machine.symbol, domain)
            })
        {
            diagnostics.push(
                Diagnostic::error(
                    "a core Content projection must name a domain owned by the package declaring the projection",
                )
                .with_source_span(machine.name.source_span()),
            );
        }
        homes.push((position, home.unwrap_or_else(SymbolHandle::invalid)));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (position, home) in homes {
        program.machines[position].attached_domain_symbol = home;
    }
    Ok(())
}
