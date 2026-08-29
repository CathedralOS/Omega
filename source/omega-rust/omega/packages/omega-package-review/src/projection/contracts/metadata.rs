use super::super::evidence::*;
use super::super::exact_identity::*;
use super::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) struct ContractProjectionContext<'a> {
    pub(crate) subject_kind: &'static str,
    pub(crate) subject_name: &'a str,
    pub(crate) owner: psi_checked_trees::ContractProofFactOwner,
    pub(crate) point: psi_facts::ProgramPoint,
    pub(crate) parameters: &'a [psi_typed_trees::signature::StateParameter],
    pub(crate) domain_symbol: Option<SymbolHandle>,
    pub(crate) data_symbol: Option<SymbolHandle>,
    pub(crate) lifetime_binders: &'a [psi_typed_trees::name::Identifier],
}

pub(crate) fn project_callable_contracts(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    entry: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    let parameters = compilation.state_parameters(entry);
    let context = ContractProjectionContext {
        subject_kind: "callable",
        subject_name: machine.name.as_str(),
        owner: psi_checked_trees::ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        point: psi_facts::ProgramPoint::Machine {
            machine_symbol: machine.symbol,
        },
        parameters,
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders: &machine.lifetime_parameters,
    };
    project_contracts(
        compilation,
        compilation.machine_contracts(machine),
        &context,
        binders,
    )
}

pub(crate) fn project_trait_requirement_contracts(
    compilation: &CheckedCompilation,
    requirement: &psi_typed_trees::signature::StateSignature,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    project_contracts(
        compilation,
        compilation.state_signature_contracts(requirement),
        context,
        binders,
    )
}

pub(crate) fn project_contracts(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    use psi_typed_trees::{domain::ProofFact, signature::SignatureContractKind};

    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "contract review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for contract in contracts {
        let (kind, guarded_symbols, result_case) = match contract.kind {
            SignatureContractKind::Requires => (PackageReviewContractKind::Requires, None, None),
            SignatureContractKind::Ensures => (PackageReviewContractKind::Ensures, None, None),
            SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } => (
                PackageReviewContractKind::Ensures,
                Some((result_data, result_case)),
                Some(PackageReviewResultCaseIdentity {
                    result_data: nominal_identity(compilation, result_data)?,
                    result_case: nominal_identity(compilation, result_case)?,
                }),
            ),
            SignatureContractKind::Crashes { .. } => continue,
        };
        if contract.facts.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an empty public {:?} contract",
                context.subject_kind, context.subject_name, kind
            ))]);
        }
        for offset in 0..contract.facts.count() {
            let fact_handle = psi_arena::Handle::from_parts(
                contract
                    .facts
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("proof fact handle index overflow"),
                contract.facts.start().generation(),
            );
            let fact = match compilation.proof_facts.get(fact_handle) {
                ProofFact::Expression(expression) => {
                    PackageReviewContractFact::Expression(project_contract_expression(
                        compilation,
                        context,
                        binders,
                        *expression,
                        Some(fact_handle),
                        0,
                    )?)
                }
                ProofFact::Membership(membership) => {
                    let domain = compilation
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(format!(
                                "reviewed {} `{}` contract refers to an unresolved domain",
                                context.subject_kind, context.subject_name
                            ))]
                        })?;
                    let domain_identity = nominal_identity(compilation, domain.symbol)?;
                    if reviewed_package_owns(&domain_identity, reviewed_package)?
                        && !domain.is_public
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "reviewed {} `{}` exposes non-public domain `{}` in its contract",
                            context.subject_kind, context.subject_name, domain.name
                        ))]);
                    }
                    PackageReviewContractFact::Membership {
                        value: project_contract_expression(
                            compilation,
                            context,
                            binders,
                            membership.value,
                            Some(fact_handle),
                            0,
                        )?,
                        domain: domain_identity,
                    }
                }
                ProofFact::Proposition(application) => project_contract_proposition(
                    compilation,
                    context,
                    binders,
                    application,
                    Some(fact_handle),
                    &[],
                    &[],
                    &mut Vec::new(),
                    0,
                )?,
            };
            let evidence_lane_position = if let Some((result_data, result_case)) = guarded_symbols {
                let checked = checked_outcome_specific_guarantee(
                    compilation,
                    context,
                    fact_handle,
                    result_data,
                    result_case,
                    contract.binding.as_ref(),
                )?;
                validate_checked_contract_evidence_components(
                    compilation,
                    context,
                    contract.binding.as_ref(),
                    psi_checked_trees::ContractProofFactOwner::Machine {
                        machine_symbol: checked.machine_symbol,
                    },
                    psi_checked_trees::ContractProofFactKind::Ensures,
                    checked.evidence_term,
                    &fact,
                )?
            } else {
                let checked = checked_contract_fact(compilation, context, fact_handle, kind)?;
                validate_checked_contract_evidence(
                    compilation,
                    context,
                    contract.binding.as_ref(),
                    checked,
                    &fact,
                )?
            };
            projected.push(PackageReviewCallableContract {
                kind,
                result_case: result_case.clone(),
                binding: match kind {
                    PackageReviewContractKind::Ensures => contract
                        .binding
                        .as_ref()
                        .map(|binding| binding.as_str().to_owned()),
                    PackageReviewContractKind::Requires => None,
                },
                evidence_lane_position,
                fact,
            });
        }
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

