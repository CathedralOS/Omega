//! Exact provider-candidate discovery for an attached Unit closure.
//!
//! Checked providers use the same ordinary/composed body closure as direct
//! calls. A structural result does not imply an identity-return implementation:
//! a checked graph may construct it after calling helpers or boundary leaves.
//! Retaining that graph as a closure root preserves those dependencies and its
//! source replay. The affine identity family keeps its existing separate emitter;
//! competing body plans must reject rather than choose whichever path succeeds.

use super::bodies::UnitPlans;
use super::operation_frame::BoundaryParameters;
use super::{
    BoundaryMachineDeclaration, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitProviderCandidate, LoweringError, MachineId, Multiplicity,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, TerminalMachine, UnitBody, lookup_machine_id,
    unique_unit_boundary, unsupported,
};
use typed_trees_to_checked_trees::checked_trees::machine::{Machine, TraitConformance};
use typed_trees_to_checked_trees::validation::{
    TopLevelSymbols, resolve_closed_requirement_application,
    validate_checked_machine_specialization_commitments,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProviderBody {
    Callable,
    AffineIdentity,
    /// A scalar provider emitted exactly as an ordinary scalar call's
    /// callee: its scalar graph, with its contract, rather than a Unit body.
    ScalarCallee,
}

pub(super) fn affine_candidate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<
    &typed_trees_to_checked_trees::checked_trees::CheckedClaimFreeAffineStructuralReturnMachinePlan,
    LoweringError,
> {
    let mut candidates = checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .filter(|plan| plan.machine == machine);
    let candidate = candidates.next().ok_or(LoweringError::Unsupported(
        "provider candidate has no checked affine identity return plan",
    ))?;
    if candidates.next().is_some()
        || checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_some()
        || checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine)
            .is_some()
    {
        return unsupported("provider candidate has ambiguous terminal body plans");
    }
    Ok(candidate)
}

fn callable_candidate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<UnitBody<'_>, LoweringError> {
    let body = UnitBody::find(
        UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        machine,
    )?;
    if checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .any(|plan| plan.machine == machine)
    {
        return unsupported("provider candidate has ambiguous terminal body plans");
    }
    Ok(body)
}

/// The requirement a boundary call names: a boundary-trait signature, or a
/// top-level `boundary requirement` declaration.
#[derive(Clone, Copy)]
enum CatalogRequirement<'a> {
    Trait {
        definition:
            &'a typed_trees_to_checked_trees::checked_trees::trait_definition::TraitDefinition,
        signature: &'a typed_trees_to_checked_trees::checked_trees::signature::StateSignature,
    },
    TopLevel(&'a typed_trees_to_checked_trees::checked_trees::machine::Machine),
}

/// A closed requirement keeps its authored conformance coordinate on the
/// provider. Rejoin through checked applications, never through equal runtime
/// signatures: two const tuples may have identical layouts and still name
/// different boundary obligations.
fn satisfies_requirement<'program>(
    checked: &'program CheckedTrees,
    machine: &Machine,
    conformance: &TraitConformance,
    requirement: CatalogRequirement<'program>,
    symbols: &mut Option<TopLevelSymbols<'program>>,
    specializations: &mut Vec<symbols::SymbolHandle>,
) -> Result<bool, LoweringError> {
    if conformance.external_binding.is_some() {
        return Ok(false);
    }
    let requirement = match requirement {
        CatalogRequirement::Trait {
            definition,
            signature,
        } => {
            return Ok(conformance.symbol == definition.symbol
                && if conformance.requirement_symbol.is_valid() {
                    conformance.requirement_symbol == signature.symbol
                } else {
                    conformance
                        .requirement
                        .as_ref()
                        .is_some_and(|name| name == &signature.name)
                });
        }
        CatalogRequirement::TopLevel(requirement) => requirement,
    };
    if conformance.symbol == requirement.symbol
        && conformance.requirement_symbol == requirement.symbol
    {
        return Ok(true);
    }
    if conformance.symbol != conformance.requirement_symbol
        || !checked.typed.machine_type_parameters(machine).is_empty()
    {
        return Ok(false);
    }
    let Some(template) = checked.machines().iter().find(|candidate| {
        candidate.symbol == conformance.symbol
            && candidate.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && !checked.typed.machine_type_parameters(candidate).is_empty()
    }) else {
        return Ok(false);
    };
    // Start at the authored satisfaction edge, not the optional receipt
    // roster. Deleting a closed requirement's receipt must fail its join
    // rather than silently turning a checked candidate into an absent one.
    if symbols.is_none() {
        let mut diagnostics = Vec::new();
        *symbols = Some(TopLevelSymbols::build(&checked.typed, &mut diagnostics));
        if !diagnostics.is_empty() {
            return unsupported("provider catalog contains invalid declaration identities");
        }
    }
    let Some(symbols) = symbols.as_ref() else {
        return unsupported("provider catalog has no declaration identities");
    };
    let closed = resolve_closed_requirement_application(&checked.typed, machine, template, symbols)
        .map_err(|_| {
            LoweringError::Unsupported(
                "provider catalog cannot rejoin the exact closed requirement application",
            )
        })?;
    if closed.symbol != requirement.symbol {
        return Ok(false);
    }
    specializations.extend([machine.symbol, requirement.symbol]);
    Ok(true)
}

