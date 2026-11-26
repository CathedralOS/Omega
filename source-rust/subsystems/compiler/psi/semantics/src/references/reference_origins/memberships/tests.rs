use super::*;
use semantic_vocabulary::{BlockId, ScalarType};
use std::cell::Cell;
use terminal_psi::{StructuralArgument, StructuralMultiplicity, TerminalIdentityIndex};

fn declarations(
    access: StructuralAccess,
) -> (
    Vec<StructuralTypeDeclaration>,
    StructuralOperationResult,
    StructuralResultDeclaration,
) {
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let types = [
        (
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        (
            reference,
            StructuralTypeShape::Reference {
                referent: scalar,
                access,
            },
        ),
    ]
    .into_iter()
    .map(|(id, shape)| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    })
    .collect();
    let result = StructuralOperationResult {
        place: PlaceId::new(99).unwrap(),
        structural_type: reference,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let declaration = StructuralResultDeclaration {
        place: result.place,
        structural_type: reference,
        multiplicity: result.multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    };
    (types, result, declaration)
}

#[test]
fn unresolved_views_keep_loans_without_becoming_reference_authority() {
    let (mut types, _, declaration) = declarations(StructuralAccess::SharedBorrow);
    let contents = crate::descriptor_results::DescriptorResultContents {
        slots: vec![crate::descriptor_results::DescriptorResultSlot {
            path: Vec::new(),
            origins: Vec::new(),
        }],
        unknown_contents: Vec::new(),
        unresolved_slots: vec![Vec::new()],
        case_facts: Default::default(),
    };
    assert_eq!(
        crate::reference_result_contents(&types, &[], &declaration, &contents),
        Err("reference result retains an unresolved spatial origin")
    );
    types[1].shape = StructuralTypeShape::ByteSequence(
        terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        },
    );
    assert_eq!(
        crate::reference_result_contents(&types, &[], &declaration, &contents),
        Ok(Vec::new())
    );
}

#[test]
fn returning_an_indexed_generic_value_requires_read_access_to_its_backing() {
    use crate::descriptor_lifetimes::DescriptorPathSegment::UnknownContents;
    use crate::descriptor_results::{
        DescriptorContentsStep, DescriptorResultContents, DescriptorResultSlot,
        DescriptorStoredOrigin,
    };
    use crate::storage::borrowed_storage::Projection;

    let (mut types, _, mut result) = declarations(StructuralAccess::SharedBorrow);
    let element = types[0].id;
    types[0].shape = StructuralTypeShape::TypeParameter;
    types[0].multiplicity = StructuralMultiplicity::Unrestricted;
    types[1].shape = StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(3),
    };
    result.structural_type = element;
    let mut input = terminal_psi::StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        structural_type: types[1].id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    };
    let contents = DescriptorResultContents {
        slots: Vec::new(),
        unknown_contents: vec![DescriptorResultSlot {
            path: vec![UnknownContents],
            origins: vec![DescriptorStoredOrigin::IncomingContents {
                position: 0,
                steps: vec![
                    DescriptorContentsStep::any_element(element),
                    DescriptorContentsStep::Address(UnknownContents),
                ],
                projection: Projection::new(&[]),
            }],
        }],
        unresolved_slots: Vec::new(),
        case_facts: Default::default(),
    };
    let check = |types: &[StructuralTypeDeclaration],
                 input: &terminal_psi::StructuralParameterDeclaration| {
        crate::reference_result_contents(types, std::slice::from_ref(input), &result, &contents)
    };
    assert_eq!(check(&types, &input), Ok(Vec::new()));
    input.access = StructuralAccess::WriteOnlyBorrow;
    let denied = Err("unknown contents were transferred without owned access");
    assert_eq!(check(&types, &input), denied);

    input.access = StructuralAccess::Owned;
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        types[1].shape = StructuralTypeShape::ElementView {
            element,
            access: Some(access),
        };
        assert_eq!(
            check(&types, &input),
            if access == StructuralAccess::WriteOnlyBorrow {
                denied.clone()
            } else {
                Ok(Vec::new())
            }
        );
    }
    types[1].shape = StructuralTypeShape::ElementView {
        element,
        access: Some(StructuralAccess::SharedBorrow),
    };
    types[0].multiplicity = StructuralMultiplicity::Affine;
    assert!(
        check(&types, &input).is_err(),
        "a shared element view cannot return an affine value by copy"
    );
}

fn mapping(extent: ReferenceOriginExtent) -> ReferenceCallResult {
    ReferenceCallResult {
        path: Vec::new(),
        sources: vec![super::super::ReferenceCallSource {
            origin: ReferenceCallOrigin::Parameter {
                source: StructuralArgument {
                    place: PlaceId::new(3).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                transfers_existing: false,
            },
            suffix: Vec::new(),
            endpoint: StructuralTypeId::new(1).unwrap(),
            extent,
        }],
    }
}

fn observe(
    types: &[StructuralTypeDeclaration],
    result: &StructuralOperationResult,
    declaration: &StructuralResultDeclaration,
    mappings: &[ReferenceCallResult],
) -> (
    Result<Vec<StructuralPathQualification>, &'static str>,
    usize,
) {
    let builds = Cell::new(0);
    let outcome = preserve_call_memberships(
        types,
        &[],
        || {
            builds.set(builds.get() + 1);
            &[]
        },
        MachineId::new(1).unwrap(),
        (BlockId::new(1).unwrap(), 0),
        result,
        declaration,
        mappings,
        &|_| {
            Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: StructuralTypeId::new(1).unwrap(),
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            })
        },
    );
    (outcome, builds.get())
}

#[test]
fn undemanded_rows_still_reject_malformed_mappings_without_a_graph() {
    let (types, mut result, declaration) =
        declarations(StructuralAccess::MutableBorrow);
    let repeated = [
        mapping(ReferenceOriginExtent::Projected),
        mapping(ReferenceOriginExtent::Projected),
    ];
    assert_eq!(
        observe(&types, &result, &declaration, &repeated),
        (
            Err("returned reference membership repeats an output leaf"),
            0
        )
    );
    let mut invalid = mapping(ReferenceOriginExtent::Projected);
    invalid.path.push(StructuralPathSegment::FixedIndex(0));
    assert_eq!(
        observe(&types, &result, &declaration, &[invalid]),
        (Err("returned reference membership has no typed leaf"), 0)
    );
    result.structural_type = StructuralTypeId::new(1).unwrap();
    assert_eq!(
        observe(&types, &result, &declaration, &[]),
        (
            Err("preserved reference result has a different declared carrier"),
            0
        )
    );
}

#[test]
fn only_a_projected_shared_referent_demands_the_graph() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let (types, result, declaration) = declarations(access);
        assert_eq!(
            observe(&types, &result, &declaration, &[]),
            (Ok(Vec::new()), 0)
        );
        assert_eq!(
            observe(
                &types,
                &result,
                &declaration,
                &[mapping(ReferenceOriginExtent::WithinRegion)]
            ),
            (Ok(Vec::new()), 0)
        );
        assert_eq!(
            observe(
                &types,
                &result,
                &declaration,
                &[mapping(ReferenceOriginExtent::Projected)]
            ),
            (
                Ok(Vec::new()),
                usize::from(access == StructuralAccess::SharedBorrow)
            )
        );
    }
}
