use super::super::{CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan};
use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, SymbolHandle, TypedTrees,
};

use crate::execution::terminal_unit::candidate_closure::retain_available;
use crate::execution::terminal_unit::scalar_targets;
use crate::tests::front_end::checked_program_result;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program_result(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn compare_reference(
    checked: &checked_trees::CheckedTrees,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed: &mut Vec<CheckedComposedUnitControlMachinePlan>,
) {
    let boundaries = checked
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .map(|plan| plan.machine)
        .collect::<Vec<_>>();
    let mut expected_candidates = candidates.clone();
    let mut expected_composed = composed.clone();
    retain_available_reference(
        &checked.typed,
        &checked.facts,
        &boundaries,
        &mut expected_candidates,
        &mut expected_composed,
    );
    retain_available(
        &checked.typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
        &boundaries,
        candidates,
        composed,
        &[],
    );
    assert_eq!(
        *candidates, expected_candidates,
        "ordinary bodies and order"
    );
    assert_eq!(*composed, expected_composed, "composed bodies and order");
}

fn composed(plan: &CheckedUnitEffectMachinePlan) -> CheckedComposedUnitControlMachinePlan {
    CheckedComposedUnitControlMachinePlan {
        machine: plan.machine,
        result: checked_trees::CheckedControlResultPlan::Unit,
        result_reference_sources: Vec::new(),
        natural_ranks: Vec::new(),
        attachment_type_identity: plan.attachment_type_identity.clone(),
        provider_attachment_requirements: plan.provider_attachment_requirements.clone(),
        body_qualifications: plan.body_qualifications.clone(),
        contract_report_fingerprint: plan.contract_report_fingerprint,
        contract_commitment: plan.contract_commitment,
        contract_service_reach: plan.contract_service_reach,
        service_reach: plan.service_reach,
        states: vec![CheckedComposedUnitControlStatePlan {
            erased_scalar_parameters: Vec::new(),
            erased_proof_parameters: Vec::new(),
            requires: Vec::new(),
            state: plan.state,
            structural_parameters: Vec::new(),
            scalar_parameters: Vec::new(),
            entry_claims: Vec::new(),
            bindings: Vec::new(),
            binding_initializers: Vec::new(),
            operations: plan.operations.clone(),
            terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
        }],
    }
}

#[test]
fn scalar_fallback_dependency_is_not_a_registered_producer_dependency() {
    let checked = checked(
        "machine touch() {}
         machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine leaf(value: u8) -> u8 {
             touch();
             let row: [u8; 2] = [7u8, 9u8];
             answer(row, value)
         }
         machine caller(value: u8) { let result: u8 = leaf(value); }",
    );
    let machine = |name: &str| {
        checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap()
            .symbol
    };
    for scenario in [
        "valid",
        "missing touch",
        "duplicate leaf",
        "composed collision",
        "invalid scalar signature",
    ] {
        let mut ordinary = checked.facts.flow.terminal_unit_effects.machines.clone();
        let mut graphs = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .clone();
        let leaf = ordinary
            .iter()
            .position(|plan| plan.machine == machine("leaf"))
            .unwrap();
        let caller = ordinary
            .iter()
            .find(|plan| plan.machine == machine("caller"))
            .unwrap();
        let call = caller
            .operations
            .iter()
            .find(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. })
            })
            .unwrap();
        assert!(
            matches!(scalar_targets::available_target(&checked.typed, &checked.facts, crate::execution::terminal_unit::ScalarCalleePlans { boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns, structural_returns: &checked.facts.flow.terminal_structural_scalar_returns }, &ordinary, &[], &scalar_targets::ScalarCallSite::of_plan(caller), call),
            Some(scalar_targets::AvailableScalarTarget::OrdinaryBody(target)) if target == leaf)
        );
        match scenario {
            "missing touch" => ordinary.retain(|plan| plan.machine != machine("touch")),
            "duplicate leaf" => ordinary.push(ordinary[leaf].clone()),
            "composed collision" => graphs.push(composed(&ordinary[leaf])),
            "invalid scalar signature" => ordinary[leaf].scalar_parameters.clear(),
            _ => {}
        }
        compare_reference(&checked, &mut ordinary, &mut graphs);
        assert_eq!(
            ordinary
                .iter()
                .any(|plan| plan.machine == machine("caller")),
            scenario == "valid",
            "{scenario}"
        );
    }
}

