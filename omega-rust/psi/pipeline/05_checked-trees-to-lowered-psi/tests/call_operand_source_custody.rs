use checked_trees::CheckedUnitEffectOperationPlan;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};

const BOUNDARY_SOURCE: &str = r#"
    boundary trait Host {
        machine measure(first: i32, second: i32) -> i32 reaches Host;
        machine finish(first: i32, second: i32) reaches Host;
    }
    data Main {}
    machine Main::main(left: i32, right: i32)
    reaches Host
    {
        let measured: i32 = Host::measure(left, right);
        Host::finish(measured, left);
    }
"#;

const UNIT_SOURCE: &str = r#"
    boundary trait Host {
        machine finish(first: i32, second: i32) reaches Host;
    }
    data Scalar {}
    machine Scalar::first(left: i32, right: i32) -> i32
    requires 0i32 == 0i32
    ensures result == left
    { left }
    data Sink {}
    machine Sink::finish(first: i32, second: i32)
    reaches Host
    { Host::finish(first, second); }
    data Main {}
    machine Main::main(left: i32, right: i32)
    reaches Host
    {
        let measured: i32 = Scalar::first(left, right);
        Sink::finish(measured, right);
    }
"#;

const BOUNDARY_RETURN_SOURCE: &str = r#"
    boundary trait Host {
        machine measure(first: i32, second: i32) -> i32 reaches Host;
    }
    data Main {}
    machine Main::main() -> i32
    reaches Host
    {
        let measured: i32 = Host::measure(23i32, 70i32);
        measured
    }
"#;

const CALLABLE_BOUNDARY_SOURCE: &str = r#"
    boundary trait Host {
        machine finish(first: i32, second: i32) reaches Host;
    }
    data Main {}
    machine Main::main(left: i32, right: i32)
    reaches Host
    {
        Host::finish(left, right);
    }
"#;

fn encoded(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("supported source lowers");
    let artifact = (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    );
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("fresh decoded verification");
    artifact
}

fn mutate_operation_custody(operation: &mut CheckedUnitEffectOperationPlan, mutation: u8) -> bool {
    let (coordinate, target_state, source_site) = match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_state,
            source_site,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            target_state,
            source_site,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            target_state,
            source_site,
            ..
        } => (coordinate, target_state, Some(source_site)),
        CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            target_state,
            ..
        }
        | CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_state,
            ..
        } => (coordinate, target_state, None),
        _ => return false,
    };
    match mutation {
        0 => coordinate.call_ordinal += 1,
        1 => *target_state = symbols::SymbolHandle::invalid(),
        2 | 3 => {
            let Some(source_site) = source_site else {
                return false;
            };
            *source_site = if mutation == 2 {
                None
            } else {
                Some(
                    match source_site.expect("baseline retains the authored boundary site") {
                        checked_trees::NominalMachineUseSite::Statement(handle) => {
                            checked_trees::NominalMachineUseSite::Statement(
                                arena::Handle::from_parts(
                                    handle.arena_index(),
                                    handle.generation() + 1,
                                ),
                            )
                        }
                        checked_trees::NominalMachineUseSite::Expression(handle) => {
                            checked_trees::NominalMachineUseSite::Expression(
                                arena::Handle::from_parts(
                                    handle.arena_index(),
                                    handle.generation() + 1,
                                ),
                            )
                        }
                    },
                )
            };
        }
        _ => unreachable!(),
    }
    true
}

fn assert_operation_custody(checked: &checked_trees::CheckedTrees) {
    let main = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap()
        .symbol;
    for (plan_index, plan) in checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .enumerate()
    {
        if plan.machine != main {
            continue;
        }
        for operation_index in 0..plan.operations.len() {
            for mutation in 0..4 {
                let mut changed = checked.clone();
                let operation = &mut changed.facts.flow.terminal_unit_effects.machines[plan_index]
                    .operations[operation_index];
                if mutate_operation_custody(operation, mutation) {
                    let operation = operation.clone();
                    assert!(
                        checked_trees_to_lowered_psi::lower_machine(
                            &changed,
                            TerminalMachineSelection::Name("Main::main")
                        )
                        .is_err(),
                        "operation={operation_index}, outer custody mutation={mutation}: {operation:?}"
                    );
                }
            }
        }
    }
    for (plan_index, plan) in checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .enumerate()
    {
        if plan.machine != main {
            continue;
        }
        for mutation in 0..4 {
            let mut changed = checked.clone();
            let operation = &mut changed.facts.flow.terminal_boundary_scalar_returns.machines
                [plan_index]
                .boundary_call;
            assert!(mutate_operation_custody(operation, mutation));
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("Main::main")
                )
                .is_err(),
                "boundary return outer custody mutation={mutation}"
            );
        }
    }
}

#[test]
fn checked_call_operations_reject_changed_ordinals_targets_and_boundary_sites() {
    for source in [
        BOUNDARY_SOURCE,
        UNIT_SOURCE,
        BOUNDARY_RETURN_SOURCE,
        CALLABLE_BOUNDARY_SOURCE,
    ] {
        let checked = crate::front_end::checked_program(source);
        encoded(&checked);
        assert_operation_custody(&checked);
    }
}
