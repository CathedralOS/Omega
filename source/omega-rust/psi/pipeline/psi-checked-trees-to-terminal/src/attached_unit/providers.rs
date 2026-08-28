//! Exact provider-candidate discovery for an attached Unit closure.

use super::*;

pub(super) fn checked_unit_provider_candidates(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
) -> Result<Vec<CheckedUnitProviderCandidate>, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut boundary_symbols = closure
        .iter()
        .flat_map(|symbol| {
            plans
                .for_machine(*symbol)
                .into_iter()
                .flat_map(|plan| &plan.operations)
        })
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                Some(*target_machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    boundary_symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    boundary_symbols.dedup();
    let mut output = Vec::new();
    for boundary_symbol in boundary_symbols {
        plans
            .boundary_for_machine(boundary_symbol)
            .ok_or(LoweringError::Unsupported(
                "Unit provider catalog references an unknown checked boundary plan",
            ))?;
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
            machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
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
            plans
                .for_machine(machine.symbol)
                .ok_or(LoweringError::Unsupported(
                    "checked Unit provider candidate has no complete terminal body plan",
                ))?;
            output.push(CheckedUnitProviderCandidate {
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
