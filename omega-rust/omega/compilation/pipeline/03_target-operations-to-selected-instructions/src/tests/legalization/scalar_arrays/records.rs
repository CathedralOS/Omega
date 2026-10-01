use super::{
    FuelScheduleIdentity, OperationId, PlaceId, ScalarType, StructuralTypeShape,
    TargetStructuralHomeLayout, TargetUnitOperation, ValueShape, fixture,
};
use crate::legalize_target_operations;
use crate::tests::legalization::scalar_arrays::K;
use crate::tests::legalization::scalar_arrays::O;
use crate::validate_legalized_operations;

#[test]
fn byte_field_snapshot_replays_copy_geometry_without_borrow_projection() {
    use crate::structural_inputs::structural_reference_input;
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    };
    let (mut source, _, _) = record_fixture();
    let root = source.structural_types[0].id;
    let carrier = semantic_vocabulary::StructuralTypeId::new(3).unwrap();
    let input = PlaceId::new(3).unwrap();
    let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
    predicates.insert(terminal_psi::ByteSequencePredicate::ValidUtf8);
    let StructuralTypeShape::Record { fields } = &mut source.structural_types.make_mut()[0].shape
    else {
        panic!("record");
    };
    for field in &mut fields[1..] {
        field.field_type = terminal_psi::StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
            predicates,
        };
    }
    source
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: carrier,
            identity: "copied-byte-field".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            }),
        });
    let path = vec![StructuralPathSegment::Field("field1".into())];
    assert!(structural_reference_input::project(root, &path, &source.structural_types).is_none());
    assert_eq!(
        structural_reference_input::runtime_leaf_copy_projection(
            root,
            &path,
            &source.structural_types
        ),
        Some((carrier, 8, Vec::new())),
    );
    let function = &mut source.functions[0];
    function
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: input,
            position: 0,
            is_self: false,
            structural_type: root,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let abstract_operations::AbstractFunctionResult::Structural(returned) = &mut function.result
    else {
        panic!("structural result");
    };
    returned.structural_type = carrier;
    let O::EstablishRecord {
        psi_operation,
        mut result,
        ..
    } = function.operations[2].clone()
    else {
        panic!("record constructor");
    };
    result.structural_type = carrier;
    function.operations[2] = O::StructuralLeafCopy {
        psi_operation,
        source: input,
        path: path.into(),
        result,
    };
    function.operations.drain(..2);
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        for corruption in 0..5 {
            let mut changed = legalized.plan().clone();
            let row = changed.scalar_functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|row| matches!(row.kind, K::StructuralLeafCopy { .. }))
                .unwrap();
            let K::StructuralLeafCopy {
                source: copied_source,
                path,
                result,
                byte_offset,
                shape,
                ..
            } = &mut row.kind
            else {
                panic!("copy");
            };
            match corruption {
                0 => *copied_source = PlaceId::new(99).unwrap(),
                1 => *path = vec![StructuralPathSegment::Field("field2".into())].into(),
                2 => *byte_offset = 24,
                3 => *shape = ValueShape::integer(24, 8),
                4 => result.structural_type = root,
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "{native:?}: byte field copy corruption {corruption}"
            );
        }
    }
}

fn record_fixture() -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = fixture(3);
    let fields = (1..=3)
        .map(|ordinal| semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap())
        .collect::<Vec<_>>();
    source.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
        fields: fields
            .iter()
            .enumerate()
            .map(
                |(ordinal, field)| terminal_psi::StructuralFieldDeclaration {
                    id: *field,
                    identity: format!("field{ordinal}"),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                },
            )
            .collect(),
    };
    let O::EstablishScalarArray {
        psi_operation,
        result,
        elements,
    } = source.functions[0].operations[2].clone()
    else {
        panic!("array fixture");
    };
    source.functions[0].operations[2] = O::EstablishRecord {
        psi_operation,
        result,
        fields: fields
            .into_iter()
            .zip(elements)
            .map(|(field, value)| terminal_psi::RecordFieldInitializer {
                field,
                value: terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation: None,
                },
            })
            .collect(),
    };
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(
            target::NativeTarget::linux_x64(),
        ),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (source, target, unit)
}

