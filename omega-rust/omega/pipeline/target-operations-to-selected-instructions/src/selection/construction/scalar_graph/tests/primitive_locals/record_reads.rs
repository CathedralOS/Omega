//! Source-free native snapshots retain their actual record and observation order.
use super::hostile::append;
use super::*;
use semantic_vocabulary::StructuralFieldId;

fn record_fixture(native: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = local_fixture(native, false);
    let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
    let fields = [
        StructuralFieldId::new(1).unwrap(),
        StructuralFieldId::new(2).unwrap(),
    ];
    source
        .structural
        .as_mut()
        .unwrap()
        .structural_types
        .make_mut()[0]
        .shape = StructuralTypeShape::Record {
        fields: fields
            .iter()
            .enumerate()
            .map(
                |(position, field)| terminal_psi::StructuralFieldDeclaration {
                    id: *field,
                    identity: format!("field{position}"),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                },
            )
            .collect(),
    };
    let LegalizedScalarInstructionKind::EstablishPrimitiveLocal { result, value, .. } =
        source.blocks[0].instructions[1].kind.clone()
    else {
        panic!("local");
    };
    source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::EstablishScalarRecord {
        result,
        shape: ValueShape::integer(16, 8),
        fields: fields
            .into_iter()
            .map(|field| terminal_psi::ScalarRecordFieldValue {
                field,
                value: value.value,
            })
            .collect(),
    };
    source.blocks[0].instructions[2].kind =
        LegalizedScalarInstructionKind::StructuralScalarFieldRead {
            source: terminal_psi::StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                access: StructuralAccess::Owned,
                path: Vec::new(),
            },
            field: fields[1],
        };
    source.blocks[0].instructions[2].ownership.clear();
    source.blocks[0].instructions[3] = fixture(native, 0).blocks[0].instructions[3].clone();
    source
}

#[test]
fn owned_record_read_rejects_wrong_root_offset_and_unavailable_home() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = record_fixture(native);
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        let selected = construct(&source).unwrap();
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        assert_eq!(selected.local_storage_slots.len(), 1);
        let mut changed = selected.clone();
        let read = changed.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| matches!(row.kind, SelectedInstructionKind::Load64 { byte_offset: 8 }))
            .unwrap();
        read.kind = SelectedInstructionKind::Load64 { byte_offset: 0 };
        assert!(validate(&source, &changed).is_err());
        for mutation in 0..3 {
            let mut changed = source.clone();
            if mutation == 2 {
                changed.blocks[0].instructions.swap(1, 2);
            } else {
                let LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, field } =
                    &mut changed.blocks[0].instructions[2].kind
                else {
                    panic!("read");
                };
                if mutation == 0 {
                    source.place = PlaceId::new(99).unwrap();
                } else {
                    *field = StructuralFieldId::new(99).unwrap();
                }
            }
            assert!(
                construct(&changed).is_err(),
                "{native:?} construction {mutation}"
            );
            assert!(
                validate(&changed, &selected).is_err(),
                "{native:?} replay {mutation}"
            );
        }
    }
}

#[test]
fn record_read_store_read_materializes_two_snapshots_before_the_consuming_call() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = record_fixture(native);
        let place = PlaceId::new(1).unwrap();
        let identity = StructuralTypeId::new(1).unwrap();
        let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: vec![ValueShape::borrowed_reference(16, 8)],
                result: None,
            },
        )
        .unwrap();
        let parameter = terminal_psi::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type: identity,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let signature = source.structural.as_mut().unwrap();
        signature.structural_places[0].kind = StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        };
        signature.parameters = vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: parameter.clone(),
            target: target_operations::TargetStructuralParameter {
                place,
                structural_type: identity,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                projected_qualifications: Vec::new(),
                shape: ValueShape::borrowed_reference(16, 8),
                placement: source.call_plan.parameters[0].clone(),
            },
        }];
        // Snapshot first, store a different scalar, snapshot again, then use both.
        let mut read = source.blocks[0].instructions[2].kind.clone();
        let LegalizedScalarInstructionKind::StructuralScalarFieldRead {
            source: argument,
            field,
        } = &mut read
        else {
            panic!("read");
        };
        argument.access = StructuralAccess::MutableBorrow;
        let field = *field;
        source.blocks[0].instructions[0].kind = read.clone();
        source.blocks[0].instructions[1] = fixture(native, 0).blocks[0].instructions[1].clone();
        source.blocks[0].instructions[2].result = None;
        source.blocks[0].instructions[2].kind =
            LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                destination: parameter,
                path: Vec::new(),
                field,
                value: abstract_operations::AbstractResult {
                    value: ValueId::new(2).unwrap(),
                    scalar_type: scalar,
                },
                byte_offset: 8,
                byte_size: 8,
            };
        source.blocks[0].instructions[3].kind = read;
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 2],
                result: None,
            },
        )
        .unwrap();
        let arguments = [1, 4]
            .into_iter()
            .zip(&call_plan.parameters)
            .map(|(raw, placement)| LegalizedScalarArgument::Scalar {
                source: ValueId::new(raw).unwrap(),
                placement: placement.clone(),
            })
            .collect();
        append(
            &mut source,
            5,
            LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                structural_result: None,
                source: LegalizedCallUnitSource::AuthoredCallUnit,
                callee: MachineId::new(10).unwrap(),
                arguments,
                call_plan,
                result_placement: None,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }),
            None,
        );
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            native,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                selected,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        let rows = &selected.blocks[0].instructions;
        let reads = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                matches!(row.kind, SelectedInstructionKind::Load64 { byte_offset: 8 })
            })
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 2);
        let store = rows
            .iter()
            .position(|row| {
                matches!(
                    row.kind,
                    SelectedInstructionKind::Store { byte_offset: 8, .. }
                )
            })
            .unwrap();
        let call = rows
            .iter()
            .position(|row| matches!(row.kind, SelectedInstructionKind::CallUnit { .. }))
            .unwrap();
        assert!(reads[0] < store && store < reads[1] && reads[1] < call);
        assert_eq!(
            rows[reads[0]].operands[0].virtual_register,
            rows[reads[1]].operands[0].virtual_register
        );
        assert_ne!(
            rows[reads[0]].operands[1].virtual_register,
            rows[reads[1]].operands[1].virtual_register
        );
        for mutation in 0..3 {
            let mut changed = selected.clone();
            match mutation {
                0 => changed.blocks[0].instructions.swap(reads[0], reads[1]),
                1 => changed.blocks[0].instructions.swap(store, reads[1]),
                _ => {
                    changed.blocks[0].instructions[reads[1]].operands[1].virtual_register =
                        rows[reads[0]].operands[1].virtual_register
                }
            }
            assert!(
                validate(&changed).is_err(),
                "{native:?} snapshot mutation {mutation}"
            );
        }
    }
}
