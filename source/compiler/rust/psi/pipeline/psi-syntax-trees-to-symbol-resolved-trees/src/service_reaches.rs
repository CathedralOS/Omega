use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    ServiceReachId, ServiceReachRowId, ServiceReachRowTable, ServiceReachTable,
};
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::name::DiagnosticName;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};
use std::fmt;

#[derive(Debug, Clone)]
pub(crate) struct PendingAuthoredServiceReach {
    pub(crate) keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub(crate) authored: Vec<DiagnosticName>,
}

pub(crate) struct PendingSignatureServiceReach {
    pub(crate) symbol: SymbolHandle,
    pub(crate) owner: crate::lowerer::PendingSignatureOwner,
    pub(crate) keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub(crate) authored: Vec<DiagnosticName>,
}

/// Normalize service reach only after every declaration and trait-parent edge
/// has a resolved symbol. Authored names live only in lowering-private
/// sidecars and are validated before the published resolved trees are
/// returned; normalized rows therefore cannot contain an invalid member or
/// retain a parallel spelling contract.
pub(crate) fn normalize_service_reaches(
    program: &mut SymbolResolvedTrees,
    machine_service_reaches: &[(psi_symbols::SymbolHandle, PendingAuthoredServiceReach)],
    signature_service_reaches: &[PendingSignatureServiceReach],
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
            let pending = machine_service_reaches
                .iter()
                .find(|(symbol, _)| *symbol == machine.symbol)
                .map(|(_, reaches)| reaches)
                .expect("surviving resolved machine retains its pending authored service row");
            validate_machine_service_reaches(
                program,
                &services,
                machine,
                &pending.authored,
                !pending.keyword_source_spans.is_empty(),
            )?;
            let authored = resolve_authored_service_reach(
                &program.symbols,
                &services,
                machine.symbol,
                &pending.keyword_source_spans,
                &pending.authored,
                machine.service_reach_is_installation_bound,
            )?;
            let mut direct = authored
                .as_ref()
                .into_iter()
                .flat_map(|row| &row.targets)
                .filter_map(|target| services.id_for_symbol(target.service))
                .collect::<Vec<_>>();
            let parameters = program
                .machine_state_handles(machine.states)
                .first()
                .map(|state| program.state_parameters(program.machine_state(*state).parameters))
                .unwrap_or_default();
            direct.extend(invoked_service_ids(
                program,
                &services,
                program.machine_invokes(machine),
                parameters,
            ));
            Ok((
                machine.symbol,
                row_for_service_ids(&services, &mut rows, direct),
                authored,
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    let signature_rows = program
        .traits
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition.machines))
        .map(|signature| {
            let pending =
                pending_signature_service_reach(signature_service_reaches, signature.symbol);
            validate_signature_service_reaches(program, &services, signature, pending)?;
            let authored = resolve_authored_service_reach(
                &program.symbols,
                &services,
                signature.symbol,
                &pending.keyword_source_spans,
                &pending.authored,
                signature.service_reach_is_installation_bound,
            )?;
            let mut direct = authored
                .as_ref()
                .into_iter()
                .flat_map(|row| &row.targets)
                .filter_map(|target| services.id_for_symbol(target.service))
                .collect::<Vec<_>>();
            direct.extend(invoked_service_ids(
                program,
                &services,
                program.signature_invokes(signature.invokes),
                program.state_parameters(signature.parameters),
            ));
            Ok((
                signature.symbol,
                row_for_service_ids(&services, &mut rows, direct),
                authored,
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    program.machines.for_each_mut(|machine| {
        machine.service_reach_row = machine_rows
            .iter()
            .find(|(symbol, _, _)| *symbol == machine.symbol)
            .map(|(_, row, _)| *row)
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
                .find(|(symbol, _, _)| *symbol == signature.symbol)
                .map(|(_, row, _)| *row)
                .unwrap_or(ServiceReachRowTable::EMPTY_ROW);
        }
    }

    let parameter_authored_rows =
        normalize_machine_parameter_rows(program, &services, &mut rows, signature_service_reaches)?;

    program.service_reaches = services;
    program.service_reach_rows = rows;
    program.authored_service_reach_rows = machine_rows
        .into_iter()
        .filter_map(|(_, _, row)| row)
        .chain(signature_rows.into_iter().filter_map(|(_, _, row)| row))
        .chain(parameter_authored_rows)
        .collect();
    Ok(())
}

fn pending_signature_service_reach(
    pending: &[PendingSignatureServiceReach],
    symbol: SymbolHandle,
) -> &PendingSignatureServiceReach {
    pending
        .iter()
        .find(|entry| entry.symbol == symbol)
        .expect("resolved state signature retains its pending authored service row")
}

fn validate_signature_service_reaches(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    signature: &psi_symbol_resolved_trees::signature::StateSignature,
    pending: &PendingSignatureServiceReach,
) -> Result<(), Diagnostic> {
    for service in &pending.authored {
        if service_for_name(&program.symbols, services, service.as_str()).is_none() {
            return Err(Diagnostic::error(format!(
                "{} state `{}` declares unknown boundary service `{service}`",
                SignatureOwnerDisplay(&pending.owner),
                signature.name,
            )));
        }
    }
    Ok(())
}

struct SignatureOwnerDisplay<'a>(&'a crate::lowerer::PendingSignatureOwner);

impl fmt::Display for SignatureOwnerDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            crate::lowerer::PendingSignatureOwner::Trait(name) => {
                write!(formatter, "trait `{name}`")
            }
            crate::lowerer::PendingSignatureOwner::Requirement(name) => {
                write!(formatter, "machine-parameter requirement `{name}`")
            }
        }
    }
}