pub(crate) fn proof_fact_handle(
    facts: psi_arena::HandleSpan<psi_typed_trees::domain::ProofFact>,
    offset: u32,
) -> psi_arena::Handle<psi_typed_trees::domain::ProofFact> {
    psi_arena::Handle::from_parts(
        facts
            .start()
            .arena_index()
            .checked_add(offset)
            .expect("proof fact handle index overflow"),
        facts.start().generation(),
    )
}

pub(crate) fn project_required_proof_fact_source_locations(
    compilation: &CheckedCompilation,
    facts: psi_arena::HandleSpan<psi_typed_trees::domain::ProofFact>,
    subject: &str,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = Vec::with_capacity(facts.len());
    for offset in 0..facts.count() {
        let source_span = compilation
            .proof_fact_source_span(proof_fact_handle(facts, offset))
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "{subject} fact has no exact authored source custody"
                ))]
            })?;
        locations.push(ProjectedNestedSourceLocation {
            source_span,
            role: PackageReviewSourceLocationRole::ProofFact,
        });
    }
    Ok(locations)
}

pub(crate) fn project_contract_source_locations(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = Vec::new();
    for contract in contracts {
        if let Some(source_span) = contract.keyword_source_span {
            locations.push(ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::ContractClause,
            });
        }
        for offset in 0..contract.facts.count() {
            let fact = proof_fact_handle(contract.facts, offset);
            match compilation.proof_fact_source_span(fact) {
                Some(source_span) => locations.push(ProjectedNestedSourceLocation {
                    source_span,
                    role: PackageReviewSourceLocationRole::ProofFact,
                }),
                None if contract.keyword_source_span.is_some() => {
                    return Err(vec![Diagnostic::error(
                        "authored package-review contract fact has no exact source custody",
                    )]);
                }
                None => {}
            }
        }
    }
    Ok(locations)
}

pub(crate) fn project_machine_service_reach_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        machine.symbol,
        machine.name.as_str(),
        machine.service_reach_is_installation_bound,
    )?;
    let parameters = compilation
        .machine_states(machine)
        .first()
        .map(|state| compilation.state_parameters(state))
        .unwrap_or_default();
    let declared = derive_declared_service_reach(
        compilation,
        authored,
        &psi_effects::declared_machine_invocations(compilation, machine),
        parameters,
        machine.name.as_str(),
    )?;
    if compilation
        .service_reach_rows
        .services(machine.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored reaches/invokes targets do not equal its exact normalized service-reach row",
            machine.name,
        ))]);
    }

    let checked = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "service-reach",
    )?;
    let should_publish = machine.supply_mode
        != psi_language_semantics::MachineSupplyMode::CheckedBody
        || machine.is_public
        || authored.is_some()
        || !declared.is_empty();
    let expected_interface = if should_publish {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(machine.service_reach_row)
    } else {
        psi_language_semantics::ServiceReachInterface::InternalInferred
    };
    if checked.interface != expected_interface
        || checked.published_ceiling != machine.service_reach_row
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored service-reach custody does not equal its exact checked service-reach fact",
            machine.name,
        ))]);
    }

    Ok(authored_service_reach_locations(authored))
}

