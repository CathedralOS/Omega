use super::*;
use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionStatus, TerminalStructuralByteArrayValue,
    TerminalStructuralValue,
};
use terminal_psi::{StructuralPathSegment, TerminalMachineResult};

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    data Reply { case Done; }
    machine probe(bytes: &[u8], marker: u8) -> Reply reaches Output {
        Output::write(bytes, marker);
        transition { _ -> finish() }
        state finish() -> Reply { Reply::Done }
    }
    data Root {}
    machine Root::enter() reaches Output {
        _ = probe("first", 1u8);
        Output::write("middle", 2u8);
        _ = probe("last", 3u8);
    }
"#;

#[test]
fn discarded_structural_calls_preserve_results_and_interleaved_effects() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace(
            "Output::write(\"middle\", 2u8);",
            "let marker: u8 = 2u8; Output::write(\"middle\", marker);",
        ),
        SOURCE.replace(
            "_ = probe(\"first\", 1u8);",
            "let ignored: Reply = probe(\"first\", 1u8);",
        ),
    ] {
        assert_eq!(
            effects(&source),
            [b"first".to_vec(), b"middle".to_vec(), b"last".to_vec()]
        );
    }
}

fn effects(source: &str) -> Vec<Vec<u8>> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked(source), "Root::enter")
        .expect("discard is a result disposition, not a Unit signature");
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("independently verified discarded results execute");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("output effect");
            };
            byte_sequence_arguments[0].clone().unwrap()
        })
        .collect()
}

#[test]
fn discarded_result_still_requires_exact_source_and_cleanup() {
    let baseline = checked(SOURCE);
    for mutation in 0..6 {
        let mut checked = baseline.clone();
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                })
            })
            .unwrap();
        let CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            result,
            discard_result_on_return,
            target_contract_commitment,
            ..
        } = &mut plan.operations[0]
        else {
            panic!("first call");
        };
        match mutation {
            0 => *discard_result_on_return = true,
            1 => result.binding_ordinal += 1,
            2 => coordinate.statement_index += 1,
            3 => {
                *target_contract_commitment =
                    checked_trees::MachineContractCommitment::from_digest([0x5a; 32])
            }
            4 => {
                let state = checked.typed.machine_states(
                    checked
                        .typed
                        .machines()
                        .iter()
                        .find(|machine| machine.symbol == plan.machine)
                        .unwrap(),
                )[0]
                .clone();
                let handle = state.statement_nodes.start();
                let checked_trees::statement::StatementNode::Call(call) =
                    checked.typed.statement_table.statement_mut(handle)
                else {
                    panic!("authored discard");
                };
                call.discards_result = false;
            }
            5 => {
                let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    affine_discards, ..
                } = &mut plan.operations[1]
                else {
                    panic!("immediate result cleanup");
                };
                affine_discards.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn discarded_boundary_result_retains_computed_arguments() {
    let source = r#"
        pub data Reply { case Done; }
        machine identity(value: u8) -> u8 { value }
        boundary trait Output {
            machine reply(marker: u8) -> Reply reaches Output;
            machine write(marker: u8) reaches Output;
        }
        data Root {}
        machine Root::enter() reaches Output {
            _ = Output::reply(identity(3u8));
            Output::write(4u8);
        }
    "#;
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked(source), "Root::enter").unwrap();
    let module =
        terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap()).unwrap();
    terminal_verifier::verify_module(&module, &lowered.proof_bundle, &AdmissionProfile::default())
        .unwrap();
}

const BUFFER_SOURCE: &str = r#"
        data Reply { case Done; }
        machine fill(out: &mut [u8]) -> Reply {
            transition out.len > 0 { true -> store(out) false -> done() }
            state store(out: &mut [u8]) -> Reply { out[0] = 42; Reply::Done }
            state done() -> Reply { Reply::Done }
        }
        data Root { buffer: [u8; 1]; }
        machine Root::enter(&mut self) { _ = fill(&mut self.buffer); }
    "#;

