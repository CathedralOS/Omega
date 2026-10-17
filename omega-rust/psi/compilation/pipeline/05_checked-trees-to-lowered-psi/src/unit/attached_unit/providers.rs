//! Exact provider-candidate discovery for an attached Unit closure.
//!
//! Checked providers use the same ordinary/composed body closure as direct
//! calls. A structural result does not imply an identity-return implementation:
//! a checked graph may construct it after calling helpers or boundary leaves.
//! Retaining that graph as a closure root preserves those dependencies and its
//! source replay. Identity returns use that same body owner, not the legacy
//! affine-return summary: a summary beside its ordinary body is not a second
//! implementation. Missing or duplicate ordinary bodies still reject in
//! `UnitBody::find`; provider signatures and custody are joined separately.
//! Retained expression computations own boundary dependencies too. Scanning
//! only statement operations can omit the supplier of an executable call.
//!
//! The catalog records every checked supplier, not the frontend's selected
//! target. Family retention also roots satisfied requirements without a call:
//! an unreachable member must not lose its conformance at this boundary.
//! Omega projects these exact callable IDs onto an execution scope before
//! installing providers; family membership itself grants no installation.
//! Named implementation rows alone supply whole-trait provider identities. A
//! realization's exact satisfaction edge proves its contract, but its qualifier
//! (a namespace or data declaration) is not another provider. Inferring that
//! extra candidate used to reject namespace-qualified realizations and invented
//! data-owned candidates beside otherwise valid named maps. Exact top-level
//! requirements and operators retain their checked satisfaction edges instead;
//! their attached owner groups leaf suppliers, never a whole-trait conformance.

use super::bodies::UnitPlans;
use super::operation_frame::BoundaryParameters;
use super::{
    BoundaryMachineDeclaration, CheckedTrees, CheckedUnitProviderCandidate, LoweringError,
    MachineId, ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, TerminalMachine, UnitBody, lookup_machine_id,
    unique_unit_boundary, unsupported,
};
use checked_trees::machine::Machine;

mod named;
mod selection_coordinates;

/// Report the exact checked provider whose ordinary body is missing, including
/// the checker-owned omission. Result shape must not hide that source frontier.
fn callable_candidate<'program>(
    checked: &CheckedTrees,
    plans: UnitPlans<'program>,
    machine: &Machine,
) -> Result<UnitBody<'program>, LoweringError> {
    UnitBody::find(plans, machine.symbol).map_err(crate::lowering_error::unit_machine_plan_error(
        checked,
        machine.name.as_str(),
        machine.symbol,
    ))
}

/// The requirement a boundary call names: a boundary-trait signature, or a
/// top-level `boundary requirement` declaration, or a legacy operator.
#[derive(Clone, Copy)]
enum CatalogRequirement<'a> {
    Trait {
        definition: &'a checked_trees::trait_definition::TraitDefinition,
        signature: &'a checked_trees::signature::StateSignature,
    },
    TopLevel(&'a checked_trees::machine::Machine),
    Operator(&'a typed_trees::operator::OperatorDefinition),
}

fn provider_identities(
    checked: &CheckedTrees,
    machine: &Machine,
    requirement: CatalogRequirement<'_>,
) -> Result<Vec<(symbols::SymbolHandle, String)>, LoweringError> {
    if matches!(requirement, CatalogRequirement::Trait { .. }) {
        return named::provider_identities(checked, machine, requirement);
    }
    let supplies = checked
        .typed
        .machine_trait_conformances(machine)
        .iter()
        .any(|conformance| {
            if conformance.external_binding.is_some() {
                return false;
            }
            match requirement {
                CatalogRequirement::TopLevel(declaration) => {
                    (conformance.symbol == declaration.symbol
                        && conformance.requirement_symbol == declaration.symbol)
                        || checked
                            .facts
                            .provider_closed_requirements
                            .iter()
                            .any(|row| {
                                row.provider == machine.symbol
                                    && row.selecting_declaration == machine.symbol
                                    && row.template == conformance.symbol
                                    && row.closed_requirement == declaration.symbol
                            })
                }
                CatalogRequirement::Operator(operator) => conformance.requirement_symbol
                    == operator.symbol
                    && typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                        &checked.typed,
                        machine,
                        conformance,
                    )
                    .is_some_and(|resolved| resolved.symbol == operator.symbol),
                CatalogRequirement::Trait { .. } => unreachable!(),
            }
        });
    if !supplies {
        return Ok(Vec::new());
    }
    let owner = machine.attached_data_symbol;
    if !owner.is_valid() {
        return unsupported("exact leaf supplier has no attached declaration owner");
    }
    let identity = checked
        .typed
        .attached_data_path(machine)
        .ok_or(LoweringError::Unsupported(
            "exact leaf supplier has no attached owner identity",
        ))?;
    Ok(vec![(owner, identity)])
}

