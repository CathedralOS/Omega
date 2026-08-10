use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    ServiceReachId, ServiceReachRowId, ServiceReachRowTable, ServiceReachTable,
};
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbols::{SymbolKind, SymbolTable};

/// Normalize service reach only after every declaration and trait-parent edge
/// has a resolved symbol. Authored root-machine names live only in the
/// lowering-private sidecar and are validated before the published resolved
/// trees are returned; normalized rows therefore cannot contain an invalid
/// member or retain a parallel spelling contract.
pub(crate) fn normalize_service_reaches(
    program: &mut SymbolResolvedTrees,
    machine_service_reaches: &[(psi_symbols::SymbolHandle, Vec<DiagnosticName>)],
) -> Result<(), Diagnostic> {
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
            let authored = machine_service_reaches
                .iter()
                .find(|(symbol, _)| *symbol == machine.symbol)
                .map(|(_, reaches)| reaches.as_slice())
                .expect("surviving resolved machine retains its pending authored service row");
            validate_machine_service_reaches(program, &services, machine, authored)?;
            let mut names = authored
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect::<Vec<_>>();
            let parameters = program
                .machine_state_handles(machine.states)
                .first()
                .map(|state| program.state_parameters(program.machine_state(*state).parameters))
                .unwrap_or_default();
            names.extend(invoked_service_names(
                program,
                &services,
                program.machine_invokes(machine),
                parameters,
            ));
            Ok((
                machine.symbol,
                row_for_names(
                    &program.symbols,
                    &services,
                    &mut rows,
                    names.iter().map(String::as_str),
                ),
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    let signature_rows = program
        .traits
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition.machines))
        .map(|signature| {
            let mut names = program
                .signature_service_reaches(signature.service_reaches)
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect::<Vec<_>>();
            names.extend(invoked_service_names(
                program,
                &services,
                program.signature_invokes(signature.invokes),
                program.state_parameters(signature.parameters),
            ));
            (
                signature.symbol,
                row_for_names(
                    &program.symbols,
                    &services,
                    &mut rows,
                    names.iter().map(String::as_str),
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
    Ok(())
}

fn validate_machine_service_reaches(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    machine: &psi_symbol_resolved_trees::machine::Machine,
    authored: &[DiagnosticName],
) -> Result<(), Diagnostic> {
    for service in authored {
        if service_for_name(&program.symbols, services, service.as_str()).is_none() {
            return Err(Diagnostic::error(format!(
                "machine `{}` declares unknown boundary service `{service}`",
                machine.name,
            )));
        }
    }

    if matches!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::ExternalRealization { .. }
    ) && !authored.is_empty()
    {
        return Err(Diagnostic::error(format!(
            "external leaf `{}` repeats an authored `reaches` row, but `via` derives behavior from the satisfied requirement and admitted binding; remove the leaf's `reaches` clause",
            machine.name,
        )));
    }

    Ok(())
}

fn invoked_service_names(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    invokes: &[psi_symbol_resolved_trees::name::DiagnosticName],
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
) -> Vec<String> {
    let mut names = Vec::new();
    for invocation in invokes {
        let service = parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .find(|parameter| parameter.name.as_str() == invocation.as_str())
            .and_then(|parameter| type_symbol(program, &parameter.type_reference))
            .and_then(|symbol| services.id_for_symbol(symbol))
            .and_then(|service| services.definition(service))
            .map(|definition| definition.name.clone())
            .or_else(|| {
                service_for_name(&program.symbols, services, invocation.as_str())
                    .and_then(|service| services.definition(service))
                    .map(|definition| definition.name.clone())
            });
        if let Some(service) = service
            && !names.contains(&service)
        {
            names.push(service);
        }
    }
    names
}

fn type_symbol(
    program: &SymbolResolvedTrees,
    type_reference: &psi_symbol_resolved_trees::types::TypeReference,
) -> Option<psi_symbols::SymbolHandle> {
    use psi_symbol_resolved_trees::types::TypeReference;
    match type_reference {
        TypeReference::Reference(reference) => {
            type_symbol(program, program.child_type_reference(reference.referee))
        }
        TypeReference::Constrained(constrained) => {
            type_symbol(program, program.child_type_reference(constrained.base_type))
        }
        TypeReference::Generic(generic) => Some(generic.base_symbol),
        TypeReference::DynamicTrait { symbol, .. }
        | TypeReference::Named { symbol, .. }
        | TypeReference::SelfType { symbol } => Some(*symbol),
        TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::Unit => None,
    }
}

fn normalize_machine_parameter_rows(
    program: &mut SymbolResolvedTrees,
    services: &ServiceReachTable,
    rows: &mut ServiceReachRowTable,
) {
    use psi_symbol_resolved_trees::data::TypeParameterKind;

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

    let service_reach_names = spans
        .iter()
        .flat_map(|span| type_parameters.span_or_empty(*span))
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Machine { contract } => {
                let mut names = program
                    .signature_service_reaches(contract.service_reaches)
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect::<Vec<_>>();
                names.extend(invoked_service_names(
                    program,
                    services,
                    program.signature_invokes(contract.invokes),
                    program.state_parameters(contract.parameters),
                ));
                Some((parameter.symbol, names))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let type_parameters = &mut program.tables.declarations.data_type_parameters;
    for span in spans {
        for parameter in type_parameters.span_mut_or_empty(span) {
            let TypeParameterKind::Machine { contract } = &mut parameter.kind else {
                continue;
            };
            let names = service_reach_names
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
    arena: &psi_arena::Arena<psi_symbol_resolved_trees::data::TypeParameter>,
    span: psi_arena::HandleSpan<psi_symbol_resolved_trees::data::TypeParameter>,
    spans: &mut Vec<psi_arena::HandleSpan<psi_symbol_resolved_trees::data::TypeParameter>>,
) {
    use psi_symbol_resolved_trees::data::TypeParameterKind;
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