#[test]
fn scalar_record_target_replays_exact_field_value_and_home_correspondence() {
    let (source, target, unit) = record_fixture();
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetUnitOperation::EstablishRecord {
            fields,
            result_home,
            psi_operation,
        } = &mut changed.functions[0].graph.blocks[0].operations[2]
        else {
            panic!("record");
        };
        match mutation {
            0 => fields[0].value = fields[1].value.clone(),
            1 => fields.swap(0, 1),
            2 => {
                fields.pop();
            }
            3 => {
                result_home.layout =
                    TargetStructuralHomeLayout::Aggregate(ValueShape::integer(4, 1))
            }
            4 => *psi_operation = OperationId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "target mutation {mutation}"
        );
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                .is_err(),
            "target replay mutation {mutation}"
        );
    }
    for mutation in 0..4 {
        let mut changed = legalized.plan().clone();
        let instruction = changed.scalar_functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| matches!(instruction.kind, K::EstablishRecord { .. }))
            .unwrap();
        let K::EstablishRecord {
            fields,
            result,
            shape,
        } = &mut instruction.kind
        else {
            panic!("record");
        };
        match mutation {
            0 => fields[0].value = fields[1].value.clone(),
            1 => fields.swap(0, 1),
            2 => *shape = ValueShape::integer(4, 1),
            3 => result.place = PlaceId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "legalized mutation {mutation}"
        );
    }
}

#[test]
fn owned_byte_record_replays_snapshot_qualifications_and_native_home() {
    use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity};
    let (mut source, _, _) = record_fixture();
    let carrier = semantic_vocabulary::StructuralTypeId::new(3).unwrap();
    let domain = semantic_vocabulary::StructuralDomainId::new(1).unwrap();
    let input = PlaceId::new(3).unwrap();
    let copied = PlaceId::new(4).unwrap();
    source
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: carrier,
            identity: "record-owned-utf8".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            }),
        });
    let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
    predicates.insert(terminal_psi::ByteSequencePredicate::ValidUtf8);
    let StructuralTypeShape::Record { fields } = &mut source.structural_types.make_mut()[0].shape
    else {
        panic!("record");
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 },
        predicates,
    };
    let function = &mut source.functions[0];
    function
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: input,
            position: 0,
            is_self: false,
            structural_type: carrier,
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![domain],
            projected_qualifications: Vec::new(),
        });
    let O::EstablishRecord { fields, .. } = &mut function.operations[2] else {
        panic!("record");
    };
    fields[0].value =
        terminal_psi::RecordFieldValue::Structural(terminal_psi::StructuralArgument {
            place: copied,
            access: StructuralAccess::Owned,
            path: Vec::new(),
        });
    function.operations.insert(
        2,
        O::StructuralLeafCopy {
            psi_operation: OperationId::new(4).unwrap(),
            source: input,
            path: Vec::<terminal_psi::StructuralPathSegment>::new().into(),
            result: terminal_psi::StructuralOperationResult {
                place: copied,
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![domain],
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
        },
    );
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "record-Utf8".into(),
        carrier,
        byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    }]
    .into();
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("exact declared byte predicates survive the snapshot");
    for corruption in 0..3 {
        let mut changed = unit.clone();
        match corruption {
            0 => changed.functions[0].structural_parameters[0]
                .qualifications
                .clear(),
            1 => {
                std::sync::Arc::make_mut(&mut changed.structural_domains)[0].byte_predicate =
                    Some(terminal_psi::ByteSequencePredicate::NoNul)
            }
            2 => {
                changed.structural_types.make_mut()[2].shape =
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        capacity: 9,
                    })
            }
            _ => unreachable!(),
        }
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
            "semantic corruption {corruption}"
        );
    }
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        for corruption in 0..4 {
            let mut changed = legalized.plan().clone();
            let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
            let copy_index = rows
                .iter()
                .position(|row| matches!(row.kind, K::StructuralLeafCopy { .. }))
                .unwrap();
            let record_index = rows
                .iter()
                .position(|row| matches!(row.kind, K::EstablishRecord { .. }))
                .unwrap();
            if corruption == 3 {
                rows.swap(copy_index, record_index);
            } else {
                let K::StructuralLeafCopy {
                    source,
                    result,
                    shape,
                    ..
                } = &mut rows[copy_index].kind
                else {
                    panic!("copy");
                };
                match corruption {
                    0 => *source = PlaceId::new(99).unwrap(),
                    1 => result.qualifications.clear(),
                    2 => *shape = ValueShape::integer(8, 8),
                    _ => unreachable!(),
                }
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "{native:?}: snapshot corruption {corruption}"
            );
        }
    }
}
