/*
    Service reaches: authored `reaches` names bound to exact boundary-trait
    identities.

    Names live in lowering-private sidecars until every declaration and
    trait-parent edge has a symbol; published rows cannot carry an invalid
    member or a parallel spelling contract.
    Invocation permissions are a separate axis. Do not insert provider-family
    names into these rows; checking derives actual calls and exact callback
    contributions after the callable contracts have been resolved.
*/

use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::data::TypeParameter;
use crate::source_resolved::machine::Machine;
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::signature::AuthoredServiceReachRow;
use crate::source_resolved::signature::AuthoredServiceReachTarget;
use crate::source_resolved::signature::StateSignature;
use crate::source_resolved::trait_definition::TraitRequirement;
use crate::source_symbols::SymbolTable;
use crate::source_tables::{ServiceReachApplication, ServiceReachRowTable};
type ServiceReachTable = crate::source_tables::ServiceReachTable<TraitRequirement>;
use diagnostics::Diagnostic;
use semantic_vocabulary::{ServiceReachId, ServiceReachRowId};
use std::fmt;
use symbols::{SymbolHandle, SymbolKind};

#[derive(Debug, Clone)]
pub(crate) enum PendingAuthoredServiceReach {    Authored {
        keyword_source_spans: Vec<source_text::SourceSpan>,
        authored: Vec<TraitRequirement>,
    },
    /*
        Exact output from this seeded resolver's predecessor. A positional
        service row and its authored ceiling travel together; no name lookup
        reconstructs their meaning under a later consumer's source selection.
    */
    Resolved {
        row: ServiceReachRowId,
        authored: Option<AuthoredServiceReachRow>,
    },
}

impl PendingAuthoredServiceReach {
    fn resolve(
        &self,
        program: &SymbolResolvedTrees,
        services: &mut ServiceReachTable,
        rows: &mut ServiceReachRowTable,
        owner: SymbolHandle,
        installation_bound: bool,
    ) -> Result<(ServiceReachRowId, Option<AuthoredServiceReachRow>), Diagnostic> {
        match self {
            Self::Resolved { row, authored } => {
                if authored.as_ref().is_some_and(|authored| {
                    authored.owner != owner || authored.installation_bound != installation_bound
                }) {
                    return Err(Diagnostic::error(
                        "retained service reach changed its exact owner or installation mode",
                    ));
                }
                Ok((*row, authored.clone()))
            }
            Self::Authored {
                keyword_source_spans,
                authored,
            } => {
                let authored = resolve_authored_service_reach(
                    program,
                    services,
                    owner,
                    keyword_source_spans,
                    authored,
                    installation_bound,
                )?;
                let direct = authored
                    .as_ref()
                    .into_iter()
                    .flat_map(|row| &row.targets)
                    .map(|target| target.service);
                Ok((row_for_service_ids(services, rows, direct), authored))
            }
        }
    }
}

pub(crate) struct PendingSignatureServiceReach {
    pub(crate) symbol: SymbolHandle,
    pub(crate) owner: crate::resolution::lowering::lowerer::PendingSignatureOwner,
    pub(crate) reaches: PendingAuthoredServiceReach,
}