pub(super) fn checked_unit_provider_candidates(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    closure: &[symbols::SymbolHandle],
) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
    let bodies = closure
        .iter()
        .map(|symbol| UnitBody::find(plans, *symbol))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut boundary_symbols = bodies
        .into_iter()
        .flat_map(UnitBody::operations)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { target_machine, .. } => {
                Some(*target_machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    boundary_symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    boundary_symbols.dedup();
    let mut output = Vec::new();
    let mut symbols = None;
    let mut specializations = Vec::new();
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
                    .trait_machine_signatures(definition)
                    .iter()
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
                None => continue,
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
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity(),
            CatalogRequirement::TopLevel(requirement) => checked
                .typed
                .normalized_machine_overload_identity(requirement)
                .map(|identity| identity.identity())
                .unwrap_or_default(),
        };
        if requirement_identity.is_empty() {
            return unsupported("Unit boundary requirement has an empty overload identity");
        }
        for machine in checked.typed.machines().iter().filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && machine.attached_data.is_some()
        }) {
            let mut matches = false;
            for conformance in checked.typed.machine_trait_conformances(machine) {
                if satisfies_requirement(
                    checked,
                    machine,
                    conformance,
                    requirement,
                    &mut symbols,
                    &mut specializations,
                )? {
                    matches = true;
                    break;
                }
            }
            if !matches {
                continue;
            }
            let body = match &boundary.result {
                typed_trees_to_checked_trees::checked_trees::CheckedBoundaryMachineResultPlan::Unit => {
                    let candidate = callable_candidate(checked, machine.symbol).map_err(
                        |error| match error {
                            LoweringError::Unsupported(reason) => {
                                LoweringError::InvalidUnitMachinePlan {
                                    machine: machine.name.as_str().to_owned(),
                                    reason,
                                    omission: crate::unit::unit_plan_omission_explanation(
                                        checked,
                                        machine.symbol,
                                    ),
                                }
                            }
                            error => error,
                        },
                    )?;
                    if candidate.result()? != typed_trees_to_checked_trees::checked_trees::CheckedControlResultPlan::Unit {
                        return unsupported(
                            "provider result disagrees with its Unit boundary requirement",
                        );
                    }
                    ProviderBody::Callable
                }
                typed_trees_to_checked_trees::checked_trees::CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    qualifications,
                } => {
                    let (body, result) = if plans.for_machine(machine.symbol).is_some()
                        || plans.composed_for_machine(machine.symbol).is_some()
                    {
                        let candidate = callable_candidate(checked, machine.symbol)?;
                        let typed_trees_to_checked_trees::checked_trees::CheckedControlResultPlan::Structural(result) =
                            candidate.result()?
                        else {
                            return unsupported(
                                "provider result disagrees with its structural boundary requirement",
                            );
                        };
                        (ProviderBody::Callable, result)
                    } else {
                        let candidate = affine_candidate(checked, machine.symbol)?;
                        (ProviderBody::AffineIdentity, candidate.result.clone())
                    };
                    // The requirement's result qualifications and argument
                    // domain requirements are minted on the caller side by the
                    // checked boundary plan and its claim transfers; the
                    // selected conformance has already joined the provider's
                    // own contract to them. The provider's declared result
                    // must still be the exact carrier, and any qualifications
                    // it declares itself must be authorized by the requirement.
                    if *multiplicity != Multiplicity::Affine
                        || result.type_identity != *type_identity
                        || result.multiplicity != *multiplicity
                        || result
                            .qualifications
                            .iter()
                            .any(|domain| !qualifications.contains(domain))
                    {
                        return unsupported(
                            "provider affine result disagrees with its boundary requirement",
                        );
                    }
                    body
                }
                // A scalar requirement is served by an ordinary callable body
                // returning the same primitive; installation replays the
                // call's scalar result like its Unit and structural cousins.
                // It takes the route an ordinary scalar call to it would:
                // its scalar owner when it has one (keeping its contract
                // lowering), otherwise its Unit body's scalar completion.
                typed_trees_to_checked_trees::checked_trees::CheckedBoundaryMachineResultPlan::Scalar(expected) => {
                    match super::CheckedScalarCallee::find_for_unit_call(checked, machine.symbol)? {
                        super::CheckedScalarCallee::Operations(_) => {
                            let candidate = callable_candidate(checked, machine.symbol)?;
                            if candidate.scalar_result_type() != Some(*expected) {
                                return unsupported(
                                    "provider result disagrees with its scalar boundary requirement",
                                );
                            }
                            ProviderBody::Callable
                        }
                        _ => ProviderBody::ScalarCallee,
                    }
                }
            };
            output.push(CheckedUnitProviderCandidate {
                body,
                boundary: boundary_symbol,
                candidate: machine.symbol,
                requirement_identity: requirement_identity.clone(),
                // Planning and Terminal installation compare the same complete
                // nominal path. A leaf spelling aliases sibling providers.
                provider_identity: checked.typed.attached_data_path(machine).ok_or(
                    LoweringError::Unsupported("provider candidate has no attached data path"),
                )?,
                // Provider selection compares semantic overload identities;
                // diagnostic display paths cannot distinguish those overloads.
                candidate_identity: checked
                    .typed
                    .normalized_machine_overload_identity(machine)
                    .ok_or(LoweringError::Unsupported(
                        "provider candidate has no normalized callable identity",
                    ))?
                    .identity(),
            });
        }
    }
    // Discovery never grants authority to self-consistent but substituted
    // applications. Replay both sides once for this catalog batch before any
    // candidate can be published or contribute its body closure.
    if !specializations.is_empty() {
        specializations.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        specializations.dedup();
        validate_checked_machine_specialization_commitments(checked, &specializations)
            .map_err(LoweringError::Unsupported)?;
    }
    output.sort_by(|left, right| {
        (
            left.boundary.arena_index(),
            left.boundary.generation(),
            &left.provider_identity,
            left.candidate.arena_index(),
            left.candidate.generation(),
        )
            .cmp(&(
                right.boundary.arena_index(),
                right.boundary.generation(),
                &right.provider_identity,
                right.candidate.arena_index(),
                right.candidate.generation(),
            ))
    });
    if output.windows(2).any(|pair| {
        pair[0].boundary == pair[1].boundary
            && pair[0].provider_identity == pair[1].provider_identity
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
            Ok(ProviderCandidateConformance {
                boundary: *boundary,
                requirement_identity: candidate.requirement_identity.clone(),
                provider_identity: candidate.provider_identity.clone(),
                candidate_identity: candidate.candidate_identity.clone(),
                candidate: terminal_candidate,
                signature: ProviderSignature {
                    parameters: parameters
                        .iter()
                        .map(|parameter| ProviderSignatureParameter {
                            position: parameter.position,
                            is_self: parameter.is_self,
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
            &left.provider_identity,
            &left.candidate_identity,
            left.candidate,
        )
            .cmp(&(
                right.boundary,
                &right.provider_identity,
                &right.candidate_identity,
                right.candidate,
            ))
    });
    Ok(conformances)
}