fn validate_machine_service_reaches(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    machine: &psi_symbol_resolved_trees::machine::Machine,
    authored: &[DiagnosticName],
    is_authored: bool,
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
    ) && is_authored
    {
        return Err(Diagnostic::error(format!(
            "external leaf `{}` repeats an authored `reaches` row, but `via` derives behavior from the satisfied requirement and admitted binding; remove the leaf's `reaches` clause",
            machine.name,
        )));
    }

    Ok(())
}

fn resolve_authored_service_reach(
    symbols: &SymbolTable,
    services: &ServiceReachTable,
    owner: SymbolHandle,
    keyword_source_spans: &[psi_source::SourceSpan],
    authored: &[DiagnosticName],
    installation_bound: bool,
) -> Result<Option<psi_symbol_resolved_trees::signature::AuthoredServiceReachRow>, Diagnostic> {
    if keyword_source_spans.is_empty() {
        if authored.is_empty() {
            return Ok(None);
        }
        return Err(Diagnostic::error(
            "authored service-reach members have no retained `reaches` clause occurrence",
        ));
    }

    let targets = authored
        .iter()
        .map(|name| {
            let service = service_for_name(symbols, services, name.as_str()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "authored service-reach member `{name}` lost its exact boundary-service identity"
                ))
            })?;
            let definition = services.definition(service).ok_or_else(|| {
                Diagnostic::error(format!(
                    "authored service-reach member `{name}` resolves outside the normalized service table"
                ))
            })?;
            Ok(psi_symbol_resolved_trees::signature::AuthoredServiceReachTarget {
                service: definition.symbol,
                source_span: name.source_span(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(Some(
        psi_symbol_resolved_trees::signature::AuthoredServiceReachRow {
            owner,
            keyword_source_spans: keyword_source_spans.to_vec(),
            targets,
            installation_bound,
        },
    ))
}

fn invoked_service_ids(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    invokes: &[psi_symbol_resolved_trees::name::DiagnosticName],
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
) -> Vec<ServiceReachId> {
    let mut service_ids = Vec::new();
    for invocation in invokes {
        let service = parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .find(|parameter| parameter.name.as_str() == invocation.as_str())
            .and_then(|parameter| type_symbol(program, &parameter.type_reference))
            .and_then(|symbol| services.id_for_symbol(symbol))
            .and_then(|service| services.definition(service).map(|_| service))
            .or_else(|| service_for_name(&program.symbols, services, invocation.as_str()));
        if let Some(service) = service
            && !service_ids.contains(&service)
        {
            service_ids.push(service);
        }
    }
    service_ids
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
    signature_service_reaches: &[PendingSignatureServiceReach],
) -> Result<Vec<psi_symbol_resolved_trees::signature::AuthoredServiceReachRow>, Diagnostic> {
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

    let mut resolved_rows = Vec::new();
    for parameter in spans
        .iter()
        .flat_map(|span| type_parameters.span_or_empty(*span))
    {
        let TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        let Some(contract) = contract.structural() else {
            continue;
        };
        let pending = pending_signature_service_reach(signature_service_reaches, contract.symbol);
        validate_signature_service_reaches(program, services, contract, pending)?;
        let authored = resolve_authored_service_reach(
            &program.symbols,
            services,
            contract.symbol,
            &pending.keyword_source_spans,
            &pending.authored,
            contract.service_reach_is_installation_bound,
        )?;
        let mut direct = authored
            .as_ref()
            .into_iter()
            .flat_map(|row| &row.targets)
            .filter_map(|target| services.id_for_symbol(target.service))
            .collect::<Vec<_>>();
        direct.extend(invoked_service_ids(
            program,
            services,
            program.signature_invokes(contract.invokes),
            program.state_parameters(contract.parameters),
        ));
        resolved_rows.push((
            parameter.symbol,
            row_for_service_ids(services, rows, direct),
            authored,
        ));
    }

    let type_parameters = &mut program.tables.declarations.data_type_parameters;
    for span in spans {
        for parameter in type_parameters.span_mut_or_empty(span) {
            let TypeParameterKind::Machine { contract } = &mut parameter.kind else {
                continue;
            };
            let Some(contract) = contract.structural_mut() else {
                continue;
            };
            let row = resolved_rows
                .iter()
                .find(|(symbol, _, _)| *symbol == parameter.symbol)
                .map(|(_, row, _)| *row)
                .unwrap_or(ServiceReachRowTable::EMPTY_ROW);
            contract.service_reach_row = row;
        }
    }
    Ok(resolved_rows
        .into_iter()
        .filter_map(|(_, _, authored)| authored)
        .collect())
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
            if let Some(contract) = contract.structural() {
                collect_parameter_spans(arena, contract.type_parameters, spans);
            }
        }
    }
}

fn row_for_service_ids(
    services: &ServiceReachTable,
    rows: &mut ServiceReachRowTable,
    service_ids: impl IntoIterator<Item = ServiceReachId>,
) -> ServiceReachRowId {
    let mut members = Vec::new();
    for service in service_ids {
        services.extend_closure(service, &mut members);
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