/*
    Normalize service reach only after every declaration and trait-parent edge
    has a resolved symbol. Authored names live only in lowering-private
    sidecars and are validated before the published resolved trees are
    returned; normalized rows therefore cannot contain an invalid member or
    retain a parallel spelling contract.
*/
pub(crate) fn normalize_service_reaches(
    program: &mut SymbolResolvedTrees,
    machine_ordinals: &[Option<usize>],
    machine_service_reaches: &[PendingAuthoredServiceReach],
    signature_service_reaches: &[PendingSignatureServiceReach],
    predecessor: Option<&SymbolResolvedTrees>,
) -> Result<(), Diagnostic> {
    let (mut services, mut rows) = predecessor
        .map(|base| {
            (
                base.service_reaches.clone(),
                base.service_reach_rows.clone(),
            )
        })
        .unwrap_or_default();
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
                    .to_vec(),
            )
        })
        .collect::<Vec<_>>();
    boundary_traits.sort_by(|left, right| left.1.cmp(&right.1));

    for (symbol, name, _) in &boundary_traits {
        services.intern(*symbol, name);
    }
    for (symbol, _, declared_parents) in &boundary_traits {
        let service = services
            .id_for_symbol(*symbol)
            .expect("registered boundary trait");
        let mut parents = Vec::new();
        for parent in declared_parents {
            if let Some(base) = services.id_for_symbol(parent.symbol) {
                parents.push(resolve_application(
                    program,
                    &mut services,
                    *symbol,
                    parent,
                    base,
                )?);
            }
        }
        services.set_parents(service, parents);
    }

    rows.intern(Vec::new());

    // Lowering records one pending reach per original machine. Conformance
    // selection keeps their order and supplies the final ordinal of each survivor.
    assert_eq!(machine_service_reaches.len(), machine_ordinals.len());
    let mut authored_machine_rows = Vec::new();
    for (&ordinal, pending) in machine_ordinals.iter().zip(machine_service_reaches) {
        let Some(ordinal) = ordinal else { continue };
        let machine = &program.machines[ordinal];
        if let PendingAuthoredServiceReach::Authored {
            keyword_source_spans,
            authored,
        } = pending
        {
            validate_machine_service_reaches(
                program,
                &services,
                machine,
                authored,
                !keyword_source_spans.is_empty(),
            )?;
        }
        let (row, authored) = pending.resolve(
            program,
            &mut services,
            &mut rows,
            machine.symbol,
            machine.service_reach_is_installation_bound,
        )?;
        if machine.service_reach_row != row {
            program.machines[ordinal].service_reach_row = row;
        }
        if let Some(authored) = authored {
            authored_machine_rows.push(authored);
        }
    }

    let signature_rows = program
        .traits
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition.machines))
        .chain(
            program
                .tables
                .declarations
                .runtime_callable_contracts
                .iter()
                .map(|(_, contract)| &contract.signature),
        )
        .map(|signature| {
            let pending =
                pending_signature_service_reach(signature_service_reaches, signature.symbol)?;
            validate_signature_service_reaches(program, &services, signature, pending)?;
            let (row, authored) = pending.reaches.resolve(
                program,
                &mut services,
                &mut rows,
                signature.symbol,
                signature.service_reach_is_installation_bound,
            )?;
            Ok((signature.symbol, row, authored))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    let signature_spans = program
        .traits
        .iter()
        .map(|definition| definition.machines)
        .collect::<Vec<_>>();
    let mut prepared_rows = signature_rows.iter();
    let mut apply_signature_row = |signature: &mut StateSignature| -> Result<(), Diagnostic> {
        let (symbol, row, _) = prepared_rows.next().ok_or_else(|| {
            Diagnostic::error("callable signature has no prepared normalized service-reach row")
        })?;
        if *symbol != signature.symbol {
            return Err(Diagnostic::error(
                "normalized service-reach row does not match its exact callable signature",
            ));
        }
        signature.service_reach_row = *row;
        Ok(())
    };
    let signature_arena = &mut program.tables.declarations.trait_machine_signatures;
    for span in signature_spans {
        for signature in signature_arena.span_mut_or_empty(span) {
            apply_signature_row(signature)?;
        }
    }

    let runtime_contracts = program
        .tables
        .declarations
        .runtime_callable_contracts
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in runtime_contracts {
        let contract = program
            .tables
            .declarations
            .runtime_callable_contracts
            .get_mut(handle);
        apply_signature_row(&mut contract.signature)?;
    }
    if prepared_rows.next().is_some() {
        return Err(Diagnostic::error(
            "normalized service-reach rows outnumber their exact callable signatures",
        ));
    }

    let parameter_spans = machine_parameter_spans(program);
    let parameter_authored_rows = normalize_machine_parameter_rows(
        program,
        &parameter_spans,
        &mut services,
        &mut rows,
        signature_service_reaches,
    )?;

    // A refinement is a view of an existing requirement. Its wildcard has no
    // service identity; concrete clauses use the same applications as a signature.
    let mut refined_rows = Vec::new();
    for declaration in program.traits.iter() {
        for clause in &declaration.refinement_clauses {
            let mut direct = Vec::new();
            for reference in &clause.service_reaches {
                if reference.name.as_str() == "_" {
                    continue;
                }
                let service = service_for_name(&program.symbols, &services, &reference.name)
                    .ok_or_else(|| Diagnostic::error(format!(
                        "refinement clause `reaches` names `{}`, which is not a boundary service", reference.name
                    )).with_source_span(reference.name.source_span()))?;
                direct.push(resolve_application(
                    program,
                    &mut services,
                    clause.signature.symbol,
                    reference,
                    service,
                )?);
            }
            refined_rows.push(row_for_service_ids(&services, &mut rows, direct));
        }
    }
    let mut refined_rows = refined_rows.into_iter();
    program.roots.traits.for_each_mut(|declaration| {
        for clause in &mut declaration.refinement_clauses {
            clause.signature.service_reach_row =
                refined_rows.next().expect("one row per refinement clause");
        }
    });

    bind_invocation_applications(program, &mut services, &parameter_spans)?;
    program.service_reaches = services;
    program.service_reach_rows = rows;
    program.authored_service_reach_rows = authored_machine_rows
        .into_iter()
        .chain(signature_rows.into_iter().filter_map(|(_, _, row)| row))
        .chain(parameter_authored_rows)
        .collect();
    Ok(())
}

fn pending_signature_service_reach(
    pending: &[PendingSignatureServiceReach],
    symbol: SymbolHandle,
) -> Result<&PendingSignatureServiceReach, Diagnostic> {
    pending
        .iter()
        .find(|entry| entry.symbol == symbol)
        .ok_or_else(|| {
            Diagnostic::error(
                "resolved callable signature has no exact pending authored service-reach row",
            )
        })
}

fn validate_signature_service_reaches(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    signature: &StateSignature,
    pending: &PendingSignatureServiceReach,
) -> Result<(), Diagnostic> {
    let PendingAuthoredServiceReach::Authored { authored, .. } = &pending.reaches else {
        return Ok(());
    };
    for service in authored {
        if service_for_name(&program.symbols, services, &service.name).is_none() {
            return Err(Diagnostic::error(format!(
                "{} state `{}` declares unknown boundary service `{}`",
                SignatureOwnerDisplay(&pending.owner),
                signature.name, service.name,
            )));
        }
    }
    Ok(())
}

struct SignatureOwnerDisplay<'a>(&'a crate::resolution::lowering::lowerer::PendingSignatureOwner);

