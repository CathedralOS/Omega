use omega_core::semantics::{
    ServiceReachId, ServiceReachRowId, ServiceReachRowTable, ServiceReachTable,
};
use omega_core::symbols::{SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

/// Normalize service reach only after every declaration and trait-parent edge
/// has a resolved symbol. Unknown or non-boundary names intentionally produce
/// no service member here; validation still owns the directed source error
/// while normalized rows remain incapable of containing an invalid member.
pub(crate) fn normalize_service_reaches(program: &mut SymbolResolvedTrees) {
    let mut boundary_traits = program
        .traits
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(|definition| {
            (
                definition.symbol,
                definition.name.as_str().to_owned(),
                program
                    .trait_requirements(definition.requires)
                    .iter()
                    .map(|requirement| requirement.symbol)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    boundary_traits.sort_by(|left, right| left.1.cmp(&right.1));

    let mut services = ServiceReachTable::default();
    for (symbol, name, _) in &boundary_traits {
        services.intern(*symbol, name);
    }
    for (symbol, _, parent_symbols) in &boundary_traits {
        let Some(service) = services.id_for_symbol(*symbol) else {
            continue;
        };
        let parents = parent_symbols
            .iter()
            .filter_map(|parent| services.id_for_symbol(*parent))
            .collect();
        services.set_parents(service, parents);
    }

    let mut rows = ServiceReachRowTable::default();
    rows.intern(Vec::new());

    let machine_rows = program
        .machines
        .iter()
        .map(|machine| {
            (
                machine.symbol,
                row_for_names(
                    &program.symbols,
                    &services,
                    &mut rows,
                    program
                        .machine_effects(machine)
                        .iter()
                        .map(|name| name.as_str()),
                ),
            )
        })
        .collect::<Vec<_>>();

    let signature_rows = program
        .traits
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition.machines))
        .map(|signature| {
            (
                signature.symbol,
                row_for_names(
                    &program.symbols,
                    &services,
                    &mut rows,
                    program
                        .signature_effects(signature.effects)
                        .iter()
                        .map(|name| name.as_str()),
                ),
            )
        })
        .collect::<Vec<_>>();

    program.machines.for_each_mut(|machine| {
        machine.service_reach_row = machine_rows
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
            .map(|(_, row)| *row)
            .unwrap_or(ServiceReachRowTable::EMPTY_ROW);
    });

    let signature_spans = program
        .traits
        .iter()
        .map(|definition| definition.machines)
        .collect::<Vec<_>>();
    let signature_arena = &mut program.tables.declarations.trait_machine_signatures;
    for span in signature_spans {
        for signature in signature_arena.span_mut_or_empty(span) {
            signature.service_reach_row = signature_rows
                .iter()
                .find(|(symbol, _)| *symbol == signature.symbol)
                .map(|(_, row)| *row)
                .unwrap_or(ServiceReachRowTable::EMPTY_ROW);
        }
    }

    normalize_machine_parameter_rows(program, &services, &mut rows);

    program.service_reaches = services;
    program.service_reach_rows = rows;
}

fn normalize_machine_parameter_rows(
    program: &mut SymbolResolvedTrees,
    services: &ServiceReachTable,
    rows: &mut ServiceReachRowTable,
) {
    use omega_symbol_resolved_trees::data::TypeParameterKind;

    let mut roots = program
        .data_definitions
        .iter()
        .map(|definition| definition.type_parameters)
        .chain(
            program
                .machines
                .iter()
                .map(|machine| machine.type_parameters),
        )
        .chain(
            program
                .traits
                .iter()
                .map(|definition| definition.type_parameters),
        )
        .collect::<Vec<_>>();
    for definition in &program.traits {
        roots.extend(
            program
                .trait_machine_signatures(definition.machines)
                .iter()
                .map(|signature| signature.type_parameters),
        );
    }

    let type_parameters = &program.tables.declarations.data_type_parameters;
    let mut spans = Vec::new();
    for root in roots {
        collect_parameter_spans(type_parameters, root, &mut spans);
    }

    let effect_names = spans
        .iter()
        .flat_map(|span| type_parameters.span_or_empty(*span))
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Machine { contract } => Some((
                parameter.symbol,
                program
                    .signature_effects(contract.effects)
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    let type_parameters = &mut program.tables.declarations.data_type_parameters;
    for span in spans {
        for parameter in type_parameters.span_mut_or_empty(span) {
            let TypeParameterKind::Machine { contract } = &mut parameter.kind else {
                continue;
            };
            let names = effect_names
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
                .map(|(_, names)| names.as_slice())
                .unwrap_or(&[]);
            contract.service_reach_row = row_for_names(
                &program.symbols,
                services,
                rows,
                names.iter().map(String::as_str),
            );
        }
    }
}

fn collect_parameter_spans(
    arena: &omega_core::arena::Arena<omega_symbol_resolved_trees::data::TypeParameter>,
    span: omega_core::arena::HandleSpan<omega_symbol_resolved_trees::data::TypeParameter>,
    spans: &mut Vec<
        omega_core::arena::HandleSpan<omega_symbol_resolved_trees::data::TypeParameter>,
    >,
) {
    use omega_symbol_resolved_trees::data::TypeParameterKind;
    if span.is_empty() || spans.contains(&span) {
        return;
    }
    spans.push(span);
    for parameter in arena.span_or_empty(span) {
        if let TypeParameterKind::Machine { contract } = &parameter.kind {
            collect_parameter_spans(arena, contract.type_parameters, spans);
        }
    }
}

fn row_for_names<'a>(
    symbols: &SymbolTable,
    services: &ServiceReachTable,
    rows: &mut ServiceReachRowTable,
    names: impl IntoIterator<Item = &'a str>,
) -> ServiceReachRowId {
    let mut members = Vec::new();
    for name in names {
        if let Some(service) = service_for_name(symbols, services, name) {
            services.extend_closure(service, &mut members);
        }
    }
    rows.intern(members)
}

fn service_for_name(
    symbols: &SymbolTable,
    services: &ServiceReachTable,
    name: &str,
) -> Option<ServiceReachId> {
    let symbol = symbols.find_child_by_name_and_kind(symbols.root(), name, SymbolKind::Trait)?;
    services.id_for_symbol(symbol)
}
