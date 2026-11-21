use super::*;
use terminal_psi::{
    BindingRelevance, StructuralFieldType, StructuralMultiplicity,
    StructuralRecordStorage, TerminalIdentityIndex,
};

fn empty(identity: u64) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        id: StructuralTypeId::new(identity).unwrap(),
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Affine,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        quotient_carrier: None,
        scalar_restriction: None,
        requires: Vec::new(),
        shape: StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: Vec::new(),
        },
    }
}

#[test]
fn claim_scratch_follows_declared_rows_with_sparse_identities() {
    let child = empty(u64::MAX - 1);
    let mut root = empty(7);
    root.multiplicity = StructuralMultiplicity::Linear;
    root.owns_linear_claim = true;
    let field = terminal_psi::StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
        identity: TerminalIdentityIndex(2),
        relevance: BindingRelevance::Relevant,
        storage: None,
        field_type: StructuralFieldType::Structural(child.id),
        inline_carrier: None,
    };
    let StructuralTypeShape::Record { fields, .. } = &mut root.shape else {
        unreachable!()
    };
    fields.push(field);
    // Unsorted, widely separated IDs still get exactly two slots.
    let mut declarations = [child, root];
    let types = ClaimTypes::new(&declarations).unwrap();
    assert_eq!(types.states.len(), 2);
    assert!(!contains_claim(declarations[0].id, &types).unwrap());
    assert!(contains_claim(declarations[1].id, &types).unwrap());
    super::super::validate_intrinsic_claim_roots(&declarations).unwrap();
    assert_eq!(
        super::super::intrinsic_claim_root_by(&types, declarations[1].id),
        Ok(true)
    );
    let paths = super::super::claim_catalogue(&types, declarations[1].id, 1).unwrap();
    assert_eq!(paths, BTreeSet::from([Vec::new()]));
    validate_complete_claim_frontier(
        &types,
        declarations[1].id,
        &[],
        paths.iter().map(Vec::as_slice),
        &|_, _| false,
    )
    .unwrap();
    assert!(
        validate_complete_claim_frontier(
            &types,
            declarations[1].id,
            &[],
            std::iter::empty(),
            &|_, _| false,
        )
        .is_err()
    );
    assert!(ClaimTypes::new([&declarations[0], &declarations[0]]).is_err());
    // Clearing the nominal root or forging a claim on the affine
    // child cannot evade the declaration checks.
    declarations[1].owns_linear_claim = false;
    assert!(super::super::validate_intrinsic_claim_roots(&declarations).is_err());
    declarations[0].owns_linear_claim = true;
    assert!(super::super::validate_intrinsic_claim_roots(&declarations).is_err());
    // A referenced type must be present in this exact roster.
    assert!(ClaimTypes::new([&declarations[1]]).is_err());
    let self_id = declarations[1].id;
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[1].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Structural(self_id);
    assert!(ClaimTypes::new([&declarations[1]]).is_err());
}
