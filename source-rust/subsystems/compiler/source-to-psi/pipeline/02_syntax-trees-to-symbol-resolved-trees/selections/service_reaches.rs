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
use crate::source_symbols::SymbolTable;
use crate::source_tables::{ServiceReachRowTable, ServiceReachTable};
use diagnostics::Diagnostic;
use semantic_vocabulary::{ServiceReachId, ServiceReachRowId};
use std::fmt;
use symbols::{SymbolHandle, SymbolKind};

#[derive(Debug, Clone)]
pub(crate) enum PendingAuthoredServiceReach {
    Authored {
        keyword_source_spans: Vec<source_text::SourceSpan>,
        authored: Vec<DiagnosticName>,
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
        symbols: &SymbolTable,
        services: &ServiceReachTable,
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
                    symbols,
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
                    .filter_map(|target| services.id_for_symbol(target.service));
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
                    .iter()
                    .map(|requirement| requirement.symbol)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    boundary_traits.sort_by(|left, right| left.1.cmp(&right.1));

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
            &program.symbols,
            &services,
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
                &program.symbols,
                &services,
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

    let parameter_authored_rows =
        normalize_machine_parameter_rows(program, &services, &mut rows, signature_service_reaches)?;

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
        if service_for_name(&program.symbols, services, service).is_none() {
            return Err(Diagnostic::error(format!(
                "{} state `{}` declares unknown boundary service `{service}`",
                SignatureOwnerDisplay(&pending.owner),
                signature.name,
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
    authored: &[DiagnosticName],
    is_authored: bool,
) -> Result<(), Diagnostic> {
    for service in authored {
        if service_for_name(&program.symbols, services, service).is_none() {
            return Err(Diagnostic::error(format!(
                "machine `{}` declares unknown boundary service `{service}`",
                machine.name,
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
    symbols: &SymbolTable,
    services: &ServiceReachTable,
    owner: SymbolHandle,
    keyword_source_spans: &[source_text::SourceSpan],
    authored: &[DiagnosticName],
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

    let targets = authored
    .iter()
    .map(|name| {
        let service = service_for_name(symbols, services, name).ok_or_else(|| {
            Diagnostic::error(format!(
                "authored service-reach member `{name}` lost its exact boundary-service identity"
            ))
        })?;
        let definition = services.definition(service).ok_or_else(|| {
            Diagnostic::error(format!(
                "authored service-reach member `{name}` resolves outside the normalized service table"
            ))
        })?;
        Ok(AuthoredServiceReachTarget {
            service: definition.symbol,
            source_span: name.source_span(),
        })
    })
    .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(Some(AuthoredServiceReachRow {
        owner,
        keyword_source_spans: keyword_source_spans.to_vec(),
        targets,
        installation_bound,
    }))
}

fn normalize_machine_parameter_rows(
    program: &mut SymbolResolvedTrees,
    services: &ServiceReachTable,
    rows: &mut ServiceReachRowTable,
    signature_service_reaches: &[PendingSignatureServiceReach],
) -> Result<Vec<AuthoredServiceReachRow>, Diagnostic> {
    use crate::source_resolved::data::TypeParameterKind;

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
            &program.symbols,
            services,
            rows,
            contract.symbol,
            contract.service_reach_is_installation_bound,
        )?;
        resolved_rows.push((parameter.symbol, row, authored));
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
    use crate::frontend_fixture::{Lexer, parse_syntax_trees};
    use crate::resolution::{ResolutionRequest, resolve};
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
        let tokens = Lexer::new(text).tokenize().unwrap();
        let trees = resolve(ResolutionRequest::new(parse_syntax_trees(&tokens).unwrap())).unwrap();
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
                trees.service_reaches.definition(expected).unwrap().symbol
            );
        }
    }
}