#[test]
fn discarded_graph_result_keeps_projected_storage_across_resumes() {
    assert_original_buffer_is_updated(BUFFER_SOURCE);
}

#[test]
fn projected_result_call_rejects_forged_paths_and_access() {
    let baseline =
        checked_trees_to_lowered_psi::lower_machine(&checked(BUFFER_SOURCE), "Root::enter")
            .unwrap()
            .semantic_module;
    terminal_verifier::validate_module(&baseline).unwrap();
    for mutation in 0..5 {
        let mut module = baseline.clone();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let argument = entry
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| {
                if let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                    structural_arguments,
                    ..
                } = &mut operation.kind
                {
                    Some(&mut structural_arguments[0])
                } else {
                    None
                }
            })
            .unwrap();
        match mutation {
            0 => argument.path = vec![StructuralPathSegment::Field("missing".into())],
            1 => argument.path.push(StructuralPathSegment::FixedIndex(0)),
            2 => argument.access = terminal_psi::StructuralAccess::SharedBorrow,
            3 => argument.access = terminal_psi::StructuralAccess::Owned,
            4 => {
                entry.structural_parameters[0].access = terminal_psi::StructuralAccess::SharedBorrow
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::validate_module(&module).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn graph_result_call_cannot_substitute_a_same_typed_mutable_input() {
    let source = r#"
        data Reply { case Done; }
        machine fill(out: &mut [u8]) -> Reply {
            transition out.len > 0 { true -> store(out) false -> done() }
            state store(out: &mut [u8]) -> Reply { out[0] = 42; Reply::Done }
            state done() -> Reply { Reply::Done }
        }
        data Root {}
        machine Root::enter(left: &mut [u8], right: &mut [u8]) {
            let reply: Reply = fill(left);
            transition reply { Reply::Done -> done() }
            state done() {}
        }
    "#;
    let mut checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
    terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let operation = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|machine| &mut machine.states)
        .flat_map(|state| &mut state.operations)
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCall { .. }
            )
        })
        .unwrap();
    let CheckedUnitEffectOperationPlan::StructuralCall {
        structural_arguments,
        ..
    } = operation
    else {
        unreachable!()
    };
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap_err();
    assert!(
        format!("{error:?}").contains("differs from its authored destination"),
        "{error:?}"
    );
}

#[test]
fn discarded_receiver_result_retains_forwarded_self() {
    let source = r#"
        data Reply { case Done; }
        data Root { buffer: [u8; 1]; }
        machine write_view(out: &mut [u8]) {
            transition out.len > 0 { true -> store(out) false -> done() }
            state store(out: &mut [u8]) { out[0] = 42; }
            state done() {}
        }
        machine Root::fill(&mut self) -> Reply {
            transition { _ -> store() }
            state store(&mut self) -> Reply { write_view(&mut self.buffer); Reply::Done }
        }
        machine Root::forward(&mut self) { _ = self.fill(); }
        machine Root::enter(&mut self) { self.forward(); }
    "#;
    assert_original_buffer_is_updated(source);
    assert_original_buffer_is_updated(&source.replace("self.forward();", "_ = self.fill();"));
}

fn assert_original_buffer_is_updated(source: &str) {
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked(source), "Root::enter").unwrap();
    let module = &lowered.semantic_module;
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(root.result, TerminalMachineResult::Unit);
    let parameter = &root.structural_parameters[0];
    let path = vec![StructuralPathSegment::Field("buffer".into())];
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
            &encode_module(module).unwrap(),
            &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
            &AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: parameter.structural_type,
                qualifications: parameter.qualifications.clone(),
                path: Vec::new(),
            }],
            &[TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: path.clone(),
                bytes: vec![19],
            }],
        )
        .unwrap();
    let mut fuel = TerminalFuelMeter::with_allowance(0);
    for _ in 0..256 {
        match execution.resume(&mut fuel).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                assert_eq!(execution.structural_byte_array(1, &path).unwrap(), &[42]);
                return;
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    panic!("graph must complete");
}