pub(crate) fn project_signature_service_reach_source_locations(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        signature.symbol,
        signature.name.as_str(),
        signature.service_reach_is_installation_bound,
    )?;
    let declared = derive_declared_service_reach(
        compilation,
        authored,
        &psi_effects::declared_signature_invocations(compilation, signature),
        compilation.state_signature_parameters(signature),
        signature.name.as_str(),
    )?;
    if compilation
        .service_reach_rows
        .services(signature.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored reaches/invokes targets do not equal its exact normalized service-reach row",
            signature.name,
        ))]);
    }

    let checked = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        signature.name.as_str(),
        "signature contract capsule",
    )?;
    let mut checked_published = checked.published_service_reach().to_vec();
    checked_published.sort();
    checked_published.dedup();
    let mut declared_names = declared
        .iter()
        .map(|service| {
            compilation
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.clone())
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed signature `{}` has a normalized service outside its exact declaration table",
                        signature.name,
                    ))]
                })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    declared_names.sort();
    declared_names.dedup();
    if checked_published != declared_names {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored service-reach custody does not equal its exact checked contract capsule",
            signature.name,
        ))]);
    }

    Ok(authored_service_reach_locations(authored))
}

pub(crate) fn exact_authored_service_reach_row<'a>(
    compilation: &'a CheckedCompilation,
    owner: SymbolHandle,
    owner_name: &str,
    installation_bound: bool,
) -> Result<Option<&'a psi_typed_trees::signature::AuthoredServiceReachRow>, Vec<Diagnostic>> {
    let matching = compilation
        .authored_service_reach_rows_for(owner)
        .collect::<Vec<_>>();
    let authored = match matching.as_slice() {
        [] => None,
        [row] => Some(*row),
        _ => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{owner_name}` has {} authored service-reach custody rows; expected at most one",
                matching.len(),
            ))]);
        }
    };
    if installation_bound != authored.is_some_and(|row| row.installation_bound) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory installation-bound service-reach custody",
        ))]);
    }
    if authored.is_some_and(|row| row.keyword_source_spans.is_empty()) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has authored service-reach custody without a `reaches` keyword occurrence",
        ))]);
    }
    if let Some(authored) = authored {
        for keyword_source_span in &authored.keyword_source_spans {
            let _ = canonical_source_span_location(
                compilation,
                *keyword_source_span,
                PackageReviewSourceLocationRole::ServiceReach,
            )?;
        }
    }
    Ok(authored)
}

pub(crate) fn derive_declared_service_reach(
    compilation: &CheckedCompilation,
    authored: Option<&psi_typed_trees::signature::AuthoredServiceReachRow>,
    invocations: &[psi_effects::InvocationTarget],
    parameters: &[psi_typed_trees::signature::StateParameter],
    owner_name: &str,
) -> Result<Vec<psi_language_semantics::ServiceReachId>, Vec<Diagnostic>> {
    let mut direct = authored
        .into_iter()
        .flat_map(|row| &row.targets)
        .map(|target| {
            compilation
                .service_reaches
                .id_for_symbol(target.service)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed callable `{owner_name}` retains an authored service-reach target that is stale or not a boundary trait",
                    ))]
                })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let non_self_parameters = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    for invocation in invocations {
        let symbol = match invocation {
            psi_effects::InvocationTarget::Parameter(ordinal) => non_self_parameters
                .get(*ordinal as usize)
                .map(|parameter| {
                    compilation
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                        .type_symbol(&compilation.type_reference_table)
                })
                .unwrap_or_else(SymbolHandle::invalid),
            psi_effects::InvocationTarget::Service(symbol) => *symbol,
        };
        let service = compilation
            .service_reaches
            .id_for_symbol(symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed callable `{owner_name}` has an invocation target without an exact boundary-service identity",
                ))]
            })?;
        direct.push(service);
    }

    let mut closure = Vec::new();
    for service in direct {
        compilation
            .service_reaches
            .extend_closure(service, &mut closure);
    }
    closure.sort_by_key(|service| service.0);
    closure.dedup();
    Ok(closure)
}

pub(crate) fn authored_service_reach_locations(
    authored: Option<&psi_typed_trees::signature::AuthoredServiceReachRow>,
) -> Vec<ProjectedNestedSourceLocation> {
    let Some(authored) = authored else {
        return Vec::new();
    };
    if authored.targets.is_empty() {
        authored
            .keyword_source_spans
            .iter()
            .copied()
            .map(|source_span| ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::ServiceReach,
            })
            .collect()
    } else {
        authored
            .targets
            .iter()
            .map(|target| ProjectedNestedSourceLocation {
                source_span: target.source_span,
                role: PackageReviewSourceLocationRole::ServiceReach,
            })
            .collect()
    }
}

pub(crate) fn project_machine_operational_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = project_operational_keyword_locations(
        compilation,
        machine.name.as_str(),
        "suspends",
        machine.suspends,
        &machine.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    locations.extend(project_operational_keyword_locations(
        compilation,
        machine.name.as_str(),
        "blocks",
        machine.blocks,
        &machine.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?);

    let suspension = compilation
        .facts
        .suspensions
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no exact suspension fact",
                machine.name
            ))]
        })?;
    let blocking = compilation
        .facts
        .blocking
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no exact blocking fact",
                machine.name
            ))]
        })?;
    let publishes = machine.is_public
        || machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody;
    let expected_suspension = if publishes || machine.suspends {
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(machine.suspends)
    } else {
        psi_language_semantics::SuspensionInterface::InternalInferred
    };
    let expected_blocking = if publishes || machine.blocks {
        psi_language_semantics::BlockingInterface::PublishedMayBlock(machine.blocks)
    } else {
        psi_language_semantics::BlockingInterface::InternalInferred
    };
    if suspension.interface != expected_suspension || blocking.interface != expected_blocking {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored operational custody does not equal its exact checked interfaces",
            machine.name
        ))]);
    }
    Ok(locations)
}

pub(crate) fn project_signature_operational_source_locations(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = project_operational_keyword_locations(
        compilation,
        signature.name.as_str(),
        "suspends",
        signature.suspends,
        &signature.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    locations.extend(project_operational_keyword_locations(
        compilation,
        signature.name.as_str(),
        "blocks",
        signature.blocks,
        &signature.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?);
    let checked = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        signature.name.as_str(),
        "signature contract capsule",
    )?;
    if checked.published_may_suspend() != signature.suspends
        || checked.published_may_block() != signature.blocks
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored operational custody does not equal its exact checked contract capsule",
            signature.name
        ))]);
    }
    Ok(locations)
}

pub(crate) fn project_operational_keyword_locations(
    compilation: &CheckedCompilation,
    owner_name: &str,
    clause: &str,
    authored: bool,
    source_spans: &[psi_source::SourceSpan],
    role: PackageReviewSourceLocationRole,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    if authored != !source_spans.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory authored `{clause}` source custody"
        ))]);
    }
    source_spans
        .iter()
        .copied()
        .map(|source_span| {
            canonical_source_span_location(compilation, source_span, role)?;
            Ok(ProjectedNestedSourceLocation { source_span, role })
        })
        .collect()
}

pub(crate) fn project_machine_invocation_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.machine_invokes(machine);
    let declared = psi_effects::declared_machine_invocations(compilation, machine);
    if declared.len() != declarations.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` has an unresolved, duplicate, or semantically aliased authored invokes target",
            machine.name,
        ))]);
    }
    let checked = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "synchronous-invocation",
    )?;
    if checked.published_targets != declared {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored invokes targets do not equal its exact checked published ceiling",
            machine.name,
        ))]);
    }
    let checked_published = canonical_checked_invocation_targets(compilation, &declared)?;
    let checked_inferred =
        canonical_checked_invocation_targets(compilation, &checked.checked_inferred_targets)?;
    if checked.plan.published != checked_published
        || checked.plan.checked_inferred != checked_inferred
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` has contradictory exact and rendered synchronous-invocation facts",
            machine.name,
        ))]);
    }

    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

pub(crate) fn project_signature_invocation_source_locations(
    compilation: &CheckedCompilation,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.state_signature_invokes(signature);
    let targets = psi_effects::declared_signature_invocations(compilation, signature);
    if targets.len() != declarations.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` has an unresolved, duplicate, or semantically aliased authored invokes target",
            signature.name,
        ))]);
    }

    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

