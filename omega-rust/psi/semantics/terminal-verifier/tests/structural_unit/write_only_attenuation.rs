//! Projected attenuation preserves write-only access and the bounded leaf shape.

use super::*;

fn indexed_attenuation_module() -> TerminalModule {
    let mut module = projected_unit_call_module();
    module.structural_domains.clear();
    module.boundary_machines.clear();
    module.services.clear();
    module.root_service_reach = Default::default();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    module.machines[1].blocks[0].operations.clear();
    for machine in &mut module.machines {
        machine.entry_claims.clear();
        machine.published_service_ceiling.clear();
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
        machine.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    }
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
    claim_transfers.clear();
    module
}

#[test]
fn indexed_mutable_root_can_lend_a_write_only_primitive() {
    let module = indexed_attenuation_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("an exact primitive subloan may attenuate mutable authority");
}

#[test]
fn indexed_write_only_attenuation_does_not_admit_whole_array_leaves() {
    let mut module = indexed_attenuation_module();
    module.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: structural_type_id(2),
        length: 1,
    };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentMultiplicityMismatch { .. })
    ));
}

fn record_receiver_module(path: &[StructuralPathSegment]) -> TerminalModule {
    let mut module = indexed_attenuation_module();
    module.structural_types.truncate(2);
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            identity: "value".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        }],
    };
    let mut referent = structural_type_id(1);
    for segment in path.iter().rev() {
        let id = structural_type_id(module.structural_types.len() as u64 + 1);
        let shape = match segment {
            StructuralPathSegment::Field(identity) => StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: identity.clone(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(referent),
                }],
            },
            StructuralPathSegment::FixedIndex(_) => StructuralTypeShape::FixedArray {
                element: referent,
                length: 2,
            },
        };
        module.structural_types.push(StructuralTypeDeclaration {
            id,
            identity: format!("ReceiverContainer{}", module.structural_types.len()),
            shape,
        });
        referent = id;
    }
    module.machines[0].structural_parameters[0].structural_type = referent;
    call_arguments(&mut module)[0].path = path.to_vec();
    let callee = &mut module.machines[1];
    callee.attachment = Some(structural_type_id(1));
    callee.structural_parameters[0].is_self = true;
    callee.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: true,
    };
    module
}

fn call_arguments(module: &mut TerminalModule) -> &mut Vec<StructuralArgument> {
    match &mut module.machines[0].blocks[0].operations[0].kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => unreachable!(),
    }
}

fn give_receiver_call_scalar_result(module: &mut TerminalModule) {
    let structural_arguments = call_arguments(module).clone();
    let operation = &mut module.machines[0].blocks[0].operations[0];
    operation.result = OperationResult::Scalar(ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    operation.kind = OperationKind::CallStructuralScalar {
        callee: machine_id(2),
        arguments: Vec::new(),
        structural_arguments,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(2).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    callee.blocks[0].operations.push(Operation {
        id: operation_id(2),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    callee.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: ValueId::new(3).unwrap(),
        cleanup_actions: Vec::new(),
    };
}

#[test]
fn indexed_write_only_record_receivers_verify_through_nested_and_interleaved_paths() {
    for path in [
        vec![StructuralPathSegment::FixedIndex(0)],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::FixedIndex(0),
        ],
        vec![
            "outer".into(),
            StructuralPathSegment::FixedIndex(1),
            "inner".into(),
            StructuralPathSegment::FixedIndex(0),
            "receiver".into(),
        ],
    ] {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            for scalar_result in [false, true] {
                let mut module = record_receiver_module(&path);
                module.machines[0].structural_parameters[0].access = access;
                if scalar_result {
                    give_receiver_call_scalar_result(&mut module);
                }
                verify_module(
                    &module,
                    &ProofBundle::default(),
                    &AdmissionProfile::default(),
                )
                .expect("exact material record receiver subloans verify without observing content");
            }
        }
    }
}

#[test]
fn indexed_write_only_record_receiver_requires_exact_target() {
    let mut module = record_receiver_module(&[StructuralPathSegment::FixedIndex(0)]);
    module.machines[1].attachment = Some(structural_type_id(2));
    module.machines[1].structural_parameters[0].structural_type = structural_type_id(2);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::StructuralArgumentTypeMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: structural_type_id(2),
            actual: structural_type_id(1),
        }
    );
}

#[test]
fn indexed_write_only_record_receiver_checks_every_index_and_field() {
    let path = vec![
        "outer".into(),
        StructuralPathSegment::FixedIndex(1),
        "inner".into(),
        StructuralPathSegment::FixedIndex(0),
        "receiver".into(),
    ];
    let module = record_receiver_module(&path);
    for (position, segment) in path.iter().enumerate() {
        let mut changed = module.clone();
        call_arguments(&mut changed)[0].path[position] = match segment {
            StructuralPathSegment::Field(_) => "absent".into(),
            StructuralPathSegment::FixedIndex(_) => StructuralPathSegment::FixedIndex(2),
        };
        assert_eq!(
            validate_module(&changed).unwrap_err(),
            ModuleError::InvalidStructuralArgumentPath {
                operation: operation_id(1),
                argument_index: 0,
            }
        );
    }
}

#[test]
fn indexed_write_only_record_receiver_cannot_widen_access() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut module = record_receiver_module(&[StructuralPathSegment::FixedIndex(0)]);
        module.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        call_arguments(&mut module)[0].access = access;
        assert!(matches!(
            validate_module(&module),
            Err(ModuleError::StructuralArgumentAccessMismatch { .. })
        ));
        module.machines[1].structural_parameters[0].access = access;
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::StructuralArgumentAccessExceedsSource {
                operation: operation_id(1),
                argument_index: 0,
                source: StructuralAccess::WriteOnlyBorrow,
                presented: access,
            }
        );
    }
}

