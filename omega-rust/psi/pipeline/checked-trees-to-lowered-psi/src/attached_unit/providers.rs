//! Exact provider-candidate discovery for an attached Unit closure.
//!
//! Checked providers use the same ordinary/composed body closure as direct
//! calls. A structural result does not imply an identity-return implementation:
//! a checked graph may construct it after calling helpers or boundary leaves.
//! Retaining that graph as a closure root preserves those dependencies and its
//! source replay. The affine identity family keeps its existing separate emitter;
//! competing body plans must reject rather than choose whichever path succeeds.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProviderBody {
    Callable,
    AffineIdentity,
}

pub(super) fn affine_candidate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<&checked_trees::CheckedClaimFreeAffineStructuralReturnMachinePlan, LoweringError> {
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
    let body = UnitBody::find(&checked.facts.flow.terminal_unit_effects, machine)?;
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

pub(super) fn checked_unit_provider_candidates(
    checked: &CheckedTrees,
    closure: &[symbols::SymbolHandle],
) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
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
        let (definition, signature) = match exact_requirements.as_slice() {
            [] => continue,
            [(definition, signature)] => (*definition, *signature),
            _ => {
                return unsupported(
                    "Unit boundary provider catalog requires one exact trait/signature symbol coordinate",
                );
            }
        };
        let requirement_identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity();
        if requirement_identity.is_empty() {
            return unsupported("Unit boundary requirement has an empty overload identity");
        }
        let candidates = checked.typed.machines().iter().filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && machine.attached_data.is_some()
                && checked
                    .typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        conformance.external_binding.is_none()
                            && conformance.symbol == definition.symbol
                            && conformance
                                .requirement
                                .as_ref()
                                .is_some_and(|name| name == &signature.name)
                    })
        });
        for machine in candidates {
            let body = match &boundary.result {
                checked_trees::CheckedBoundaryMachineResultPlan::Unit => {
                    let candidate = callable_candidate(checked, machine.symbol).map_err(
                        |error| match error {
                            LoweringError::Unsupported(reason) => {
                                LoweringError::InvalidUnitMachinePlan {
                                    machine: machine.name.as_str().to_owned(),
                                    reason,
                                }
                            }
                            error => error,
                        },
                    )?;
                    if candidate.result()? != checked_trees::CheckedControlResultPlan::Unit {
                        return unsupported(
                            "provider result disagrees with its Unit boundary requirement",
                        );
                    }
                    ProviderBody::Callable
                }
                checked_trees::CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    qualifications,
                } => {
                    let (body, result) = if plans.composed_for_machine(machine.symbol).is_some() {
                        let candidate = callable_candidate(checked, machine.symbol)?;
                        let checked_trees::CheckedControlResultPlan::Structural(result) =
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
                    if *multiplicity != Multiplicity::Affine
                        || !qualifications.is_empty()
                        || result.type_identity != *type_identity
                        || result.multiplicity != *multiplicity
                        || !result.qualifications.is_empty()
                        || !boundary.domain_requirements.is_empty()
                    {
                        return unsupported(
                            "provider affine result disagrees with its boundary requirement",
                        );
                    }
                    body
                }
                checked_trees::CheckedBoundaryMachineResultPlan::Scalar(_) => {
                    return unsupported(
                        "scalar-result provider candidates have no admitted terminal route",
                    );
                }
            };
            output.push(CheckedUnitProviderCandidate {
                body,
                boundary: boundary_symbol,
                candidate: machine.symbol,
                requirement_identity: requirement_identity.clone(),
                provider_identity: machine
                    .attached_data
                    .as_ref()
                    .expect("candidate filter requires an attached provider type")
                    .as_str()
                    .to_owned(),
                candidate_identity: checked_terminal_machine_name(checked, machine.symbol)?
                    .to_owned(),
            });
        }
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
