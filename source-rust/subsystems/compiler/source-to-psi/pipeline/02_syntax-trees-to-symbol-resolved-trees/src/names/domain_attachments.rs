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
    let mut projections = std::collections::HashMap::<_, Vec<_>>::new();
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
        if projection && let Some(domain) = home {
            projections.entry(domain).or_default().push(machine.symbol);
        }
        homes.push((position, home.unwrap_or_else(SymbolHandle::invalid)));
    }
    // A domain without parameters names one exact qualification. Its first
    // publisher can live in an earlier checked cohort of the same package.
    // An applied family must instead compare concrete domain applications.
    for domain in program.domain_definitions.iter() {
        if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
            continue;
        }
        let mut first = domain.content_projections.first().copied();
        for &candidate in projections.get(&domain.symbol).into_iter().flatten() {
            if first.is_some_and(|prior| prior != candidate) {
                let mut error = Diagnostic::error(
                    "an exact qualification may publish only one core Content projection",
                );
                if let Some(span) = program.symbols.symbol_source_span(candidate) {
                    error = error.with_source_span(span);
                }
                diagnostics.push(error);
            } else {
                first = Some(candidate);
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (position, home) in homes {
        program.machines[position].attached_domain_symbol = home;
    }
    program.roots.domain_definitions.for_each_mut(|domain| {
        // Retained source can reactivate the domain without the file that
        // published its projection. Keep that dependency visible to selection.
        for projection in projections.remove(&domain.symbol).into_iter().flatten() {
            if !domain.content_projections.contains(&projection) {
                domain.content_projections.push(projection);
            }
        }
    });
    Ok(())
}
