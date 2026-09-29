//! A conditional successor may name an authored `(expression)` target instead
//! of a named state: that arm returns the established value rather than
//! transferring control, and the verifier must accept the emitted module.
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::Terminator;

const SOURCE: &str = r#"
    data Answer { value: u64; }
    machine pick(b: bool) -> Answer {
        transition b {
            true -> named()
            _ -> (Answer { value: 7 })
        }
        state named() -> Answer { Answer { value: 3 } }
    }
    machine pick_reversed(b: bool) -> Answer {
        transition b {
            true -> (Answer { value: 7 })
            _ -> named()
        }
        state named() -> Answer { Answer { value: 3 } }
    }
    machine pick_bounded(b: bool, position: u64) -> Answer {
        transition b && position < 4 {
            true -> named()
            _ -> (Answer { value: 0 })
        }
        state named() -> Answer { Answer { value: 3 } }
    }
    data Plan { frontier: u64; }
    data Kernel [copy] { case SimdLinear; case ScalarLinear; case ScalarBinary; }
    machine Plan::count(&self) -> u64 { self.frontier }
    machine Kernel::select(plan: &Plan) -> Kernel {
        transition plan.count() <= 8 {
            true -> (Kernel::SimdLinear)
            _ -> choose(plan)
        }
        state choose(plan: &Plan) -> Kernel {
            transition plan.count() <= 64 {
                true -> (Kernel::ScalarLinear)
                _ -> (Kernel::ScalarBinary)
            }
        }
    }
"#;

fn produce(machine: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let checked = crate::front_end::checked_program(SOURCE);
    terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name(machine),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact()
}

fn verified_module(machine: &str) -> terminal_psi::TerminalModule {
    let artifact = produce(machine);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    module
}

/// Some control edge must reach a block closed by `ReturnStructural`, since
/// each machine has exactly one authored `(expression)` arm.
fn assert_branch_reaches_return(module: &terminal_psi::TerminalModule) {
    let return_blocks = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .filter(|block| matches!(block.terminator, Terminator::ReturnStructural { .. }))
        .map(|block| block.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        !return_blocks.is_empty(),
        "the authored `(expression)` arm must return its value"
    );
    let edge_targets = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| {
            let mut targets = Vec::new();
            match &block.terminator {
                Terminator::Jump { target, .. } => targets.push(*target),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    targets.push(when_true.target);
                    targets.push(when_false.target);
                }
                Terminator::StructuralCase { cases, .. } => {
                    targets.extend(cases.iter().map(|case| case.target));
                }
                _ => {}
            }
            targets
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        return_blocks.intersection(&edge_targets).next().is_some(),
        "a conditional arm must transfer control into a return block"
    );
}

#[test]
fn conditional_case_arm_returns_while_borrow_transfers() {
    assert_branch_reaches_return(&verified_module("Kernel::select"));
}

const UNIT_SOURCE: &str = r#"
    data Output { value: i32; }
    machine maybe_write(flag: bool, output: &mut Output) {
        output.value = 5;
        transition flag {
            true -> write(output)
            false -> {}
        }
        state write(output: &mut Output) { output.value = 7; }
    }
    machine maybe_write_reversed(flag: bool, output: &mut Output) {
        output.value = 5;
        transition flag {
            true -> {}
            false -> write(output)
        }
        state write(output: &mut Output) { output.value = 7; }
    }
    data Payload { value: i32; }
    machine drop_or_forward(flag: bool, output: &mut Output) {
        output.value = 5;
        let payload: Payload = Payload { value: 7 };
        transition flag {
            true -> {}
            false -> write(output, payload)
        }
        state write(output: &mut Output, payload: Payload) {
            output.value = payload.value;
        }
    }
"#;

#[test]
fn conditional_unit_returns_keep_both_arm_positions_and_borrow_custody() {
    let checked = crate::front_end::checked_program(UNIT_SOURCE);
    for name in ["maybe_write", "maybe_write_reversed", "drop_or_forward"] {
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name(name),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("conditional Unit return must publish")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("Unit completion preserves loan custody");
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .any(|block| matches!(block.terminator, Terminator::Conditional { .. }))
        );
    }
}

#[test]
fn conditional_unit_return_rejects_changed_coordinate_or_arm_position() {
    for change_position in [false, true] {
        let mut checked = crate::front_end::checked_program(UNIT_SOURCE);
        let machine = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .find(|graph| checked.symbols.display_path(graph.machine, "::") == "maybe_write")
            .expect("conditional graph")
            .machine;
        let graph = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|graph| graph.machine == machine)
            .expect("conditional graph");
        let checked_trees::CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
            return_arm: checked_trees::CheckedConditionalReturnArm::Unit { statement_ordinal },
            return_when_true,
            ..
        } = &mut graph.states[0].terminator
        else {
            panic!("Unit return arm");
        };
        if change_position {
            *return_when_true = !*return_when_true;
        } else {
            *statement_ordinal += 1;
        }
        assert!(
            super::lower_machine(
                &checked,
                crate::TerminalMachineSelection::Name("maybe_write"),
            )
            .is_err(),
            "forged Unit return reached publication"
        );
    }
}
