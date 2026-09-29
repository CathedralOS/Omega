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

const DISPATCH: &str = r#"
    data Cmd { tag: u64; }
    data Msg { case A(cmd: Cmd); case B; }
    data Answer { n: u64; }
    machine param_subject(m: Msg) -> Answer {
        transition m {
            Msg::A { cmd } -> hop(cmd)
            Msg::B -> (Answer { n: 0 })
        }
        state hop(cmd: Cmd) -> Answer { (Answer { n: cmd.tag }) }
    }
    machine local_subject(tag: u64) -> Answer {
        let m: Msg = Msg::A { cmd: Cmd { tag: tag } };
        transition m {
            Msg::A { cmd } -> hop(cmd)
            Msg::B -> (Answer { n: 0 })
        }
        state hop(cmd: Cmd) -> Answer { (Answer { n: cmd.tag }) }
    }
"#;

fn verified_dispatch(machine: &str) -> terminal_psi::TerminalModule {
    let checked = crate::front_end::checked_program(DISPATCH);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name(machine),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("{machine} produces a terminal artifact: {error:?}"))
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{machine} verifies: {error:?}"));
    module
}

/// The case dispatch consumes its subject once: along each arm's path —
/// the outer edge plus the staged forwarding jumps ahead of it — the
/// subject's place is released exactly once, on the edge where control
/// leaves the dispatch's custody.
fn assert_dispatch_arms_share_subject_disposal(module: &terminal_psi::TerminalModule) {
    let machines = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => Some((&machine.blocks, when_true, when_false)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (blocks, when_true, when_false) in &machines {
        let released = |edge: &terminal_psi::SuccessorEdge| {
            let mut released = edge.trivial_affine_discards.clone();
            let mut target = edge.target;
            while let Some(block) = blocks.iter().find(|block| block.id == target) {
                match &block.terminator {
                    Terminator::Jump {
                        target: next,
                        trivial_affine_discards,
                        ..
                    } => {
                        released.extend(trivial_affine_discards.iter().copied());
                        target = *next;
                    }
                    _ => break,
                }
            }
            released
        };
        assert_eq!(
            released(when_true),
            released(when_false),
            "each dispatch arm's path must release the same subject place"
        );
        assert!(
            !released(when_true).is_empty(),
            "the dispatch arms must release their subject"
        );
    }
}

#[test]
fn value_arm_edge_disposes_parameter_subject() {
    assert_dispatch_arms_share_subject_disposal(&verified_dispatch("param_subject"));
}

#[test]
fn value_arm_edge_disposes_local_subject() {
    assert_dispatch_arms_share_subject_disposal(&verified_dispatch("local_subject"));
}