impl fmt::Display for SignatureOwnerDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            crate::resolution::lowering::lowerer::PendingSignatureOwner::Trait(name) => {
                write!(formatter, "trait `{name}`")
            }
            crate::resolution::lowering::lowerer::PendingSignatureOwner::Requirement(name) => {
                write!(formatter, "machine-parameter requirement `{name}`")
            }
        }
    }
}

fn validate_machine_service_reaches(
    program: &SymbolResolvedTrees,
    services: &ServiceReachTable,
    machine: &Machine,
    authored: &[TraitRequirement],
    is_authored: bool,
) -> Result<(), Diagnostic> {
    for service in authored {
        if service_for_name(&program.symbols, services, &service.name).is_none() {
            return Err(Diagnostic::error(format!(
                "machine `{}` declares unknown boundary service `{}`",
                machine.name, service.name,
            )));
        }
    }

    if matches!(
        machine.supply_mode,
        source_declarations::MachineSupplyMode::ExternalRealization { .. }
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
    program: &SymbolResolvedTrees,
    services: &mut ServiceReachTable,
    owner: SymbolHandle,
    keyword_source_spans: &[source_text::SourceSpan],
    authored: &[TraitRequirement],
    installation_bound: bool,
) -> Result<Option<AuthoredServiceReachRow>, Diagnostic> {
    if keyword_source_spans.is_empty() {
        if authored.is_empty() {
            return Ok(None);
        }
        return Err(Diagnostic::error(
            "authored service-reach members have no retained `reaches` clause occurrence",
        ));
    }

    let targets = authored.iter().map(|reference| {
        let name = &reference.name;
        let service = service_for_name(&program.symbols, services, name).ok_or_else(||
            Diagnostic::error(format!("authored service-reach member `{name}` lost its exact boundary-service identity")))?;
        let service = resolve_application(program, services, owner, reference, service)?;
        Ok(AuthoredServiceReachTarget { service, source_span: name.source_span() })
    }).collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(Some(AuthoredServiceReachRow {
        owner,
        keyword_source_spans: keyword_source_spans.to_vec(),
        targets,
        installation_bound,
    }))
}

fn resolve_application(
    program: &SymbolResolvedTrees,
    services: &mut ServiceReachTable,
    owner: SymbolHandle,
    reference: &TraitRequirement,
    service: ServiceReachId,
) -> Result<ServiceReachId, Diagnostic> {
    let declaration = services
        .definition(service)
        .and_then(|row| program.trait_definition(row.symbol))
        .ok_or_else(|| {
            Diagnostic::error("an effect reference has no exact boundary declaration")
        })?;
    let generic =
        !declaration.type_parameters.is_empty() || !declaration.lifetime_parameters.is_empty();
    let applied = !reference.arguments.is_empty() || !reference.lifetime_arguments.is_empty();
    if applied && !generic {
        return Err(Diagnostic::error(format!(
            "boundary service `{}` has no static or lifetime parameters",
            reference.name
        ))
        .with_source_span(reference.name.source_span()));
    }
    if !applied {
        return Ok(service);
    }
    let mut arguments = reference.clone();
    arguments.symbol = declaration.symbol;
    Ok(services.intern_application(
        declaration.symbol,
        declaration.name.as_str(),
        Some(ServiceReachApplication {
            lexical_owner: owner,
            arguments,
        }),
    ))
}

/// The same declaration/application lookup serves both axes. An invocation
/// that names a parameter remains an entry in the lexical parameter scope.
fn bind_invocation_applications(
    program: &mut SymbolResolvedTrees,
    services: &mut ServiceReachTable,
    parameter_spans: &[arena::HandleSpan<TypeParameter>],
) -> Result<(), Diagnostic> {
    use crate::source_resolved::signature::InvocationReference;
    let static_contracts = parameter_spans
        .iter()
        .flat_map(|span| {
            program
                .tables
                .declarations
                .data_type_parameters
                .span_or_empty(*span)
        })
        .filter_map(|parameter| match &parameter.kind {
            crate::source_resolved::data::TypeParameterKind::Machine { contract } => {
                contract.structural()
            }
            _ => None,
        });
    let signatures = program
        .machines
        .iter()
        .filter_map(|machine| {
            let entry =
                program.machine_state(*program.machine_state_handles(machine.states).first()?);
            Some((
                machine.symbol,
                machine.invokes,
                program.state_parameters(entry.parameters),
            ))
        })
        .chain(
            program
                .traits
                .iter()
                .flat_map(|declaration| program.trait_machine_signatures(declaration.machines))
                .chain(
                    program
                        .tables
                        .declarations
                        .runtime_callable_contracts
                        .iter()
                        .map(|(_, contract)| &contract.signature),
                )
                .chain(static_contracts)
                .map(|signature| {
                    (
                        signature.symbol,
                        signature.invokes,
                        program.state_parameters(signature.parameters),
                    )
                }),
        );
    let mut selected = Vec::new();
    for (owner, span, parameters) in signatures {
        for (handle, reference) in span.handles().zip(program.signature_invokes(span)) {
            let InvocationReference::Name(reference) = reference else {
                continue;
            };
            let name = &reference.name;
            let applied =
                !reference.arguments.is_empty() || !reference.lifetime_arguments.is_empty();
            let is_parameter = parameters
                .iter()
                .any(|parameter| parameter.name.as_str() == name.as_str());
            let service = if is_parameter {
                None
            } else {
                service_for_name(&program.symbols, services, name)
            };
            if let Some(service) = service {
                let service = resolve_application(program, services, owner, reference, service)?;
                selected.push((
                    handle,
                    InvocationReference::Service {
                        name: name.clone(),
                        service,
                    },
                ));
            } else if applied {
                return Err(Diagnostic::error(format!(
                    "invocation target `{name}` takes arguments only when it names a boundary service"
                )).with_source_span(name.source_span()));
            }
        }
    }
    for (handle, reference) in selected {
        *program
            .tables
            .declarations
            .signature_invokes
            .get_mut(handle) = reference;
    }
    Ok(())
}

fn machine_parameter_spans(program: &SymbolResolvedTrees) -> Vec<arena::HandleSpan<TypeParameter>> {
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

    roots.extend(
        program
            .tables
            .declarations
            .runtime_callable_contracts
            .iter()
            .map(|(_, contract)| contract.signature.type_parameters),
    );

    let type_parameters = &program.tables.declarations.data_type_parameters;
    let mut spans = Vec::new();
    for root in roots {
        collect_parameter_spans(type_parameters, root, &mut spans);
    }
    spans
}

fn normalize_machine_parameter_rows(
    program: &mut SymbolResolvedTrees,
    spans: &[arena::HandleSpan<TypeParameter>],
    services: &mut ServiceReachTable,
    rows: &mut ServiceReachRowTable,
    signature_service_reaches: &[PendingSignatureServiceReach],
) -> Result<Vec<AuthoredServiceReachRow>, Diagnostic> {
    use crate::source_resolved::data::TypeParameterKind;

    let type_parameters = &program.tables.declarations.data_type_parameters;
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
        let pending = pending_signature_service_reach(signature_service_reaches, contract.symbol)?;
        validate_signature_service_reaches(program, services, contract, pending)?;
        let (row, authored) = pending.reaches.resolve(
            program,
            services,
            rows,
            contract.symbol,
            contract.service_reach_is_installation_bound,
        )?;
        resolved_rows.push((parameter.symbol, row, authored));
    }

    let type_parameters = &mut program.tables.declarations.data_type_parameters;
    for &span in spans {
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
    arena: &arena::Arena<TypeParameter>,
    span: arena::HandleSpan<TypeParameter>,
    spans: &mut Vec<arena::HandleSpan<TypeParameter>>,
) {
    use crate::source_resolved::data::TypeParameterKind;
    if span.is_empty() || spans.contains(&span) {
        return;
    }
    spans.push(span);
    for parameter in arena.span_or_empty(span) {
        if let TypeParameterKind::Machine { contract } = &parameter.kind
            && let Some(contract) = contract.structural()
        {
            collect_parameter_spans(arena, contract.type_parameters, spans);
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
    name: &DiagnosticName,
) -> Option<ServiceReachId> {
    let symbol = symbols.find_top_level_by_name_and_kinds_from_source(
        name.as_str(),
        &[SymbolKind::Trait],
        name.source_span(),
    )?;
    services.id_for_symbol(symbol)
}

#[cfg(test)]
mod tests {
    use crate::resolution::resolve;
    use crate::source_resolved::trait_definition::ConformanceRowSource;

    #[test]
    fn overriding_one_default_preserves_the_next_machines_reach() {
        // Both defaults are synthesized. The explicit read removes the first
        // default and the inherited write moves into its place.
        let text = r#"
            public boundary trait Input {}
            public boundary trait Output {}
            trait Device {
                machine read() -> u64 reaches Input { 1u64 }
                machine write() -> u64 reaches Output { 2u64 }
            }
            Driver satisfies Device {
                machine read() -> u64 reaches Input { 3u64 }
            }
        "#;
        let trees = resolve(crate::resolution::tests::source_request(text)).unwrap();
        let driver = trees.conformances.iter().next().unwrap();
        assert_eq!(driver.rows.len(), 2);
        for (requirement, source, service) in [
            ("read", ConformanceRowSource::Inline, "Input"),
            ("write", ConformanceRowSource::TraitDefault, "Output"),
        ] {
            let row = driver
                .rows
                .iter()
                .find(|row| row.requirement_name.as_str() == requirement)
                .unwrap();
            assert_eq!(row.source, source);
            let machine = trees.machine(row.realization_machine).unwrap();
            let expected = trees.service_reaches.id_for_name(service).unwrap();
            assert_eq!(
                trees.service_reach_rows.services(machine.service_reach_row),
                &[expected]
            );
            let authored = trees
                .authored_service_reach_rows_for(machine.symbol)
                .collect::<Vec<_>>();
            assert_eq!(authored.len(), 1);
            assert_eq!(
                authored[0].targets[0].service,
                expected
            );
        }
    }
}
