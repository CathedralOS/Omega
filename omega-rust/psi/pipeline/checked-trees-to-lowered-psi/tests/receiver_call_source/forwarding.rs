//! Transitive receiver retention and provisional shared-receiver erasure.

use super::*;

#[test]
fn transitive_write_only_self_calls_retain_receivers_in_every_declaration_order() {
    let declarations = [
        "machine Record::outer(&write self) { self.forward(); }",
        "machine Record::forward(&write self) { self.replace(); }",
        "machine Record::replace(&write self) { self.value = 17; }",
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let source = format!(
            "data Record {{ value: u16; }}\n{}\n{}\n{}",
            declarations[order[0]], declarations[order[1]], declarations[order[2]],
        );
        let checked = checked_from_source(&source);
        let plans = [
            unit_plan(&checked, "Record::outer"),
            unit_plan(&checked, "Record::forward"),
            unit_plan(&checked, "Record::replace"),
        ];
        for plan in plans {
            let [receiver] = plan.structural_parameters.as_slice() else {
                panic!("every machine in the chain retains exactly one receiver: {order:?}")
            };
            assert!(receiver.is_self);
            assert_eq!(receiver.position, 0);
            assert_eq!(receiver.access, CheckedStructuralAccess::WriteOnlyBorrow);
            assert_eq!(
                receiver.type_identity,
                plans[2].structural_parameters[0].type_identity
            );
            assert!(plan.scalar_parameters.is_empty());
        }
        for pair in plans.windows(2) {
            let [
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    ..
                },
                CheckedUnitEffectOperationPlan::ReturnUnit { .. },
            ] = pair[0].operations.as_slice()
            else {
                panic!("forwarding plan retains its exact call and return: {order:?}")
            };
            assert_eq!(*target_machine, pair[1].machine);
            assert!(scalar_arguments.is_empty());
            let [argument] = structural_arguments.as_slice() else {
                panic!("each call forwards exactly one receiver: {order:?}")
            };
            assert_eq!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
            );
            assert!(argument.path.is_empty());
            assert_eq!(argument.access, CheckedStructuralAccess::WriteOnlyBorrow);
            assert_eq!(
                argument.type_identity,
                pair[0].structural_parameters[0].type_identity
            );
        }

        let artifact = terminal_production::produce_terminal_artifact(&checked, "Record::outer")
            .expect("transitive receiver chain reaches canonical Terminal production");
        drop(checked);
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        assert_eq!(
            terminal_codec::encode_module(&module).unwrap(),
            artifact.semantic_bytes()
        );
        let profile = proof_admission::AdmissionProfile::default();
        let verified = terminal_verifier::verify_module(&module, &proof, &profile)
            .expect("decoded transitive receiver chain independently verifies");
        let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry)
            .expect("transitive receiver chain has fixed fuel");
        assert_eq!(module.machines.len(), 3);
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("outer entry");
        let [entry_receiver] = entry.structural_parameters.as_slice() else {
            panic!("outer retains exactly one Terminal receiver")
        };
        let mut machine = entry;
        let mut executed_operations = Vec::new();
        for depth in 0..3 {
            let [receiver] = machine.structural_parameters.as_slice() else {
                panic!("each transitive callee retains exactly one Terminal receiver")
            };
            assert!(receiver.is_self);
            assert_eq!(receiver.position, 0);
            assert_eq!(receiver.access, StructuralAccess::WriteOnlyBorrow);
            assert_eq!(receiver.structural_type, entry_receiver.structural_type);
            assert_eq!(receiver.multiplicity, StructuralMultiplicity::Unrestricted);
            assert!(machine.parameters.is_empty());
            assert_eq!(machine.result, TerminalMachineResult::Unit);
            let [block] = machine.blocks.as_slice() else {
                panic!("one chain block")
            };
            assert!(matches!(block.terminator, Terminator::ReturnUnit { .. }));
            if depth < 2 {
                let [call] = block.operations.as_slice() else {
                    panic!("one forwarding call")
                };
                let OperationKind::CallUnit {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } = &call.kind
                else {
                    panic!("outer and forward each invoke the next Unit machine")
                };
                assert!(arguments.is_empty());
                let [argument] = structural_arguments.as_slice() else {
                    panic!("one forwarded receiver")
                };
                assert_eq!(argument.place, receiver.place);
                assert!(argument.path.is_empty());
                assert_eq!(argument.access, StructuralAccess::WriteOnlyBorrow);
                executed_operations.push(call.id);
                machine = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == *callee)
                    .expect("retained chain target");
            } else {
                let [constant, store] = block.operations.as_slice() else {
                    panic!("leaf literal and store")
                };
                assert_eq!(
                    constant.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(17)
                    }
                );
                let OperationResult::Scalar(result) = constant.result else {
                    panic!("literal scalar")
                };
                let OperationKind::StructuralScalarFieldStore {
                    destination,
                    path,
                    value,
                    ..
                } = &store.kind
                else {
                    panic!("replace retains its actual receiver store")
                };
                assert_eq!(*destination, receiver.place);
                assert!(path.is_empty());
                assert_eq!(*value, result.id);
                executed_operations.push(store.id);
            }
        }

        // Opaque structural arguments support store execution; completion does
        // not expose a public post-return field value or establish native execution.
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: entry_receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .expect("outer accepts its supplied interpreter receiver");
        let mut meter =
            terminal_fuel::TerminalFuelMeter::with_allowance(certificate.ceiling_units());
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
        for operation in executed_operations {
            assert_eq!(
                meter
                    .usage()
                    .at(terminal_fuel::FuelChargeSite::Operation(operation))
                    .expect("both forwarding calls and leaf store execute")
                    .executions(),
                1
            );
        }
    }
}

