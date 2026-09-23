//! A guarded case arm reads its payload through the case dispatch that selects
//! it: `Msg::Move { dx as step, .. } if step == 30 -> good(step)` observes `dx`
//! only as a parameter of the `StructuralCase` successor for `Move`, in the
//! guard and in the selected edge's argument alike.

use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralInputs,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

fn source(message: &str) -> String {
    format!(
        r#"
        boundary trait Sink {{ machine record(value: i32); }}
        data Msg {{
            case Move(dx: i32 [0..=50], dy: i32 [0..=50]);
            case Stop(code: i32);
        }}
        machine apply(m: Msg) reaches Sink {{
            transition m {{
                Msg::Move {{ dx as step, .. }} if step == 30 -> good(step)
                _ -> bad()
            }}
            state good(step: i32 [0..=50]) {{ Sink::record(step + 40); }}
            state bad() {{ Sink::record(1); }}
        }}
        data Main {{}}
        machine Main::main() reaches Sink {{
            apply({message});
        }}
    "#
    )
}

fn produce(
    checked: &checked_trees::CheckedTrees,
) -> Result<terminal_codec::CanonicalTerminalArtifact, String> {
    terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .map(|produced| produced.into_artifact())
    .map_err(|error| format!("{error:?}"))
}

fn recorded(artifact: &terminal_codec::CanonicalTerminalArtifact) -> Vec<i128> {
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("the verified artifact starts");
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    for _ in 0..512 {
        match execution
            .resume(&mut fuel, &mut AcceptTerminalEffects)
            .expect("execution resumes")
        {
            TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                return execution
                    .effects()
                    .iter()
                    .map(|effect| {
                        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                            panic!("only Sink recording effects");
                        };
                        let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice()
                        else {
                            panic!("one i32 recording argument");
                        };
                        match value {
                            semantic_vocabulary::IntegerValue::Signed(value) => *value,
                            other => panic!("signed recording argument, found {other:?}"),
                        }
                    })
                    .collect();
            }
            other => panic!("guarded case execution: {other:?}"),
        }
    }
    panic!("guarded case execution did not complete");
}

#[test]
fn guarded_arm_reads_its_payload_through_the_selecting_case_dispatch() {
    for (message, expected) in [
        ("Msg::Move { dx: 30, dy: 40 }", vec![70]),
        ("Msg::Move { dx: 20, dy: 40 }", vec![1]),
        ("Msg::Stop { code: 30 }", vec![1]),
    ] {
        let checked = crate::front_end::checked_program(&source(message));
        let artifact = produce(&checked)
            .unwrap_or_else(|error| panic!("{message}: payload guard lowers: {error}"));
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let blocks = || module.machines.iter().flat_map(|machine| &machine.blocks);
        // The payload is never read through a case-qualified place; the one
        // dispatch binds the scalar `Move` payload for the guard and the edge.
        assert!(
            blocks()
                .flat_map(|block| &block.operations)
                .all(|operation| !matches!(
                    &operation.kind,
                    terminal_psi::OperationKind::IntegerStructuralField { path, .. }
                        if path.iter().any(|segment| matches!(
                            segment,
                            semantic_vocabulary::CanonicalStructuralPathSegment::Case(_)
                        ))
                )),
            "{message}: no case-qualified runtime field read"
        );
        let dispatches = blocks()
            .filter_map(|block| match &block.terminator {
                terminal_psi::Terminator::StructuralCase { cases, .. } => Some(cases),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(dispatches.len(), 1, "{message}: one guard dispatch");
        assert_eq!(
            dispatches[0]
                .iter()
                .map(|case| case.payload_fields.len())
                .collect::<Vec<_>>(),
            vec![2, 0],
            "{message}: only the selected Move successor binds its payload"
        );
        assert_eq!(recorded(&artifact), expected, "{message}");
    }
}

#[test]
fn verifier_rejects_a_payload_bound_under_another_case() {
    // The payload's case knowledge is the verifier's own tag dispatch: moving
    // the `Move` payload binding onto the `Stop` successor must not verify.
    let checked = crate::front_end::checked_program(&source("Msg::Move { dx: 30, dy: 40 }"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        checked_trees_to_lowered_psi::TerminalMachineSelection::Name("Main::main"),
    )
    .expect("payload guard lowers");
    let verify = |module: &terminal_psi::TerminalModule| {
        terminal_verifier::verify_module(
            module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .map(|_| ())
    };
    verify(&lowered.semantic_module).expect("the exact dispatch verifies");
    let mut changed = lowered.semantic_module.clone();
    let cases = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::StructuralCase { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("the guard dispatch");
    let (selected, other) = cases.split_at_mut(1);
    std::mem::swap(&mut selected[0].target, &mut other[0].target);
    std::mem::swap(
        &mut selected[0].payload_fields,
        &mut other[0].payload_fields,
    );
    assert!(
        verify(&changed).is_err(),
        "a payload bound under another case has no case knowledge"
    );
}
