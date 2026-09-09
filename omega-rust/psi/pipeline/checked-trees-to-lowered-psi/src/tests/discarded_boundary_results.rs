//! Explicit result discard belongs to the boundary's immediate normal continuation.

use super::*;
use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralValue,
};

const SOURCE: &str = r#"
    data ReadResult { case Empty; case Bytes(count: u64); }
    boundary trait Host {
        machine read(buffer: &mut [u8]) -> ReadResult reaches Host;
        machine mark(value: u64) reaches Host;
    }
    machine run(buffer: &mut [u8]) reaches Host {
        Host::mark(1);
        _ = Host::read(buffer);
        Host::mark(2);
    }
"#;

fn run_symbol(checked: &CheckedTrees) -> SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("authored run machine")
        .symbol
}

#[test]
fn discarded_boundary_result_replays_immediate_cleanup_and_canonical_artifact() {
    let checked = checked_source(SOURCE);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(run_symbol(&checked))
        .expect("ordinary Unit source plan");
    let [
        CheckedUnitEffectOperationPlan::BoundaryCall { .. },
        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            result,
            discard_result_on_return,
            ..
        },
        CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate: cleanup_coordinate,
            affine_discards,
        },
        CheckedUnitEffectOperationPlan::BoundaryCall { .. },
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = plan.operations.as_slice()
    else {
        panic!(
            "mark, structural read, immediate cleanup, mark: {:?}",
            plan.operations
        );
    };
    assert_eq!(coordinate, cleanup_coordinate);
    assert!(!discard_result_on_return);
    assert_eq!(result.statement_index, 1);
    assert_eq!(result.type_identity, "named(name(ReadResult))");
    assert_eq!(affine_discards.len(), 1);
    assert_eq!(affine_discards[0].type_identity, result.type_identity);
    assert!(affine_discards[0].path.is_empty());
    assert_eq!(
        affine_discards[0].source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        }
    );

    let lowered = lower_machine(&checked, "run").expect("explicit discard lowers");
    let artifact = produce_terminal_artifact(&checked, "run").expect("publish checked discard");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent artifact verification");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let (block, returned) = entry
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find_map(|operation| match &operation.result {
                    terminal_psi::OperationResult::Structural(result) => Some((block, result)),
                    _ => None,
                })
        })
        .expect("boundary produces a real structural result");
    let terminal_psi::Terminator::Jump {
        target,
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("discard occurs on the immediate normal continuation");
    };
    assert_eq!(trivial_affine_discards, &[returned.place]);
    assert!(residual_affine_discards.is_empty());
    let continuation = entry
        .blocks
        .iter()
        .find(|block| block.id == *target)
        .unwrap();
    assert!(continuation.operations.iter().any(|operation| matches!(
        operation.kind,
        terminal_psi::OperationKind::BoundaryCall { .. }
    )));
    assert!(matches!(&continuation.terminator,
        terminal_psi::Terminator::ReturnUnit { trivial_affine_discards, .. }
        if trivial_affine_discards.is_empty()));
}

#[test]
fn discarded_boundary_result_rejects_missing_authored_discard_marker() {
    let mut checked = checked_source(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == run_symbol(&checked))
        .unwrap();
    let statements = checked.machine_states(machine)[0].statement_nodes;
    let typed_trees::statement::StatementNode::Call(call) =
        &mut checked.typed.statement_table.statements_mut(statements)[1]
    else {
        panic!("authored discard call");
    };
    assert!(call.discards_result);
    call.discards_result = false;
    assert!(lower_machine(&checked, "run").is_err());
    assert!(produce_terminal_artifact(&checked, "run").is_err());
}

