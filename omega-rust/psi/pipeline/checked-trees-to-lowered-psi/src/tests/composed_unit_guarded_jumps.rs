//! Ordered guarded jump chains lower to nested decisions whose guards evaluate
//! in authored order, with the wildcard transition retained as the fallback.

use super::{checked_source, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::CheckedComposedUnitControlTerminatorPlan;

#[test]
fn ordered_literal_dispatch_selects_each_arm_and_the_fallback() {
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
        TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralInputs,
    };

    let checked = checked_source(
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
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_artifact()
        .expect("ordered literal dispatch publishes one terminal artifact");

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
    let mut corrupted = checked_source(
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