pub(crate) fn canonical_checked_invocation_targets(
    compilation: &CheckedCompilation,
    targets: &[psi_effects::InvocationTarget],
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut canonical = targets
        .iter()
        .map(|target| match target {
            psi_effects::InvocationTarget::Parameter(index) => Ok(format!("parameter:{index}")),
            psi_effects::InvocationTarget::Service(symbol) => {
                let matching = compilation
                    .traits()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol)
                    .collect::<Vec<_>>();
                let [definition] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves service symbol {} to {} declarations; expected exactly one",
                        symbol.arena_index(),
                        matching.len(),
                    ))]);
                };
                if !definition.is_boundary {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves `{}` to a non-boundary trait",
                        definition.name,
                    ))]);
                }
                Ok(format!("service:{}", definition.name))
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    canonical.sort();
    canonical.dedup();
    Ok(canonical)
}

pub(crate) fn collect_type_parameter_source_locations(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    locations: &mut Vec<ProjectedNestedSourceLocation>,
) -> Result<(), Vec<Diagnostic>> {
    for parameter in parameters {
        let psi_typed_trees::data::TypeParameterKind::Machine {
            contract: psi_typed_trees::data::MachineParameterContract::Structural(signature),
        } = &parameter.kind
        else {
            continue;
        };
        collect_callable_parameter_source_locations(
            compilation,
            compilation.state_signature_parameters(signature),
            "structural machine parameter contract value parameter",
            locations,
        )?;
        locations.extend(project_contract_source_locations(
            compilation,
            compilation.state_signature_contracts(signature),
        )?);
        locations.extend(project_signature_invocation_source_locations(
            compilation,
            signature,
        )?);
        locations.extend(project_signature_service_reach_source_locations(
            compilation,
            parameter.symbol,
            signature,
        )?);
        locations.extend(project_signature_operational_source_locations(
            compilation,
            parameter.symbol,
            signature,
        )?);
        collect_type_parameter_source_locations(
            compilation,
            compilation.state_signature_type_parameters(signature),
            locations,
        )?;
    }
    Ok(())
}

