//! Physical array construction and receiving replay retain each ordered leaf.
use super::*;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn array_fixture(target: target::NativeTarget, length: u16) -> LegalizedScalarFunction {
    let mut source = fixture(target, 0);
    source.attachment = None;
    source.blocks[0].instructions.truncate(3);
    source.provenance.operations.truncate(3);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    for row in &mut source.blocks[0].instructions[..2] {
        row.result.as_mut().unwrap().scalar_type = scalar;
    }
    let array = StructuralTypeId::new(1).unwrap();
    let leaf = StructuralTypeId::new(2).unwrap();
    let place = PlaceId::new(1).unwrap();
    let producer = source.blocks[0].instructions[2].operation;
    let result = StructuralOperationResult {
        place,
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        result: Some(StructuralResultDeclaration {
            place: PlaceId::new(2).unwrap(),
            structural_type: array,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: array,
                identity: "array".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: leaf,
                    length: u64::from(length),
                },
            },
            StructuralTypeDeclaration {
                id: leaf,
                identity: "u8".into(),
                shape: StructuralTypeShape::PrimitiveScalar(scalar),
            },
        ],
        parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: array,
            },
        }],
    });
    let row = &mut source.blocks[0].instructions[2];
    row.result = None;
    row.kind = LegalizedScalarInstructionKind::EstablishScalarArray {
        result: result.clone(),
        elements: (0..length)
            .map(|ordinal| ValueId::new(u64::from(ordinal % 2) + 1).unwrap())
            .collect(),
        shape: ValueShape::integer(length, 1),
    };
    let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
        panic!("return");
    };
    returned.value = LegalizedScalarReturnValue::Structural {
        defining_operation: producer,
        result,
    };
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(length, 1)),
        },
    )
    .unwrap();
    source
}

#[test]
fn array_selection_rejects_reordered_operands_and_changed_layout() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = array_fixture(target, 2);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("array selection for {target:?}: {error:?}"));
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected)
            .unwrap_or_else(|error| panic!("array replay for {target:?}: {error:?}"));
        for mutation in 0..3 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::EstablishScalarArray {
                elements, shape, ..
            } = &mut changed.blocks[0].instructions[2].kind
            else {
                panic!("array");
            };
            match mutation {
                0 => elements.swap(0, 1),
                1 => {
                    elements.pop();
                }
                2 => shape.byte_size = 4,
                _ => unreachable!(),
            }
            assert!(validate(&changed, &selected).is_err());
        }
        let mut changed = selected.clone();
        let store = changed.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
            .unwrap();
        store.kind = SelectedInstructionKind::Store {
            byte_offset: 1,
            byte_size: 1,
        };
        assert!(validate(&source, &changed).is_err());
        for length in [0, 3, 5, 6, 7] {
            assert!(
                build(
                    0,
                    &array_fixture(target, length),
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints()
                )
                .is_err(),
                "unsupported exact fragment width {length}"
            );
        }
    }
}

#[test]
fn empty_nested_array_shape_survives_earlier_dimension_overflow() {
    let mut source = array_fixture(target::NativeTarget::linux_arm64(), 0);
    let signature = source.structural.as_mut().unwrap();
    let outer = StructuralTypeId::new(1).unwrap();
    let leaf = StructuralTypeId::new(2).unwrap();
    let middle = StructuralTypeId::new(3).unwrap();
    let inner = StructuralTypeId::new(4).unwrap();
    signature.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: middle,
        length: u64::MAX,
    };
    signature.structural_types.push(StructuralTypeDeclaration {
        id: middle,
        identity: "middle".into(),
        shape: StructuralTypeShape::FixedArray {
            element: inner,
            length: u64::MAX,
        },
    });
    signature.structural_types.push(StructuralTypeDeclaration {
        id: inner,
        identity: "inner".into(),
        shape: StructuralTypeShape::FixedArray {
            element: leaf,
            length: 0,
        },
    });
    assert_eq!(
        crate::selection::scalar_array_input::shape(&source, outer)
            .unwrap()
            .2,
        ValueShape::integer(0, 1)
    );
    source
        .structural
        .as_mut()
        .unwrap()
        .structural_types
        .last_mut()
        .unwrap()
        .shape = StructuralTypeShape::FixedArray {
        element: inner,
        length: 0,
    };
    assert!(
        crate::selection::scalar_array_input::shape(&source, outer).is_none(),
        "empty dimension must not hide a cycle"
    );
}