#[test]
fn discarded_boundary_result_rejects_result_cleanup_and_later_operand_drift() {
    let original = checked_source(SOURCE);
    for mutation in [
        "result type",
        "result ordinal",
        "result statement",
        "return discard",
        "missing cleanup",
        "replace immediate cleanup with return discard",
        "duplicate cleanup",
        "delayed cleanup",
        "cleanup ordinal",
        "cleanup type",
        "cleanup coordinate",
        "later operand",
    ] {
        let mut checked = original.clone();
        let symbol = run_symbol(&checked);
        let operations = &mut checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == symbol)
            .unwrap()
            .operations;
        match mutation {
            "result type" | "result ordinal" | "result statement" | "return discard" => {
                let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } = &mut operations[1]
                else {
                    panic!("structural read");
                };
                match mutation {
                    "result type" => result.type_identity = "Host".to_owned(),
                    "result ordinal" => result.binding_ordinal += 1,
                    "result statement" => result.statement_index += 1,
                    "return discard" => *discard_result_on_return = true,
                    _ => unreachable!(),
                }
            }
            "missing cleanup" => {
                operations.remove(2);
            }
            "replace immediate cleanup with return discard" => {
                operations.remove(2);
                let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    discard_result_on_return,
                    ..
                } = &mut operations[1]
                else {
                    panic!("structural read");
                };
                *discard_result_on_return = true;
            }
            "duplicate cleanup" => {
                operations.insert(3, operations[2].clone());
            }
            "delayed cleanup" => {
                operations.swap(2, 3);
            }
            "cleanup ordinal" | "cleanup type" | "cleanup coordinate" => {
                let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards,
                } = &mut operations[2]
                else {
                    panic!("immediate cleanup");
                };
                match mutation {
                    "cleanup ordinal" => {
                        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        } = &mut affine_discards[0].source else { panic!("result root"); };
                        *binding_ordinal += 1;
                    }
                    "cleanup type" => affine_discards[0].type_identity = "Host".to_owned(),
                    "cleanup coordinate" => coordinate.statement_index += 1,
                    _ => unreachable!(),
                }
            }
            "later operand" => {
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &operations[0]
                else {
                    panic!("first marker");
                };
                let substituted = scalar_arguments.clone();
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &mut operations[3]
                else {
                    panic!("last marker");
                };
                *scalar_arguments = substituted;
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&checked, "run").is_err(),
            "source drift: {mutation}"
        );
        assert!(
            produce_terminal_artifact(&checked, "run").is_err(),
            "publication drift: {mutation}"
        );
    }
}

#[test]
fn discarded_boundary_result_cannot_replace_a_same_typed_live_operand() {
    let mut checked = checked_source(
        r#"
        data ReadResult { case Empty; case Bytes(count: u64); }
        boundary trait Host {
            machine read() -> ReadResult reaches Host;
            machine inspect(value: &ReadResult) reaches Host;
        }
        machine run(other: &ReadResult) reaches Host {
            _ = Host::read();
            Host::inspect(other);
        }
    "#,
    );
    let _artifact = produce_terminal_artifact(&checked, "run")
        .expect("independent live operand survives discard");
    let symbol = run_symbol(&checked);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    let CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } = &plan.operations[0]
    else {
        panic!("discard producer");
    };
    let result = result.clone();
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &mut plan.operations[2]
    else {
        panic!("live operand consumer");
    };
    assert_eq!(structural_arguments[0].type_identity, result.type_identity);
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        };
    assert!(produce_terminal_artifact(&checked, "run").is_err());
}

#[test]
fn discarded_boundary_result_preserves_effect_order_across_every_fuel_pause() {
    // Exercise result custody independently of host buffer realization. The
    // artifact test above retains the borrowed-buffer signature; arbitrary host
    // callbacks over raw arrays are not the selected checked-provider route.
    let source = SOURCE
        .replace("buffer: &mut [u8]", "")
        .replace("Host::read(buffer)", "Host::read()");
    let checked = checked_source(&source);
    let artifact = produce_terminal_artifact(&checked, "run").expect("source discard caller");
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    struct Host {
        events: Vec<u64>,
    }
    impl TerminalEffectHandler for Host {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("exact boundary result handler required")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                arguments, result, ..
            } = effect
            else {
                panic!("Host boundary");
            };
            match result {
                terminal_psi::BoundaryMachineResult::Unit => {
                    assert!(buffers.is_empty());
                    let [
                        TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(value),
                            ..
                        },
                    ] = arguments.as_slice()
                    else {
                        panic!("u64 marker");
                    };
                    self.events.push(u64::try_from(*value).unwrap());
                    Ok(TerminalEffectResult::Unit)
                }
                terminal_psi::BoundaryMachineResult::Structural(result) => {
                    assert!(arguments.is_empty());
                    assert!(buffers.is_empty());
                    self.events.push(3);
                    // Discard observes no tag/payload. Opaque structural custody is enough.
                    Ok(TerminalEffectResult::Structural(TerminalStructuralValue {
                        opaque_identity: 99,
                        structural_type: result.structural_type,
                        qualifications: result.qualifications.clone(),
                        path: Vec::new(),
                    }))
                }
                _ => panic!("no scalar read result"),
            }
        }
    }
    let mut host = Host { events: Vec::new() };
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut completed = false;
    for _ in 0..256 {
        match execution
            .resume_with_effect_handler(&mut fuel, &mut host)
            .unwrap_or_else(|error| panic!("{error:?}, observed host events: {:?}", host.events))
        {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let events = host.events.clone();
                let effects = execution.effects().to_vec();
                assert!(matches!(
                    execution
                        .resume_with_effect_handler(&mut fuel, &mut host)
                        .unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(
                    host.events, events,
                    "exhausted resume cannot replay host work"
                );
                assert_eq!(execution.effects(), effects.as_slice());
                fuel.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                completed = true;
                break;
            }
            other => panic!("unexpected execution status: {other:?}"),
        }
    }
    assert!(completed, "bounded test execution must complete");
    assert_eq!(host.events, [1, 3, 2]);
    assert_eq!(execution.effects().len(), 3);
}
