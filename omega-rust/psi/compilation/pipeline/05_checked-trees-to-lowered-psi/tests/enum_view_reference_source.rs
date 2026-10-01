//! Closed-case reference returns dispose their temporary view on every arm
//! while preserving the exact authored source of the surviving permission.

use checked_trees::{
    CheckedClosedSumCaseOutcomePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedConditionalReturnArm, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralPathSegment,
};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

const SOURCE: &str = r#"
    data Choice [copy] { case First; case Second; case Fallback; }
    data Holder { value: u64; }
    data Bag { holders: [Holder; 3]; }
    machine pick(bag: &mut Bag, choice: Choice) -> &mut Holder {
        let slots: &mut [Holder] = bag.holders.as_mut_slice();
        transition choice {
            Choice::First -> &mut slots[0]
            Choice::Second -> &mut slots[1]
            _ -> &mut slots[0]
        }
    }
"#;

#[test]
fn enum_view_reference_returns_replay_sources_and_cleanup() {
    let attached = SOURCE
        .replace(
            "machine pick(bag: &mut Bag,",
            "machine Bag::pick(&mut self,",
        )
        .replace("bag.holders", "self.holders");
    for (source, name) in [(SOURCE, "pick"), (attached.as_str(), "Bag::pick")] {
        let checked = crate::front_end::checked_program(source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("selected source machine")
            .symbol;
        let plan_index = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .position(|plan| plan.machine == machine)
            .expect("enum returns retain the composed plan");
        let plan = &checked.facts.flow.terminal_unit_effects.composed_machines[plan_index];
        let state_index = plan
            .states
            .iter()
            .position(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .expect("enum dispatch retains its closed-case terminator");
        let state = &plan.states[state_index];
        let view_index = state
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        discard_result_on_return: true,
                        ..
                    }
                )
            })
            .expect("all-return arms dispose the affine view carrier");
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name(name))
            .expect("enum-selected reference retains its source and cleans up its view");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("every return has exact reference custody and affine cleanup");
        for corruption in [
            "missing cleanup",
            "source and receipt",
            "access",
            "missing case",
            "missing candidate",
            "duplicate candidate",
            "extra candidate",
        ] {
            let mut changed = checked.clone();
            let plan = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index];
            let state = &mut plan.states[state_index];
            if corruption == "missing candidate" {
                plan.result_reference_sources.pop().expect("candidate");
            } else if corruption == "duplicate candidate" {
                plan.result_reference_sources
                    .push(plan.result_reference_sources[0].clone());
            } else if corruption == "extra candidate" {
                let mut candidate = plan.result_reference_sources[0].clone();
                let Some(CheckedUnitStructuralPathSegment::FixedIndex(index)) =
                    candidate.source.path.last_mut()
                else {
                    panic!("exact element candidate")
                };
                *index = 2;
                plan.result_reference_sources.push(candidate);
            } else if corruption == "missing cleanup" {
                let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    discard_result_on_return,
                    ..
                } = &mut state.operations[view_index]
                else {
                    unreachable!()
                };
                *discard_result_on_return = false;
            } else {
                let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
                    &mut state.terminator
                else {
                    unreachable!()
                };
                if corruption == "missing case" {
                    cases.pop().expect("fallback case");
                } else {
                    // Select the unique second-element arm: changing both it
                    // and its receipt leaves a complete, well-typed candidate
                    // roster, so only authored replay can reject substitution.
                    let selected = cases.iter_mut().find(|case| matches!(
                        &case.outcome,
                        CheckedClosedSumCaseOutcomePlan::Return(
                            CheckedConditionalReturnArm::Structural(
                                CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
                            )
                        ) if source.path.last() == Some(&CheckedUnitStructuralPathSegment::FixedIndex(1))
                    )).expect("unique second-element return");
                    let CheckedClosedSumCaseOutcomePlan::Return(
                        CheckedConditionalReturnArm::Structural(
                            CheckedUnitEffectOperationPlan::EstablishReference { source, .. },
                        ),
                    ) = &mut selected.outcome
                    else {
                        panic!("reference return arm")
                    };
                    if corruption == "access" {
                        source.access = CheckedStructuralAccess::SharedBorrow;
                    } else {
                        let original = source.clone();
                        let Some(CheckedUnitStructuralPathSegment::FixedIndex(index)) =
                            source.path.last_mut()
                        else {
                            panic!("exact element path")
                        };
                        *index = 2;
                        let forged = source.clone();
                        let receipt = plan
                            .result_reference_sources
                            .iter_mut()
                            .find(|receipt| receipt.source == original)
                            .expect("returned source receipt");
                        receipt.source = forged;
                    }
                }
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name(name)).is_err(),
                "{name} must reject {corruption}"
            );
        }
    }
}