pub(crate) fn collect_callable_parameter_source_locations(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::signature::StateParameter],
    subject: &str,
    locations: &mut Vec<ProjectedNestedSourceLocation>,
) -> Result<(), Vec<Diagnostic>> {
    for parameter in parameters {
        locations.push(project_nested_declaration_source_location(
            compilation,
            parameter.symbol,
            PackageReviewSourceLocationRole::CallableParameter,
            subject,
        )?);
    }
    Ok(())
}

pub(crate) fn checked_outcome_specific_guarantee<'a>(
    compilation: &'a CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    result_data: SymbolHandle,
    result_case: SymbolHandle,
    binding: Option<&psi_typed_trees::name::Identifier>,
) -> Result<&'a psi_checked_trees::OutcomeSpecificGuaranteeFact, Vec<Diagnostic>> {
    let psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol } = context.owner
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` publishes an outcome-specific guarantee without a checked machine owner",
            context.subject_kind, context.subject_name
        ))]);
    };
    let public_selector = binding.map(|binding| binding.as_str());
    let matching = compilation
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, checked)| {
            (checked.machine_symbol == machine_symbol
                && checked.fact == fact
                && checked.result_data == result_data
                && checked.result_case == result_case
                && checked.public_selector.as_deref() == public_selector)
                .then_some(checked)
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` outcome-specific guarantee has {} exact checked carrier rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    Ok(*checked)
}

