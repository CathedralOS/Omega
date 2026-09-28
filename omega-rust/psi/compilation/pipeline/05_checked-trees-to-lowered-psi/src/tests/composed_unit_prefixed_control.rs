//! Scalar prefixes compose with ordinary graph edges and effect sequencing.

use super::{CheckedTrees, LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::CheckedComposedUnitControlTerminatorPlan;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn interleaved_states_preserve_mixed_handoffs_and_effect_order() {
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
        TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
        TerminalStructuralInputs,
    };

    let checked = crate::front_end::checked_program(
        r#"
            boundary trait Host { machine emit(value: i32); }
            data Root {}
            machine Root::enter(flag: bool, value: i32) reaches Host {
                Host::emit(1);
                transition { _ -> dispatch(flag, value) }
                state yes(value: i32) {
                    Host::emit(value);
                    transition { _ -> done() }
                }
                state dispatch(flag: bool, value: i32) {
                    Host::emit(2);
                    transition flag { true -> yes(value) _ -> no() }
                }
                state done() { Host::emit(3); }
                state no() {
                    Host::emit(4);
                    transition { _ -> done() }
                }
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
    .expect("mixed signatures and interleaved state declarations publish one graph")
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
                panic!("each boundary receives its one signed integer");
            };
            self.0.push(*value);
            Ok(())
        }
    }
    for (flag, expected) in [(true, vec![1, 2, 37, 3]), (false, vec![1, 2, 4, 3])] {
        let arguments = [
            TerminalScalarValue::Boolean(flag),
            TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .expect("i32 carrier"),
                value: semantic_vocabulary::IntegerValue::Signed(37),
            },
        ];
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &arguments,
            TerminalStructuralInputs::default(),
        )
        .expect("serialized graph independently checks");
        let mut trace = Trace::default();
        assert_eq!(
            execution
                .resume(
                    &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                    &mut trace
                )
                .expect("acyclic graph executes"),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
        );
        assert_eq!(trace.0, expected);
    }
    let mut corrupted = checked;
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut corrupted.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        panic!("entry forwards to the dispatch state");
    };
    successor.scalar_arguments.swap(0, 1);
    assert!(
        lower_machine(&corrupted, TerminalMachineSelection::Name("Root::enter")).is_err(),
        "mixed lanes cannot be swapped"
    );
}

fn checked_multi_prefixed_control() -> CheckedTrees {
    crate::front_end::checked_program(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(flag: bool) reaches Host {
                transition { _ -> relay(flag) }
                state relay(flag: bool) { transition { _ -> dispatch(flag) } }
                state dispatch(flag: bool) {
                    transition flag { true -> yes() _ -> no() }
                }
                state yes() { Host::exit(1); }
                state no() { Host::exit(2); }
            }
        "#,
    )
}

#[test]
fn multi_prefixed_control_rejects_second_edge_corruption() {
    let mut checked = checked_multi_prefixed_control();
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[1].terminator
    else {
        unreachable!()
    };
    successor.scalar_arguments[0].source =
        checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 1 };
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("Root::enter")),
        Err(LoweringError::Unsupported(_))
    ));
}