fn retain_available_reference(
    program: &TypedTrees,
    facts: &CheckFacts,
    boundary_symbols: &[SymbolHandle],
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed_machines: &mut Vec<CheckedComposedUnitControlMachinePlan>,
) {
    // Both catalogs contain complete admitted bodies. Resolve ordinary calls
    // against their joint entry roster, then prune both sides to a fixed point:
    // an invalid composed leaf must also retire its ordinary upstream callers.
    loop {
        let entries = candidates
            .iter()
            .map(|plan| (plan.machine, plan.state))
            .chain(composed_machines.iter().map(|plan| {
                (
                    plan.machine,
                    plan.states
                        .first()
                        .map_or(SymbolHandle::invalid(), |state| state.state),
                )
            }))
            .collect::<Vec<_>>();
        let unique_entries = entries
            .iter()
            .filter(|(machine, state)| {
                machine.is_valid()
                    && state.is_valid()
                    && entries
                        .iter()
                        .filter(|(candidate, _)| candidate == machine)
                        .count()
                        == 1
            })
            .copied()
            .collect::<Vec<_>>();
        let old_lengths = (candidates.len(), composed_machines.len());
        // Every scalar call observes the same candidate roster for this pass.
        // Mutating it during availability checks would make transitive pruning
        // depend on declaration order; no body copies are needed to retain it.
        let retained_candidates = candidates
            .iter()
            .map(|plan| {
                if !unique_entries.contains(&(plan.machine, plan.state)) {
                    return false;
                }
                plan.operations
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .all(|operation| match operation {
                        CheckedUnitEffectOperationPlan::CallUnit {
                            target_machine,
                            target_state,
                            ..
                        } => unique_entries.contains(&(*target_machine, *target_state)),
                        CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                            boundary_symbols.contains(target_machine)
                        }
                        CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                            target_machine,
                            ..
                        } => boundary_symbols.contains(target_machine),
                        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            target_machine,
                            ..
                        } => boundary_symbols.contains(target_machine),
                        CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                            scalar_targets::available_target(
                                program,
                                facts,
                                crate::execution::terminal_unit::ScalarCalleePlans {
                                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                                    structural_returns: &facts
                                        .flow
                                        .terminal_structural_scalar_returns,
                                },
                                candidates,
                                &[],
                                &scalar_targets::ScalarCallSite::of_plan(plan),
                                operation,
                            )
                            .is_some()
                        }
                        CheckedUnitEffectOperationPlan::StructuralCall {
                            target_machine,
                            target_state,
                            ..
                        } => {
                            unique_entries.contains(&(*target_machine, *target_state))
                                || facts
                                    .flow
                                    .terminal_structural_returns
                                    .claim_free_affine_for_machine(*target_machine)
                                    .is_some()
                        }
                        CheckedUnitEffectOperationPlan::PortWrite { .. }
                        | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                        | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                        | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                        | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                        | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                        | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                        | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                        | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                        | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                        | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                        | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                        | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                        | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                        | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                        | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                        | CheckedUnitEffectOperationPlan::Complete { .. } => true,
                    })
            })
            .collect::<Vec<_>>();
        let mut retained_candidates = retained_candidates.into_iter();
        candidates.retain(|_| retained_candidates.next().unwrap_or(false));
        composed_machines.retain(|plan| {
            unique_entries
                .iter()
                .any(|(machine, _)| *machine == plan.machine)
                && plan
                    .states
                    .iter()
                    .flat_map(|state| &state.operations)
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .all(|operation| match operation {
                        CheckedUnitEffectOperationPlan::CallUnit {
                            target_machine,
                            target_state,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::StructuralCall {
                            target_machine,
                            target_state,
                            ..
                        } => unique_entries.contains(&(*target_machine, *target_state)),
                        CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                            boundary_symbols.contains(target_machine)
                        }
                        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            target_machine,
                            ..
                        } => boundary_symbols.contains(target_machine),
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                        | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                        | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                        | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. } => true,
                        _ => false,
                    })
        });
        if (candidates.len(), composed_machines.len()) == old_lengths {
            break;
        }
    }
}