pub(super) fn checked_unit_provider_candidates(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
    retained_requirements: &mut Vec<symbols::SymbolHandle>,
) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
    let bodies = closure
        .iter()
        .map(|symbol| UnitBody::find(plans, *symbol))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut boundary_symbols = bodies
        .into_iter()
        .flat_map(UnitBody::operations)
        .filter_map(|operation| {
            operation
                .call()
                .filter(|call| call.is_boundary())
                .map(|call| call.target.machine)
        })
        .chain(retained_requirements.iter().copied())
        .collect::<Vec<_>>();
    for machine in closure {
        boundary_symbols.extend(
            crate::expression_preparation::computation_graph::boundary_targets(checked, *machine)?,
        );
    }
    // A runtime call demands its complete boundary-trait interface. Retain
    // unused coordinates before candidate/body discovery so automatic unique
    // selection has the same whole-plan semantics as an authored default.
    // The checker still owns every concrete boundary plan; no open application
    // or missing method is synthesized from a provider body.
    let demanded_traits = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && checked
                    .typed
                    .trait_machine_instances(definition)
                    .any(|signature| boundary_symbols.contains(&signature.symbol))
        })
        .collect::<Vec<_>>();
    for definition in demanded_traits {
        typed_trees::trait_definition::AppliedTraitScope::abstract_scope(definition).visit(
            &checked.typed,
            &mut |scope| {
                boundary_symbols.extend(
                    checked
                        .typed
                        .trait_machine_instances(scope.declaration)
                        .map(|signature| signature.symbol),
                );
            },
        );
    }
    boundary_symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    boundary_symbols.dedup();
    *retained_requirements = boundary_symbols.clone();
    let mut output = Vec::new();
    for boundary_symbol in boundary_symbols {
        let boundary = unique_unit_boundary(plans, boundary_symbol)?;
        let exact_requirements = checked
            .typed
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                checked
                    .typed
                    .trait_machine_instances(definition)
                    .filter(move |signature| signature.symbol == boundary_symbol)
                    .map(move |signature| (definition, signature))
            })
            .collect::<Vec<_>>();
        // A boundary-trait requirement names its trait and signature; a
        // top-level `boundary requirement` is its own declaration, which its
        // adapters satisfy directly. Either way the catalog lists every
        // checked adapter, and Omega installs the Build's selected one.
        let requirement = match exact_requirements.as_slice() {
            [(definition, signature)] => CatalogRequirement::Trait {
                definition,
                signature,
            },
            [] => match checked.typed.machines().iter().find(|machine| {
                machine.symbol == boundary_symbol
                    && machine.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            }) {
                Some(requirement) => CatalogRequirement::TopLevel(requirement),
                None => match checked
                    .typed
                    .operators()
                    .iter()
                    .find(|operator| operator.is_boundary && operator.symbol == boundary_symbol)
                {
                    Some(operator) => CatalogRequirement::Operator(operator),
                    None => continue,
                },
            },
            _ => {
                return unsupported(
                    "Unit boundary provider catalog requires one exact trait/signature symbol coordinate",
                );
            }
        };
        let requirement_identity = match requirement {
            CatalogRequirement::Trait {
                definition,
                signature,
            } => checked
                .typed
                .trait_requirement_callable_identity(definition, signature),
            CatalogRequirement::TopLevel(requirement) => {
                checked.typed.machine_callable_identity(requirement)
            }
            CatalogRequirement::Operator(operator) => checked
                .typed
                .boundary_operator_requirement_callable_identity(operator),
        };
        if !requirement_identity.is_valid() {
            return unsupported("Unit boundary requirement has an empty overload identity");
        }
        for machine in checked.typed.machines().iter().filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        }) {
            let providers = provider_identities(checked, machine, requirement)?;
            if providers.is_empty() {
                continue;
            }
            // Every result shape is served by the candidate's ordinary callable
            // body; this match only rejoins the body's result to the
            // requirement's.
            match &boundary.result {
                checked_trees::CheckedBoundaryMachineResultPlan::Unit => {
                    let candidate = callable_candidate(checked, plans, machine)?;
                    if candidate.result()? != checked_trees::CheckedControlResultPlan::Unit {
                        return unsupported(
                            "provider result disagrees with its Unit boundary requirement",
                        );
                    }
                }
                checked_trees::CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    qualifications,
                    projected_qualifications,
                } => {
                    let candidate = callable_candidate(checked, plans, machine)?;
                    let checked_trees::CheckedControlResultPlan::Structural(result) =
                        candidate.result()?
                    else {
                        return unsupported(
                            "provider result disagrees with its structural boundary requirement",
                        );
                    };
                    // Conformance authorizes boundary result qualifications.
                    // Ownership comes from the selected ordinary body's actual
                    // return claims, joined by typed path at installation and
                    // transferred through its ordinary call continuation.
                    if result.type_identity != *type_identity
                        || result.multiplicity != *multiplicity
                        || projected_qualifications
                            .iter()
                            .any(|row| !result.projected_qualifications.contains(row))
                        || result
                            .qualifications
                            .iter()
                            .any(|domain| !qualifications.contains(domain))
                    {
                        return unsupported(
                            "provider structural result disagrees with its boundary requirement",
                        );
                    }
                }
                // A scalar requirement is served by an ordinary callable body
                // returning the same primitive; installation replays the
                // call's scalar result like its Unit and structural cousins.
                checked_trees::CheckedBoundaryMachineResultPlan::Scalar(expected) => {
                    let candidate = callable_candidate(checked, plans, machine)?;
                    if candidate.scalar_result_type() != Some(*expected) {
                        return unsupported(
                            "provider result disagrees with its scalar boundary requirement",
                        );
                    }
                }
            }
            for (provider, provider_identity) in providers {
                if !provider.is_valid() {
                    return unsupported("provider candidate has no exact declaration owner");
                }
                output.push(CheckedUnitProviderCandidate {
                    provider,
                    boundary: boundary_symbol,
                    candidate: machine.symbol,
                    requirement_identity,
                    // The selected declaration owns provider identity; the body
                    // may be an independently owned named-map realization.
                    provider_identity,
                    // Provider selection compares semantic overload identities;
                    // diagnostic display paths cannot distinguish those overloads.
                    candidate_identity: checked.typed.machine_callable_identity(machine),
                });
            }
        }
    }
    output.sort_by(|left, right| {
        (
            left.boundary.arena_index(),
            left.boundary.generation(),
            &left.provider_identity,
            left.provider.arena_index(),
            left.provider.generation(),
            left.candidate.arena_index(),
            left.candidate.generation(),
        )
            .cmp(&(
                right.boundary.arena_index(),
                right.boundary.generation(),
                &right.provider_identity,
                right.provider.arena_index(),
                right.provider.generation(),
                right.candidate.arena_index(),
                right.candidate.generation(),
            ))
    });
    if output.windows(2).any(|pair| {
        pair[0].boundary == pair[1].boundary
            && pair[0].provider_identity == pair[1].provider_identity
            && pair[0].provider == pair[1].provider
            && pair[0].candidate == pair[1].candidate
    }) {
        return unsupported("Unit provider catalog contains a duplicate exact candidate");
    }
    Ok(output)
}

