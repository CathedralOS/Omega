//! Runtime binding must preserve exclusive referents independently of multiplicity.

use super::*;

fn parameter(
    position: u32,
    access: StructuralAccess,
    multiplicity: StructuralMultiplicity,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(u64::from(position) + 1).unwrap(),
        position,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn argument(path: Vec<StructuralPathSegment>) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity: 41,
        structural_type: StructuralTypeId::new(1).unwrap(),
        qualifications: Vec::new(),
        path,
    }
}

fn assert_alias_rejected(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
) {
    assert!(matches!(
        bind_structural_arguments(parameters, arguments),
        Err(TerminalInterpretError::StructuralArgumentAliasing(41))
    ));
}

#[test]
fn unrestricted_exclusive_arguments_reject_same_referent_in_both_orders() {
    for (left, right) in [
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
        ),
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::MutableBorrow,
        ),
        (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::SharedBorrow,
        ),
        (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::MutableBorrow,
        ),
        (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ),
    ] {
        for (first, second) in [(left, right), (right, left)] {
            let parameters = [
                parameter(0, first, StructuralMultiplicity::Unrestricted),
                parameter(1, second, StructuralMultiplicity::Unrestricted),
            ];
            for path in [
                Vec::new(),
                vec![StructuralPathSegment::Field("bytes".into())],
            ] {
                assert_alias_rejected(&parameters, &[argument(path.clone()), argument(path)]);
            }
        }
    }
}

#[test]
fn exclusive_arguments_reject_ancestor_descendant_overlap_in_both_orders() {
    let parent = vec![StructuralPathSegment::Field("items".into())];
    let child = vec![
        StructuralPathSegment::Field("items".into()),
        StructuralPathSegment::FixedIndex(1),
    ];
    for (left, right) in [(parent.clone(), child.clone()), (child, parent)] {
        let parameters = [
            parameter(
                0,
                StructuralAccess::MutableBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
            parameter(
                1,
                StructuralAccess::SharedBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
        ];
        assert_alias_rejected(&parameters, &[argument(left), argument(right)]);
    }
}

#[test]
fn disjoint_projected_paths_and_shared_aliases_remain_legal() {
    let field = |name: &str| StructuralPathSegment::Field(name.into());
    for (left, right) in [
        (vec![field("left")], vec![field("right")]),
        (
            vec![field("items"), StructuralPathSegment::FixedIndex(0)],
            vec![field("items"), StructuralPathSegment::FixedIndex(1)],
        ),
    ] {
        let parameters = [
            parameter(
                0,
                StructuralAccess::MutableBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
            parameter(
                1,
                StructuralAccess::WriteOnlyBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
        ];
        assert!(bind_structural_arguments(&parameters, &[argument(left), argument(right)]).is_ok());
    }
    let parameters = [
        parameter(
            0,
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Unrestricted,
        ),
        parameter(
            1,
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Unrestricted,
        ),
    ];
    assert!(
        bind_structural_arguments(&parameters, &[argument(Vec::new()), argument(Vec::new())])
            .is_ok()
    );
    assert!(
        bind_structural_arguments(
            &parameters,
            &[argument(Vec::new()), argument(vec![field("child")])]
        )
        .is_ok()
    );
}

#[test]
fn non_unrestricted_identity_alias_prohibition_is_preserved() {
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Linear,
    ] {
        for (left, right) in [
            (multiplicity, StructuralMultiplicity::Unrestricted),
            (StructuralMultiplicity::Unrestricted, multiplicity),
        ] {
            let parameters = [
                parameter(0, StructuralAccess::SharedBorrow, left),
                parameter(1, StructuralAccess::SharedBorrow, right),
            ];
            // Preserve the existing whole-identity multiplicity fence even
            // when the supplied paths happen to be disjoint.
            assert_alias_rejected(
                &parameters,
                &[
                    argument(vec![StructuralPathSegment::Field("left".into())]),
                    argument(vec![StructuralPathSegment::Field("right".into())]),
                ],
            );
        }
    }
}
