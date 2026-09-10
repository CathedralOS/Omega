use super::*;

fn transported_case() -> TerminalModule {
    let mut module = scalar_case_call_module();
    let caller = &mut module.machines[0];
    let mut dispatch = caller.blocks[0].terminator.clone();
    let Terminator::StructuralCase { source, cases } = &mut dispatch else {
        unreachable!()
    };
    *source = place_id(7);
    cases[0].trivial_affine_discards = vec![place_id(7)];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(7),
        kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
            block: block_id(4),
            position: 0,
        },
    });
    caller.blocks[0].terminator = jump(4);
    caller.blocks.push(Block {
        id: block_id(4),
        parameters: vec![],
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(7),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: vec![],
            projected_qualifications: vec![],
        }],
        operations: vec![],
        terminator: dispatch,
    });
    module
}

fn jump(edge: u64) -> Terminator {
    Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(4),
        arguments: vec![],
        structural_arguments: vec![StructuralArgument {
            place: place_id(3),
            path: vec![],
            access: StructuralAccess::Owned,
        }],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    }
}

#[test]
fn owned_case_block_parameter_preserves_call_payload_at_every_fuel_pause() {
    assert_payload_at_every_fuel_pause(&transported_case());
}

fn assert_payload_at_every_fuel_pause(module: &TerminalModule) {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let semantic = encode_module(module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    for (first, second) in [(17, 83), (u64::MAX, 0)] {
        let start = || {
            TerminalExecution::start_artifact(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[count(first), count(second)],
            )
            .unwrap()
        };
        let expected =
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(count(first)));
        let mut execution = start();
        let mut meter = TerminalFuelMeter::with_allowance(100);
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        let total = meter.usage().total_units();
        for split in 0..total {
            let mut execution = start();
            let mut meter = TerminalFuelMeter::with_allowance(split);
            assert!(matches!(
                execution.resume(&mut meter).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            meter.replenish(total - split).unwrap();
            assert_eq!(
                execution.resume(&mut meter).unwrap(),
                expected,
                "payload after fuel split {split}"
            );
            assert_eq!(meter.usage().total_units(), total);
            assert_eq!(
                meter
                    .usage()
                    .at(FuelChargeSite::Operation(operation_id(1)))
                    .unwrap()
                    .units(),
                1
            );
        }
    }
}

#[test]
fn unrestricted_case_copies_preserve_both_destinations_and_original() {
    let mut module = transported_case();
    for machine in &mut module.machines {
        if let TerminalMachineResult::Structural(result) = &mut machine.result {
            result.multiplicity = StructuralMultiplicity::Unrestricted;
        }
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                if let OperationResult::Structural(result) = &mut operation.result {
                    result.multiplicity = StructuralMultiplicity::Unrestricted;
                }
            }
        }
    }
    let caller = &mut module.machines[0];
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments.push(structural_arguments[0].clone());
    let dispatch = &mut caller.blocks[2];
    dispatch.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    let mut second = dispatch.structural_parameters[0].clone();
    second.place = place_id(8);
    second.position = 1;
    dispatch.structural_parameters.push(second);
    let Terminator::StructuralCase { cases, .. } = &mut dispatch.terminator else {
        unreachable!()
    };
    cases[0].trivial_affine_discards.clear();
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(8),
        kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
            block: block_id(4),
            position: 1,
        },
    });
    // Each destination and the original producer remains independently usable
    // after the same edge copied the result into both parameter positions.
    for selected in [7, 8, 3] {
        let Terminator::StructuralCase { source, .. } =
            &mut module.machines[0].blocks[2].terminator
        else {
            unreachable!()
        };
        *source = place_id(selected);
        assert_payload_at_every_fuel_pause(&module);
    }
}

#[test]
fn owned_case_block_parameter_rejects_forged_transfer_and_origin() {
    let module = transported_case();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    for corruption in 0..6 {
        let mut forged = module.clone();
        if corruption == 2 {
            let mut other = forged.structural_types[0].clone();
            other.id = structural_type_id(2);
            other.identity = "OtherNominalCase".into();
            forged.structural_types.push(other);
        }
        let caller = &mut forged.machines[0];
        match corruption {
            0 => {
                let Terminator::Jump {
                    structural_arguments,
                    ..
                } = &mut caller.blocks[0].terminator
                else {
                    unreachable!()
                };
                structural_arguments.push(structural_arguments[0].clone());
                let mut second = caller.blocks[2].structural_parameters[0].clone();
                second.place = place_id(8);
                second.position = 1;
                caller.blocks[2].structural_parameters.push(second);
                caller.structural_places.push(StructuralPlaceDeclaration {
                    id: place_id(8),
                    kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                        block: block_id(4),
                        position: 1,
                    },
                });
            }
            1 => {
                let Terminator::Jump {
                    trivial_affine_discards,
                    ..
                } = &mut caller.blocks[0].terminator
                else {
                    unreachable!()
                };
                trivial_affine_discards.push(place_id(3));
            }
            2 => caller.blocks[2].structural_parameters[0].structural_type = structural_type_id(2),
            3 => {
                caller.entry = block_id(5);
                let successor = |edge, target| SuccessorEdge {
                    edge: edge_id(edge),
                    target: block_id(target),
                    arguments: vec![],
                    structural_arguments: vec![],
                    trivial_affine_discards: vec![],
                };
                caller.blocks.push(Block {
                    id: block_id(5),
                    parameters: vec![],
                    structural_parameters: vec![],
                    operations: vec![Operation {
                        id: operation_id(90),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_id(90),
                            scalar_type: ScalarType::Boolean,
                            qualifications: Default::default(),
                        }),
                        kind: OperationKind::BooleanConstant { value: false },
                    }],
                    terminator: Terminator::Conditional {
                        condition: value_id(90),
                        when_true: successor(5, 1),
                        when_false: successor(6, 6),
                    },
                });
                caller.blocks.push(Block {
                    id: block_id(6),
                    parameters: vec![],
                    structural_parameters: vec![],
                    operations: vec![],
                    terminator: jump(7),
                });
            }
            4 => {
                let Terminator::StructuralCase { source, .. } = &mut caller.blocks[2].terminator
                else {
                    unreachable!()
                };
                *source = place_id(3);
            }
            5 => {
                caller
                    .structural_places
                    .iter_mut()
                    .find(|place| place.id == place_id(7))
                    .unwrap()
                    .kind = semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                    block: block_id(3),
                    position: 0,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &forged,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .is_err(),
            "forged owned transfer {corruption}"
        );
    }
}