#[test]
fn indexed_write_only_record_receivers_reject_alias_overlap() {
    let mut module = record_receiver_module(&[StructuralPathSegment::FixedIndex(0)]);
    let callee = &mut module.machines[1];
    let mut parameter = callee.structural_parameters[0].clone();
    parameter.position = 1;
    parameter.is_self = false;
    parameter.place = place_id(3);
    callee.structural_parameters.push(parameter);
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let argument = call_arguments(&mut module)[0].clone();
    call_arguments(&mut module).push(argument);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ProjectedUnitCallOutsideBoundedSlice {
            operation: operation_id(1),
        }
    );

    // Scalar calls already admit multiple structural arguments, so they reach
    // the common overlap check after exact path, type, and access validation.
    give_receiver_call_scalar_result(&mut module);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OverlappingExclusiveStructuralArguments {
            operation: operation_id(1),
            first_argument: 0,
            second_argument: 1,
        }
    );
    call_arguments(&mut module)[1].path = vec![StructuralPathSegment::FixedIndex(1)];
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("distinct fixed record elements do not overlap");
    call_arguments(&mut module)[1].path.clear();
    module.machines[1].structural_parameters[1].structural_type =
        module.machines[0].structural_parameters[0].structural_type;
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OverlappingExclusiveStructuralArguments {
            operation: operation_id(1),
            first_argument: 0,
            second_argument: 1,
        }
    );
}

#[test]
fn indexed_write_only_record_receiver_rejects_nonmaterial_fields() {
    let module = record_receiver_module(&[StructuralPathSegment::FixedIndex(0)]);
    for (relevance, field_type) in [
        (
            terminal_psi::BindingRelevance::Relevant,
            StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
        ),
        (
            terminal_psi::BindingRelevance::Erased,
            StructuralFieldType::Erased {
                type_identity: "proof".into(),
            },
        ),
    ] {
        let mut changed = module.clone();
        let StructuralTypeShape::Record { fields } = &mut changed.structural_types[0].shape else {
            unreachable!()
        };
        fields[0].relevance = relevance;
        fields[0].field_type = field_type;
        assert!(matches!(
            validate_module(&changed),
            Err(ModuleError::StructuralArgumentMultiplicityMismatch { .. })
        ));
    }
}

#[test]
fn indexed_write_only_record_receiver_preserves_multiplicity_and_claim_checks() {
    let module = record_receiver_module(&[StructuralPathSegment::FixedIndex(0)]);
    for machine_index in 0..2 {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Linear,
        ] {
            let mut changed = module.clone();
            changed.machines[machine_index].structural_parameters[0].multiplicity = multiplicity;
            assert!(validate_module(&changed).is_err());
        }
        let mut changed = module.clone();
        let machine = &mut changed.machines[machine_index];
        machine.entry_claims.push(EntryClaim {
            claim: claim_id(1),
            input: machine.structural_parameters[0].place,
            path: Vec::new(),
        });
        assert_eq!(
            validate_module(&changed).unwrap_err(),
            ModuleError::EntryClaimRequiresOwnedParameter(claim_id(1))
        );
    }
    for path in [
        vec![StructuralPathSegment::FixedIndex(0)],
        vec![StructuralPathSegment::FixedIndex(0), "receiver".into()],
    ] {
        let mut changed = record_receiver_module(&path);
        for machine in &mut changed.machines {
            machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        }
        assert!(validate_module(&changed).is_err());
    }
}

#[test]
fn indexed_write_only_receiver_rejects_sums_and_unknown_referents() {
    let module = record_receiver_module(&[StructuralPathSegment::FixedIndex(0)]);
    let cases = vec![terminal_psi::StructuralCaseDeclaration {
        id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
        identity: "only".into(),
        fields: Vec::new(),
    }];
    for shape in [
        StructuralTypeShape::Sum {
            cases: cases.clone(),
        },
        StructuralTypeShape::Mixed {
            fields: Vec::new(),
            cases,
        },
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
    ] {
        let mut changed = module.clone();
        changed.structural_types[0].shape = shape;
        assert!(matches!(
            validate_module(&changed),
            Err(ModuleError::StructuralArgumentMultiplicityMismatch { .. })
        ));
    }
    let mut changed = module;
    let StructuralTypeShape::FixedArray { element, .. } = &mut changed.structural_types[2].shape
    else {
        unreachable!()
    };
    *element = structural_type_id(99);
    assert_eq!(
        validate_module(&changed).unwrap_err(),
        ModuleError::UnknownStructuralType(structural_type_id(99))
    );
}

#[test]
fn indexed_write_only_attenuation_rejects_shared_and_unserved_owned_roots() {
    let mut module = indexed_attenuation_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentAccessExceedsSource { .. })
    ));

    // Owned projected loans have a separate custody contract; this change only
    // removes observation authority from an already exclusive borrowed root.
    module.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentMultiplicityMismatch { .. })
    ));
}

#[test]
fn indexed_write_only_attenuation_keeps_exact_access_and_bounds() {
    let module = indexed_attenuation_module();
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let mut changed = module.clone();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut changed.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = access;
        assert!(matches!(
            validate_module(&changed),
            Err(ModuleError::StructuralArgumentAccessMismatch { .. })
        ));
    }
    let mut changed = module;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut changed.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    assert!(matches!(
        validate_module(&changed),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
}
