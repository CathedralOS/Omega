//! Ordered guarded jump chains lower to nested decisions whose guards evaluate
//! in authored order, with the wildcard transition retained as the fallback.
//! A graph guard with no pure Boolean form evaluates its checked computation
//! root once before either edge.

use super::lower_machine;
use crate::TerminalMachineSelection;
use checked_trees::CheckedComposedUnitControlTerminatorPlan;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn ordered_literal_dispatch_selects_each_arm_and_the_fallback() {
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
        TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralInputs,
    };

    let checked = crate::front_end::checked_program(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(choice: i32) reaches Host {
                transition choice {
                    1 -> one()
                    2 -> two()
                    3 -> three()
                    _ -> other()
                }
                state one() { Host::exit(21); }
                state two() { Host::exit(22); }
                state three() { Host::exit(23); }
                state other() { Host::exit(29); }
            }
        "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("ordered literal dispatch publishes one terminal artifact")
    .into_artifact();

    #[derive(Default)]
    struct Trace(Vec<i128>);
    impl TerminalEffectHandler for Trace {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("only authored boundary calls are observable");
            };
            let [
                TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Signed(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("each exit call receives its one signed status");
            };
            self.0.push(*value);
            Ok(())
        }
    }
    for (choice, expected) in [
        (1i128, vec![21]),
        (2, vec![22]),
        (3, vec![23]),
        (0, vec![29]),
        (4, vec![29]),
    ] {
        let arguments = [TerminalScalarValue::Integer {
            scalar_type: semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Signed,
                32,
            )
            .expect("i32 carrier"),
            value: semantic_vocabulary::IntegerValue::Signed(choice),
        }];
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &arguments,
            TerminalStructuralInputs::default(),
        )
        .expect("serialized dispatch graph independently checks");
        let mut trace = Trace::default();
        assert!(
            matches!(
                execution.resume(
                    &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                    &mut trace
                ),
                Ok(TerminalExecutionStatus::Complete(_))
            ),
            "choice {choice} must execute the dispatch graph to completion"
        );
        assert_eq!(
            trace.0, expected,
            "choice {choice} must select exactly one authored arm"
        );
    }
}

#[test]
fn stale_arm_guard_rejects_lowering() {
    let mut corrupted = crate::front_end::checked_program(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(choice: i32) reaches Host {
                transition choice {
                    1 -> one()
                    2 -> two()
                    _ -> other()
                }
                state one() { Host::exit(21); }
                state two() { Host::exit(22); }
                state other() { Host::exit(29); }
            }
        "#,
    );
    let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } =
        &mut corrupted.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        panic!("literal dispatch chain is an ordered guarded jump tail");
    };
    arms[0].guard = arms[1].guard.clone();
    assert!(
        lower_machine(&corrupted, TerminalMachineSelection::Name("Root::enter")).is_err(),
        "an arm guard that disagrees with the checked guarded-exit roster must reject"
    );
}

const SELECTED_FLOAT_GUARD: &str = r#"
    boundary operator < Float::less(left: f64, right: f64) -> bool;
    boundary trait Host { machine exit(code: i32); }
    data Root {}
    machine Root::enter(left: f64, right: f64) reaches Host {
        transition left < right {
            true -> below()
            _ -> above()
        }
        state below() { Host::exit(21); }
        state above() { Host::exit(22); }
    }
"#;

/// Source-to-Terminal custody for the selected comparison; Omega separately
/// rejoins these opaque commitments to the actual selected ProviderPlans.
fn selected_float_guard() -> checked_trees::CheckedTrees {
    let mut checked = crate::front_end::checked_program(SELECTED_FLOAT_GUARD);
    let handles = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let selected = checked.facts.operators.uses.get_mut(handle);
        selected.provider_plan_report_fingerprint = 7;
        selected.provider_plan_commitment =
            checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]);
    }
    checked
}

fn computed_guard(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedCallScalarArgument {
    let CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        panic!("the selected float guard is a two-way graph conditional");
    };
    guard
}

#[test]
fn selected_float_guard_evaluates_its_computation_root_once() {
    let mut checked = selected_float_guard();
    assert!(matches!(
        computed_guard(&mut checked),
        checked_trees::CheckedCallScalarArgument::Computation(_)
    ));
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("a computed graph guard lowers through the shared computation expander");
    assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 1);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::IeeeFloatCompare { .. }
            ))
            .count(),
        1,
        "the guard is evaluated once, before either successor edge"
    );
    let produced = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the composed guard's selected comparison custody publishes");
    let [published] = produced.boundary_operator_scope().occurrences() else {
        panic!("one exact published comparison occurrence");
    };
    assert_eq!(
        published.terminal_operation(),
        lowered.selected_ieee_float_comparison_occurrences[0].terminal_operation,
    );
}

#[test]
fn computed_guard_drift_rejects_lowering() {
    // A Boolean computation other than the coordinate's root: the authored
    // `(left < right) == true` subject's inner selected comparison is a real
    // Boolean node, but not the retained guard.
    let mut retargeted = selected_float_guard();
    let checked_trees::CheckedCallScalarArgument::Computation(root) =
        *computed_guard(&mut retargeted)
    else {
        panic!("the selected float guard is computed");
    };
    let computations = &retargeted.facts.values.scalar_computations;
    let checked_trees::CheckedScalarComputationKind::Apply { operands, .. } =
        computations.nodes.get(root).kind
    else {
        panic!("the guard root applies Boolean equality to the selected comparison");
    };
    let inner = computations.operands.span_or_empty(operands)[0];
    assert!(matches!(
        computations.nodes.get(inner).kind,
        checked_trees::CheckedScalarComputationKind::SelectedComparison { .. }
    ));
    *computed_guard(&mut retargeted) = checked_trees::CheckedCallScalarArgument::Computation(inner);
    assert!(
        lower_machine(&retargeted, TerminalMachineSelection::Name("Root::enter")).is_err(),
        "a guard naming another computation node must reject"
    );
    // A pure Boolean in place of the computed guard competes with no retained
    // pure expression and must not replace the selected comparison.
    let mut replaced = selected_float_guard();
    *computed_guard(&mut replaced) = checked_trees::CheckedCallScalarArgument::Pure(
        checked_trees::CheckedScalarExpression::Boolean(Box::new(
            checked_trees::CheckedBooleanExpression::Constant(true),
        )),
    );
    assert!(
        lower_machine(&replaced, TerminalMachineSelection::Name("Root::enter")).is_err(),
        "a pure guard replacing the retained computation must reject"
    );
}
