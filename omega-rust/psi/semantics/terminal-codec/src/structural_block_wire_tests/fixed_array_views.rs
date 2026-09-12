use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use terminal_psi::{StructuralFieldType, StructuralPathSegment};

fn fixture(projected: bool) -> TerminalModule {
    let mut module = unit_byte_field_module();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: id(3),
            identity: "FixedOctets".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(4),
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: id(4),
            identity: "Octet".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
    ]);
    if projected {
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
            unreachable!()
        };
        fields[0].field_type = StructuralFieldType::Structural(id(3));
    } else {
        module.machines[0].structural_parameters[0].structural_type = id(3);
        argument(&mut module).path.clear();
    }
    module
}

fn argument(module: &mut TerminalModule) -> &mut StructuralArgument {
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    &mut structural_arguments[0]
}

fn boundary_fixture(mut module: TerminalModule) -> TerminalModule {
    let structural_arguments = vec![argument(&mut module).clone()];
    let callee = module.machines.pop().unwrap();
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            crash_routes: Vec::new(),
            id: id(1),
            identity: "Host::borrow_bytes".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: callee.structural_parameters,
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    module.machines[0].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary: id(1),
        arguments: Vec::new(),
        structural_arguments,
        completion_receipts: Vec::new(),
    };
    module
}

#[test]
fn fixed_byte_array_call_presentations_round_trip_real_array_and_paths() {
    for projected in [false, true] {
        for boundary in [false, true] {
            let module = fixture(projected);
            let module = if boundary {
                boundary_fixture(module)
            } else {
                module
            };
            let bytes = encode_module(&module).unwrap();
            assert_eq!(decode_module(&bytes), Ok(module.clone()));
            assert_eq!(
                encode_module(&decode_module(&bytes).unwrap()).unwrap(),
                bytes
            );
            assert_eq!(
                module.structural_types[2].shape,
                StructuralTypeShape::FixedArray {
                    element: id(4),
                    length: 3
                }
            );
        }
    }
}

#[test]
fn fixed_byte_array_call_presentations_hostile_bytes_replay_exact_access_type_and_path() {
    for mutation in 0..7 {
        let mut module = fixture(true);
        let bytes = encode_module(&module).expect("hostile fixture starts lawful");
        assert_eq!(decode_module(&bytes), Ok(module.clone()));
        match mutation {
            0 => {
                module.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap()),
                )
            }
            1 => {
                module.structural_types[3].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
                )
            }
            2 => {
                module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            3 => {
                module.machines[1].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            4 => argument(&mut module).path = vec![StructuralPathSegment::Field("missing".into())],
            5 => argument(&mut module)
                .path
                .push(StructuralPathSegment::FixedIndex(0)),
            _ => {
                let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape
                else {
                    unreachable!()
                };
                fields[0].relevance = terminal_psi::BindingRelevance::Erased;
            }
        }
        for (boundary, module) in [(false, module.clone()), (true, boundary_fixture(module))] {
            assert!(
                encode_module(&module).is_err(),
                "encode mutation {mutation}, boundary {boundary}"
            );
            let changed = crate::module_wire::encode_raw(&module).unwrap();
            assert!(
                decode_module(&changed).is_err(),
                "decode mutation {mutation}, boundary {boundary}"
            );
        }
    }
}

#[test]
fn fixed_byte_array_view_ledger_replays_the_exact_array_extent() {
    for projected in [false, true] {
        for boundary in [false, true] {
            let module = fixture(projected);
            let module = if boundary {
                boundary_fixture(module)
            } else {
                module
            };
            let trust = crate::current_terminal_trust_graph().unwrap();
            let ledger = crate::build_terminal_obligation_ledger(&module, &trust).unwrap();
            let bytes = crate::encode_terminal_obligation_ledger(&ledger).unwrap();
            let decoded = crate::decode_terminal_obligation_ledger(&bytes).unwrap();
            crate::validate_terminal_obligation_ledger(&decoded, &module, &trust).unwrap();
            let mut changed = module;
            let StructuralTypeShape::FixedArray { length, .. } =
                &mut changed.structural_types[2].shape
            else {
                unreachable!()
            };
            *length = 4;
            encode_module(&changed)
                .expect("a different array extent remains a lawful independent module");
            assert!(
                crate::validate_terminal_obligation_ledger(&decoded, &changed, &trust).is_err()
            );
        }
    }
}
