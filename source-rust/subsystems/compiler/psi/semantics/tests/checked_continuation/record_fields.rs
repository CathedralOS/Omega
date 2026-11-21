use super::{constant, program};
use semantic_vocabulary::{
    MachineId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, RecordFieldInitializer, RecordFieldValue,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralRecordStorage,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
};
use terminal_semantics::continuation::CheckedPrefix;

fn record(ids: &[u64]) -> (TerminalModule, (MachineId, OperationId)) {
    let (mut module, cut) = program();
    let mut names = std::mem::take(&mut module.identities).into_builder();
    let ty = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let constructor = OperationId::new(2).unwrap();
    let mut fields = Vec::with_capacity(ids.len());
    let mut initializers = Vec::with_capacity(ids.len());
    for (ordinal, &number) in ids.iter().enumerate() {
        let id = StructuralFieldId::new(number).unwrap();
        fields.push(StructuralFieldDeclaration {
            id,
            identity: names.intern(&format!("f{ordinal}")),
            declaration_identity: names.intern(&format!("Flags::f{ordinal}")),
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            relevance: Default::default(),
            storage: None,
            inline_carrier: None,
        });
        initializers.push(RecordFieldInitializer {
            field: id,
            value: RecordFieldValue::Scalar {
                value: ValueId::new(3).unwrap(),
                range_obligation: None,
            },
        });
    }
    module
        .types
        .structural_types
        .push(StructuralTypeDeclaration {
            id: ty,
            identity: names.intern("Flags storage"),
            declaration_identity: names.intern("Flags"),
            shape: StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Packed,
            },
            multiplicity: StructuralMultiplicity::Unrestricted,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            quotient_carrier: None,
            scalar_restriction: None,
            requires: Vec::new(),
        });
    module.identities = names.build();
    let root = &mut module.machines[0];
    root.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: constructor,
            structural_type: ty,
        },
    });
    root.blocks[0].operations.splice(
        0..0,
        [
            constant(true, 3),
            Operation::unsuspended(
                constructor,
                OperationResult::Structural(StructuralOperationResult {
                    place,
                    structural_type: ty,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                }),
                OperationKind::EstablishRecord {
                    fields: initializers,
                },
            ),
        ],
    );
    (module, cut)
}

#[test]
fn a_wide_constructor_rejoins_each_field_and_refuses_duplicate_selectors() {
    let old_ids: Vec<_> = (1..=256).collect();
    let mut new_ids: Vec<_> = old_ids.iter().copied().rev().collect();
    let (old, site) = record(&old_ids);
    let (new, _) = record(&new_ids);
    CheckedPrefix::check(&old, site, &new, site).unwrap();

    new_ids[255] = new_ids[0];
    let (ambiguous, _) = record(&new_ids);
    for (previous, successor) in [(&old, &ambiguous), (&ambiguous, &new)] {
        assert_eq!(
            CheckedPrefix::check(previous, site, successor, site).err(),
            Some("successor changed a selected field declaration")
        );
    }
}