/// Each candidate's conformance to the boundary requirement it satisfies:
/// the emitted candidate's scalar signature equals the boundary's, and its
/// structural parameters refine the boundary's positionally. Rows are in
/// canonical (boundary, provider, candidate) order.
pub(super) fn conformances(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    candidates: &[CheckedUnitProviderCandidate],
    boundary_parameters: &[BoundaryParameters],
    boundary_machines: &[BoundaryMachineDeclaration],
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    machines: &[TerminalMachine],
) -> Result<Vec<ProviderCandidateConformance>, LoweringError> {
    let mut conformances = candidates
        .iter()
        .map(|candidate| {
            let BoundaryParameters {
                id: boundary,
                structural: parameters,
                scalar: scalar_parameters,
                ..
            } = boundary_parameters
                .iter()
                .find(|boundary| boundary.source == candidate.boundary)
                .ok_or(LoweringError::Unsupported(
                    "provider candidate references an unlowered Unit boundary requirement",
                ))?;
            let terminal_candidate = lookup_machine_id(machine_ids, candidate.candidate)?;
            let realized = machines
                .iter()
                .find(|machine| machine.id == terminal_candidate)
                .expect("provider candidate root was lowered as an ordinary terminal machine");
            if scalar_parameters.len() != realized.parameters.len()
                || scalar_parameters
                    .iter()
                    .zip(&realized.parameters)
                    .any(|(boundary, candidate)| *boundary != candidate.scalar_type)
            {
                return unsupported(
                    "provider candidate scalar signature disagrees with its boundary requirement",
                );
            }
            let provider_identity = identities.intern(&candidate.provider_identity);
            let (
                source_requirement_identity,
                source_candidate_identity,
                source_candidate_package_identity,
            ) = selection_coordinates::intern(identities, checked, candidate)?;
            let (receiving_application, receiving_region_arguments) =
                selection_coordinates::intern_receiving_application(
                    identities, checked, candidate,
                )?;
            Ok(ProviderCandidateConformance {
                boundary: *boundary,
                source_requirement_identity,
                receiving_application,
                receiving_region_arguments,
                source_candidate_identity,
                source_candidate_package_identity,
                requirement_identity: identities.intern(
                    &checked
                        .typed
                        .callable_identity_text(candidate.requirement_identity),
                ),
                provider_identity,
                provider_declaration: crate::machine_lowering::declarations::nominal(
                    identities,
                    checked,
                    exact_toolchain_sources,
                    candidate.provider,
                )
                .map_err(LoweringError::Unsupported)?,
                candidate_identity: identities.intern(
                    &checked
                        .typed
                        .callable_identity_text(candidate.candidate_identity),
                ),
                candidate: terminal_candidate,
                signature: ProviderSignature {
                    parameters: parameters
                        .iter()
                        .map(|parameter| ProviderSignatureParameter {
                            position: parameter.position,
                            structural_type: parameter.structural_type,
                            multiplicity: parameter.multiplicity,
                            access: parameter.access,
                            qualifications: parameter.qualifications.clone(),
                            projected_qualifications: parameter.projected_qualifications.clone(),
                        })
                        .collect(),
                },
                refinement: ProviderRefinement {
                    positional_parameters: (0..parameters.len())
                        .map(|index| {
                            let index = u32::try_from(index).map_err(|_| {
                                LoweringError::Unsupported("provider signature arity exceeds u32")
                            })?;
                            Ok(ProviderParameterRefinement {
                                boundary_index: index,
                                candidate_index: index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                    required_domains: boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                        .expect("lowered provider boundary declaration exists")
                        .requires
                        .clone(),
                    realized_service_ceiling: realized.published_service_ceiling.clone(),
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    conformances.sort_by(|left, right| {
        (
            left.boundary,
            identities.text(left.provider_identity).unwrap_or_default(),
            identities
                .text(left.provider_declaration.identity())
                .unwrap_or_default(),
            identities.text(left.candidate_identity).unwrap_or_default(),
            left.candidate,
        )
            .cmp(&(
                right.boundary,
                identities.text(right.provider_identity).unwrap_or_default(),
                identities
                    .text(right.provider_declaration.identity())
                    .unwrap_or_default(),
                identities
                    .text(right.candidate_identity)
                    .unwrap_or_default(),
                right.candidate,
            ))
    });
    Ok(conformances)
}