pub(crate) fn checked_contract_fact<'a>(
    compilation: &'a CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    kind: PackageReviewContractKind,
) -> Result<&'a psi_checked_trees::ContractProofFact, Vec<Diagnostic>> {
    let checked_kind = match kind {
        PackageReviewContractKind::Requires => psi_checked_trees::ContractProofFactKind::Requires,
        PackageReviewContractKind::Ensures => psi_checked_trees::ContractProofFactKind::Ensures,
    };
    let matching = compilation
        .facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, checked)| {
            (checked.fact == fact && checked.kind == checked_kind && checked.owner == context.owner)
                .then_some(checked)
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract fact has {} checked owner rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    Ok(*checked)
}

pub(crate) fn validate_checked_contract_evidence(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binding: Option<&psi_typed_trees::name::Identifier>,
    checked: &psi_checked_trees::ContractProofFact,
    projected: &PackageReviewContractFact,
) -> Result<Option<u32>, Vec<Diagnostic>> {
    validate_checked_contract_evidence_components(
        compilation,
        context,
        binding,
        checked.owner,
        checked.kind,
        checked.evidence_term,
        projected,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_checked_contract_evidence_components(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binding: Option<&psi_typed_trees::name::Identifier>,
    checked_owner: psi_checked_trees::ContractProofFactOwner,
    checked_kind: psi_checked_trees::ContractProofFactKind,
    checked_evidence_term: Option<psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>>,
    projected: &PackageReviewContractFact,
) -> Result<Option<u32>, Vec<Diagnostic>> {
    let Some(binding) = binding else {
        if checked_evidence_term.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an unnamed contract with a checked evidence term",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(None);
    };
    let Some(term_handle) = checked_evidence_term else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let term = compilation.facts.proof.evidence_terms.get(term_handle);
    if term.name != binding.as_str() || term.owner != checked_owner || term.kind != checked_kind {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not match its checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    if matches!(
        projected,
        PackageReviewContractFact::PropositionParameter(_)
    ) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` uses a generic proposition endpoint without an exact checked witness interface",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let PackageReviewContractFact::Proposition(application) = projected else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` is not a proposition",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    if nominal_identity(compilation, term.proposition.declaration)? != application.declaration {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed proposition endpoint during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let PackageReviewPropositionEvidence::Witness(interface) = &application.evidence else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not expose witness evidence",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let Some(checked_interface) = term.evidence_interface.as_ref() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no exact checked witness interface",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    if nominal_identity(compilation, checked_interface.trait_symbol)? != interface.trait_identity {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed witness trait during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let mut checked_requirements = checked_interface
        .requirements
        .iter()
        .map(|requirement| {
            let owner = compilation
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == requirement.declaring_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked witness requirement has no exact declaring trait",
                    )]
                })?;
            let signature = compilation
                .trait_machine_signatures(owner)
                .iter()
                .find(|candidate| candidate.symbol == requirement.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked witness requirement has no exact overload declaration",
                    )]
                })?;
            Ok((
                nominal_identity(compilation, requirement.declaring_trait)?,
                trait_requirement_identity(compilation, owner, signature)?,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_requirements.sort();
    let mut projected_requirements = interface
        .requirements
        .iter()
        .map(|requirement| {
            (
                requirement.declaring_trait.clone(),
                requirement.requirement.clone(),
            )
        })
        .collect::<Vec<_>>();
    projected_requirements.sort();
    if checked_requirements != projected_requirements {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed witness requirements during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    portable_parameter_position(term.lane_position).map(Some)
}
