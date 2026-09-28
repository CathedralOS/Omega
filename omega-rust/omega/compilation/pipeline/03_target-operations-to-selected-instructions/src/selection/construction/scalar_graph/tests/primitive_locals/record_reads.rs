//! Source-free native snapshots retain their actual record and observation order.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, PlaceId, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, StructuralAccess, StructuralTypeShape,
    ValueShape, build, fixture,
};
use crate::selection::construction::scalar_graph::tests::primitive_locals::local_fixture;
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
    source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::EstablishRecord {
        result,
        shape: ValueShape::integer(16, 8),
        fields: fields
            .into_iter()
            .map(|field| terminal_psi::RecordFieldInitializer {
                field,
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: value.value,
                    range_obligation: None,
                },
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