#[test]
fn empty_shared_receiver_callee_keeps_provisional_self_erased() {
    let callee_source = "machine Record::noop(&self) {}";
    let caller_source = "machine invoke(destination: &Record) { destination.noop(); }";
    for caller_first in [false, true] {
        let source = if caller_first {
            format!("data Record {{ value: u16; }}\n{caller_source}\n{callee_source}")
        } else {
            format!("data Record {{ value: u16; }}\n{callee_source}\n{caller_source}")
        };
        let checked = checked_from_source(&source);
        let caller = unit_plan(&checked, "invoke");
        let callee = unit_plan(&checked, "Record::noop");
        assert!(callee.structural_parameters.is_empty());
        let [receiver] = caller.structural_parameters.as_slice() else {
            panic!("caller retains its explicitly declared shared reference parameter")
        };
        assert!(!receiver.is_self);
        assert_eq!(receiver.access, CheckedStructuralAccess::SharedBorrow);
        let [
            CheckedUnitEffectOperationPlan::CallUnit {
                target_machine,
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = caller.operations.as_slice()
        else {
            panic!("shared noop call and return")
        };
        assert_eq!(*target_machine, callee.machine);
        assert!(structural_arguments.is_empty());

        let artifact = terminal_production::produce_terminal_artifact(&checked, "invoke")
            .expect("erased shared noop receiver reaches Terminal production");
        drop(checked);
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("erased shared noop receiver independently verifies");
        let caller = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("shared caller entry");
        let [block] = caller.blocks.as_slice() else {
            panic!("one shared caller block")
        };
        let [call] = block.operations.as_slice() else {
            panic!("one shared noop call")
        };
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            ..
        } = &call.kind
        else {
            panic!("shared noop remains an ordinary Unit call")
        };
        assert!(structural_arguments.is_empty());
        let callee = module
            .machines
            .iter()
            .find(|machine| machine.id == *callee)
            .expect("shared noop target");
        assert!(callee.structural_parameters.is_empty());
        assert!(callee.parameters.is_empty());
        assert_eq!(callee.result, TerminalMachineResult::Unit);
        let [block] = callee.blocks.as_slice() else {
            panic!("one noop block")
        };
        assert!(block.operations.is_empty());
        assert!(matches!(block.terminator, Terminator::ReturnUnit { .. }));
    }
}
