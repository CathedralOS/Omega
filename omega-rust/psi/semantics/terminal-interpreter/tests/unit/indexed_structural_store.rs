use super::*;

#[test]
fn canonical_indexed_store_rejects_bounds_and_carrier_path_corruption() {
    let field = StructuralPathSegment::Field("item".into());
    for path in [
        vec![field.clone(), StructuralPathSegment::FixedIndex(2)],
        vec![field.clone(), StructuralPathSegment::FixedIndex(u64::MAX)],
        vec![StructuralPathSegment::FixedIndex(0)],
        vec![
            field.clone(),
            StructuralPathSegment::FixedIndex(0),
            StructuralPathSegment::FixedIndex(1),
        ],
        vec![field.clone(), StructuralPathSegment::FixedIndex(0), field],
        vec![
            StructuralPathSegment::Field(String::new()),
            StructuralPathSegment::FixedIndex(0),
        ],
    ] {
        let mut module = indexed_store_call_module(0);
        let OperationKind::StructuralScalarFieldStore {
            path: destination, ..
        } = &mut module.machines[2].blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        *destination = path.clone();
        assert!(
            encode_module(&module).is_err(),
            "codec admitted invalid store path {path:?}"
        );
    }
}

#[test]
fn indexed_stores_survive_callee_return_and_every_fuel_pause() {
    for (index, expected) in [(0, 99), (1, 41)] {
        let module = indexed_store_call_module(index);
        let proof_bundle = ProofBundle::default();
        let verified = verify_module(&module, &proof_bundle, &AdmissionProfile::default())
            .expect("indexed store, return, and projected read verify");
        let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry)
            .expect("indexed stores and calls have fixed fuel");
        terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate)
            .expect("fixed-fuel evidence reconstructs independently");
        assert_eq!(certificate.ceiling_units(), 10);
        let semantic = encode_module(&module).expect("indexed stores encode");
        let proof = encode_proof_bundle(&proof_bundle).unwrap();
        drop(module);

        // Pause at each operation and return boundary, including after the
        // setter returns. Both elements share a type and field identity, so
        // losing the index would make the first read observe the second store.
        for allowance in 0..=10 {
            let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[],
                &[TerminalStructuralValue {
                    opaque_identity: 95,
                    structural_type: structural_type_id(95),
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
            )
            .expect("execution reconstructs the serialized artifact");
            let mut meter = TerminalFuelMeter::with_allowance(allowance);
            let status = execution.resume(&mut meter).unwrap();
            let result = if allowance < 10 {
                assert!(matches!(
                    status,
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                meter.replenish(10 - allowance).unwrap();
                execution.resume(&mut meter).unwrap()
            } else {
                status
            };
            assert_eq!(
                result,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        value: IntegerValue::Signed(expected),
                    }
                ))
            );
            assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
            for store in [operation_id(2), operation_id(7)] {
                assert_eq!(
                    meter
                        .usage()
                        .at(FuelChargeSite::Operation(store))
                        .unwrap()
                        .executions(),
                    1
                );
            }
        }
    }
}

fn indexed_store_call_module(read_index: u64) -> TerminalModule {
    let mut module = structural_scalar_field_call_module();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Structural(structural_type_id(97));
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(97),
        identity: "test::Items".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(96),
            length: 2,
        },
    });

    let mut setter = module.machines[0].clone();
    setter.id = machine_id(97);
    setter.result = TerminalMachineResult::Unit;
    setter.contract = empty_contract(contract_id(97));
    setter.entry = block_id(97);
    setter.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    setter.structural_parameters[0].place = place_id(97);
    setter.structural_places[0].id = place_id(97);
    let block = &mut setter.blocks[0];
    block.id = block_id(97);
    block.operations.truncate(2);
    let OperationKind::StructuralScalarFieldStore {
        destination, path, ..
    } = &mut block.operations[1].kind
    else {
        unreachable!()
    };
    *destination = place_id(97);
    path.push(StructuralPathSegment::FixedIndex(0));
    let mut second_constant = block.operations[0].clone();
    second_constant.id = operation_id(6);
    second_constant.result = OperationResult::Scalar(ValueDeclaration {
        id: value_id(6),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
    });
    second_constant.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(41),
    };
    let mut second_store = block.operations[1].clone();
    second_store.id = operation_id(7);
    let OperationKind::StructuralScalarFieldStore { path, value, .. } = &mut second_store.kind
    else {
        unreachable!()
    };
    path[1] = StructuralPathSegment::FixedIndex(1);
    *value = value_id(6);
    block.operations.extend([second_constant, second_store]);
    block.terminator = Terminator::ReturnUnit {
        edge: edge_id(97),
        trivial_affine_discards: Vec::new(),
    };

    let caller = &mut module.machines[0];
    caller.blocks[0].operations.drain(..2);
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(StructuralPathSegment::FixedIndex(read_index));
    caller.blocks[0].operations.insert(
        0,
        Operation {
            id: operation_id(5),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: setter.id,
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(95),
                    path: Vec::new(),
                    access: StructuralAccess::WriteOnlyBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    );
    module.machines.push(setter);
    module
}
