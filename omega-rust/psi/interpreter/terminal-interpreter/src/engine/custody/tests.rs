//! Claim transfer and argument aliasing are pure custody joins over one
//! call or edge: each accepted composition sits beside the refusal that
//! keeps it exact.
use super::{bind_structural_arguments, require_owned_value_claims, transfer_claims};
use crate::boundary::errors::TerminalInterpretError;
use crate::boundary::values::TerminalStructuralValue;
use crate::engine::execution::LiveClaim;
use crate::storage::runtime_rows::RuntimeRows;
use semantic_vocabulary::{ClaimId, PlaceId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{
    ClaimTransfer, EntryClaim, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment,
};

fn nominal_value_types() -> Vec<terminal_psi::StructuralTypeDeclaration> {
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralRecordStorage, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalIdentityIndex,
    };
    let token_field = |index| StructuralFieldDeclaration {
        inline_carrier: None,
        id: semantic_vocabulary::StructuralFieldId::new(index).unwrap(),
        identity: TerminalIdentityIndex(index as u32),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type(1)),
        storage: None,
    };
    let declaration = |index, owns_linear_claim, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        id: structural_type(index),
        identity: TerminalIdentityIndex(index as u32),
        owns_linear_claim,
        copyable: false,
        no_code_affine_cleanup: !owns_linear_claim,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    };
    vec![
        declaration(
            1,
            true,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: Vec::new(),
            },
        ),
        declaration(
            2,
            false,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![token_field(1), token_field(2)],
            },
        ),
        declaration(
            3,
            false,
            StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                        identity: TerminalIdentityIndex(3),
                        fields: vec![token_field(1)],
                    },
                    StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                        identity: TerminalIdentityIndex(4),
                        fields: vec![token_field(2)],
                    },
                    StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(3).unwrap(),
                        identity: TerminalIdentityIndex(5),
                        fields: Vec::new(),
                    },
                ],
            },
        ),
        declaration(
            4,
            false,
            StructuralTypeShape::FixedArray {
                element: structural_type(3),
                length: terminal_psi::ArrayExtent::Literal(2),
            },
        ),
    ]
}

#[test]
fn zero_byte_nominal_value_needs_one_live_root_claim() {
    let declarations = nominal_value_types();
    let types = crate::embedding::program::ProgramDeclarations::new(&declarations);
    let argument = StructuralArgument {
        place: place(5),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    };
    let check = |claims: &BTreeMap<ClaimId, LiveClaim>| {
        require_owned_value_claims(&types, claims, structural_type(1), &argument, &|_, _| false)
    };
    assert_eq!(
        check(&BTreeMap::new()),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
    let mut claims = BTreeMap::from([(claim(1), linear(5, Vec::new()))]);
    assert_eq!(check(&claims), Ok(()));
    claims.insert(claim(2), linear(5, Vec::new()));
    assert_eq!(
        check(&claims),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
}

#[test]
fn nominal_projection_requires_its_own_claim_without_claiming_the_affine_parent() {
    let declarations = nominal_value_types();
    let types = crate::embedding::program::ProgramDeclarations::new(&declarations);
    let mut argument = StructuralArgument {
        place: place(5),
        path: vec![field("left")],
        access: StructuralAccess::Owned,
    };
    let mut claims = BTreeMap::from([
        (claim(1), linear(5, vec![field("left")])),
        (claim(2), linear(5, vec![field("right")])),
    ]);
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(2), &argument, &|_, _| {
            false
        }),
        Ok(())
    );
    claims.remove(&claim(1));
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(2), &argument, &|_, _| {
            false
        }),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
    claims.insert(claim(3), linear(5, Vec::new()));
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(2), &argument, &|_, _| {
            false
        }),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
    claims.remove(&claim(3));
    claims.insert(claim(1), linear(5, vec![field("left")]));
    argument.path.clear();
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(2), &argument, &|_, _| {
            false
        }),
        Ok(())
    );
    claims.remove(&claim(1));
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(2), &argument, &|_, _| {
            false
        }),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
}

