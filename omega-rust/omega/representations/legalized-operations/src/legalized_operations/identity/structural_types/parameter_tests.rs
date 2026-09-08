use super::*;

#[test]
fn semantic_and_target_parameters_bind_ordered_projected_qualifications() {
    let rows = [1, 2]
        .map(|index| terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(index)],
            domain: semantic_vocabulary::StructuralDomainId::new(index + 10).unwrap(),
        })
        .to_vec();
    let semantic = StructuralParameterDeclaration {
        place: semantic_vocabulary::PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: rows.clone(),
    };
    let shape = ValueShape::integer(8, 8);
    let target = target_operations::TargetStructuralParameter {
        place: semantic.place,
        structural_type: semantic.structural_type,
        multiplicity: semantic.multiplicity,
        access: semantic.access,
        projected_qualifications: rows.clone(),
        shape,
        placement: ValuePlacement {
            shape,
            locations: Vec::new(),
        },
    };
    let encode_semantic = |parameter: &StructuralParameterDeclaration| {
        let mut bytes = Vec::new();
        encode_structural_parameter(&mut bytes, parameter);
        bytes
    };
    let encode_target = |parameter: &target_operations::TargetStructuralParameter| {
        let mut bytes = Vec::new();
        encode_target_structural_parameter(&mut bytes, parameter);
        bytes
    };
    for mutation in 0..5 {
        let mut changed = rows.clone();
        match mutation {
            0 => changed[0].domain = semantic_vocabulary::StructuralDomainId::new(99).unwrap(),
            1 => changed[0].path[0] = StructuralPathSegment::FixedIndex(99),
            2 => changed.swap(0, 1),
            3 => {
                changed.pop();
            }
            _ => changed.clear(),
        }
        let mut changed_semantic = semantic.clone();
        changed_semantic.projected_qualifications = changed.clone();
        let mut changed_target = target.clone();
        changed_target.projected_qualifications = changed;
        assert_ne!(
            encode_semantic(&semantic),
            encode_semantic(&changed_semantic)
        );
        assert_ne!(encode_target(&target), encode_target(&changed_target));
    }
}