#[test]
fn nominal_case_projection_cannot_use_another_cases_claim() {
    let declarations = nominal_value_types();
    let types = crate::embedding::program::ProgramDeclarations::new(&declarations);
    let case = |index| {
        StructuralPathSegment::Case(semantic_vocabulary::StructuralCaseId::new(index).unwrap())
    };
    let argument = StructuralArgument {
        place: place(5),
        path: vec![case(2), field("right")],
        access: StructuralAccess::Owned,
    };
    let selected = |path: &[StructuralPathSegment], selected| {
        path.is_empty() && selected == semantic_vocabulary::StructuralCaseId::new(2).unwrap()
    };
    let mut claims = BTreeMap::from([(claim(1), linear(5, vec![case(1), field("left")]))]);
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(3), &argument, &selected),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
    claims.clear();
    claims.insert(claim(2), linear(5, argument.path.clone()));
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(3), &argument, &selected),
        Ok(())
    );
    let whole = StructuralArgument {
        path: Vec::new(),
        ..argument
    };
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(3), &whole, &selected),
        Ok(())
    );
    assert_eq!(
        require_owned_value_claims(
            &types,
            &BTreeMap::new(),
            structural_type(3),
            &whole,
            &selected
        ),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
}

#[test]
fn owned_array_claims_follow_each_actual_case_including_empty_cases() {
    let declarations = nominal_value_types();
    let types = crate::embedding::program::ProgramDeclarations::new(&declarations);
    let argument = StructuralArgument {
        place: place(5),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    };
    let empty = semantic_vocabulary::StructuralCaseId::new(3).unwrap();
    let held = semantic_vocabulary::StructuralCaseId::new(2).unwrap();
    let selected = |path: &[StructuralPathSegment], case| match path {
        [StructuralPathSegment::FixedIndex(0)] => case == empty,
        [StructuralPathSegment::FixedIndex(1)] => case == held,
        _ => false,
    };
    let claims = BTreeMap::from([(
        claim(1),
        linear(
            5,
            vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Case(held),
                field("right"),
            ],
        ),
    )]);
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(4), &argument, &selected),
        Ok(())
    );
    assert_eq!(
        require_owned_value_claims(
            &types,
            &BTreeMap::new(),
            structural_type(4),
            &argument,
            &selected
        ),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
    assert_eq!(
        require_owned_value_claims(
            &types,
            &BTreeMap::new(),
            structural_type(4),
            &argument,
            &|_, case| case == empty
        ),
        Ok(())
    );
}

#[test]
fn guarded_frontier_keeps_all_unexcluded_case_claims() {
    let declarations = nominal_value_types();
    let types = crate::embedding::program::ProgramDeclarations::new(&declarations);
    let argument = StructuralArgument {
        place: place(5),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    };
    let case = |index| {
        StructuralPathSegment::Case(semantic_vocabulary::StructuralCaseId::new(index).unwrap())
    };
    let mut claims = BTreeMap::from([
        (claim(1), linear(5, vec![case(1), field("left")])),
        (claim(2), linear(5, vec![case(2), field("right")])),
    ]);
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(3), &argument, &|_, _| true),
        Ok(())
    );
    claims.remove(&claim(1));
    assert_eq!(
        require_owned_value_claims(&types, &claims, structural_type(3), &argument, &|_, _| true),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
}

fn place(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn claim(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}

fn structural_type(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn field(name: &str) -> StructuralPathSegment {
    // The custody structures under test compare segments by index equality;
    // names map to fixed table positions rather than a shared module table.
    let index = match name {
        "left" => 1,
        "right" => 2,
        "middle" => 3,
        _ => 0,
    };
    StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(index))
}

fn parameter(
    raw_place: u64,
    position: u32,
    raw_type: u64,
    multiplicity: StructuralMultiplicity,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(raw_place),
        position,
        structural_type: structural_type(raw_type),
        multiplicity,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn value(
    opaque_identity: u64,
    raw_type: u64,
    path: Vec<StructuralPathSegment>,
) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity,
        structural_type: structural_type(raw_type),
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        path,
    }
}

fn linear(raw_place: u64, path: Vec<StructuralPathSegment>) -> LiveClaim {
    LiveClaim {
        place: Some(place(raw_place)),
        path,
        multiplicity: Some(StructuralMultiplicity::Linear),
    }
}

/// A caller holding claim 2 at `right` and claim 3 at `left` lends both
/// through one argument to a callee whose entry claims are 1 at `left` and
/// 2 at `right`, as `settle(pair)` after `pair.left = make(5)` does.
fn pair_transfer(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError> {
    let callee = parameter(2, 0, 1, StructuralMultiplicity::Affine);
    let callee_values =
        RuntimeRows::from_rows(None, [(callee.place, value(9, 1, Vec::new()))]).unwrap();
    transfer_claims(
        &crate::embedding::program::ProgramDeclarations::new(&[]),
        &BTreeMap::new(),
        caller_claims,
        &[StructuralArgument {
            place: place(5),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        &[
            ClaimTransfer {
                claim: claim(2),
                argument_index: 0,
                path: vec![field("right")],
            },
            ClaimTransfer {
                claim: claim(3),
                argument_index: 0,
                path: vec![field("left")],
            },
        ],
        std::slice::from_ref(&callee),
        &[
            EntryClaim {
                claim: claim(1),
                input: callee.place,
                path: vec![field("left")],
            },
            EntryClaim {
                claim: claim(2),
                input: callee.place,
                path: vec![field("right")],
            },
        ],
        &[],
        &callee_values,
    )
}

#[test]
fn claim_transfers_join_entry_claims_by_path_not_position() {
    let caller = BTreeMap::from([
        (claim(2), linear(5, vec![field("right")])),
        (claim(3), linear(5, vec![field("left")])),
    ]);
    let (remaining, callee) = pair_transfer(&caller).unwrap();
    assert!(remaining.is_empty());
    assert_eq!(callee[&claim(1)], linear(2, vec![field("left")]));
    assert_eq!(callee[&claim(2)], linear(2, vec![field("right")]));
}

#[test]
fn claim_transfer_without_its_entry_path_refuses() {
    let caller = BTreeMap::from([
        (claim(2), linear(5, vec![field("right")])),
        (claim(3), linear(5, vec![field("middle")])),
    ]);
    assert_eq!(
        pair_transfer(&caller),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
}

#[test]
fn claim_transfers_cannot_match_one_entry_claim_twice() {
    let caller = BTreeMap::from([
        (claim(2), linear(5, vec![field("right")])),
        (claim(3), linear(5, vec![field("right")])),
    ]);
    assert_eq!(
        pair_transfer(&caller),
        Err(TerminalInterpretError::ClaimTransferMismatch)
    );
}

#[test]
fn disjoint_projections_of_one_root_bind_as_separate_arguments() {
    let parameters = [
        parameter(1, 0, 3, StructuralMultiplicity::Affine),
        parameter(2, 1, 3, StructuralMultiplicity::Affine),
    ];
    let bound = bind_structural_arguments(
        &parameters,
        &[
            value(7, 3, vec![field("right")]),
            value(7, 3, vec![field("left")]),
        ],
        None,
    )
    .unwrap();
    assert_eq!(bound.len(), 2);
}

#[test]
fn overlapping_custody_bearing_arguments_still_alias() {
    let parameters = [
        parameter(1, 0, 3, StructuralMultiplicity::Affine),
        parameter(2, 1, 3, StructuralMultiplicity::Affine),
    ];
    assert_eq!(
        bind_structural_arguments(
            &parameters,
            &[
                value(7, 3, vec![field("left")]),
                value(7, 3, vec![field("left")]),
            ],
            None,
        )
        .unwrap_err(),
        TerminalInterpretError::StructuralArgumentAliasing(7)
    );
}
