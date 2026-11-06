//! Claim transfer and argument aliasing are pure custody joins over one
//! call or edge: each accepted composition sits beside the refusal that
//! keeps it exact.
use super::{bind_structural_arguments, transfer_claims};
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
        additional_input_qualifications: Vec::new(),
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

#[test]
fn a_resolved_reference_can_arrive_beside_its_held_owner() {
    let parameters = retained_owner_parameters(StructuralMultiplicity::Affine);
    let arguments = [value(7, 3, Vec::new()), value(7, 3, Vec::new())];
    let bound =
        bind_saved_reference(&parameters, &arguments, Some(value(7, 3, Vec::new()))).unwrap();
    assert_eq!(bound.len(), 2);
    // Independent calls receive no internal state-arrival provenance.
    assert!(bind_structural_arguments(&parameters, &arguments, None).is_err());
}

#[test]
fn equal_runtime_values_do_not_replace_missing_or_other_reference_provenance() {
    let parameters = retained_owner_parameters(StructuralMultiplicity::Affine);
    let arguments = [value(7, 3, Vec::new()), value(7, 3, Vec::new())];
    for source in [
        None,
        Some(value(8, 3, Vec::new())),
        Some(value(7, 3, vec![field("left")])),
    ] {
        assert!(bind_saved_reference(&parameters, &arguments, source).is_err());
    }
}

#[test]
fn reference_provenance_cannot_duplicate_linear_custody_or_exclusive_loans() {
    let arguments = [value(7, 3, Vec::new()), value(7, 3, Vec::new())];
    let mut parameters = retained_owner_parameters(StructuralMultiplicity::Linear);
    assert!(bind_saved_reference(&parameters, &arguments, Some(arguments[1].clone())).is_err());
    parameters[0].access = StructuralAccess::MutableBorrow;
    parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    assert!(bind_saved_reference(&parameters, &arguments, Some(arguments[1].clone())).is_err());
}

#[test]
fn a_write_only_child_can_arrive_beside_its_retained_mutable_parent() {
    let mut parent = parameter(5, 0, 3, StructuralMultiplicity::Unrestricted);
    parent.access = StructuralAccess::MutableBorrow;
    let arguments = [value(7, 3, Vec::new()), value(7, 3, Vec::new())];
    assert!(bind_retained_parent(Some(&parent), &arguments, Some(&arguments[1])).is_ok());
    parent.access = StructuralAccess::SharedBorrow;
    assert!(bind_retained_parent(Some(&parent), &arguments, Some(&arguments[1])).is_err());
    assert!(bind_retained_parent(None, &arguments, Some(&arguments[1])).is_err());
    parent.access = StructuralAccess::MutableBorrow;
    assert!(bind_retained_parent(Some(&parent), &arguments, None).is_err());
    assert!(
        bind_retained_parent(Some(&parent), &arguments, Some(&value(8, 3, Vec::new()))).is_err()
    );
}

fn bind_retained_parent(
    parent: Option<&StructuralParameterDeclaration>,
    arguments: &[TerminalStructuralValue],
    reference: Option<&TerminalStructuralValue>,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    let mut held = parameter(1, 0, 3, StructuralMultiplicity::Unrestricted);
    held.access = StructuralAccess::SharedBorrow;
    let mut child = parameter(2, 1, 3, StructuralMultiplicity::Unrestricted);
    child.access = StructuralAccess::WriteOnlyBorrow;
    let parameters = [held, child];
    assert!(bind_structural_arguments(&parameters, arguments, None).is_err());
    super::bind_existing_structural_places(
        &parameters,
        arguments,
        &[
            StructuralArgument {
                place: place(5),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            StructuralArgument {
                place: place(9),
                path: vec![StructuralPathSegment::Referent],
                access: StructuralAccess::WriteOnlyBorrow,
            },
        ],
        &[
            super::BlockArgumentProvenance {
                parameter: parent,
                reference_projection: None,
            },
            super::BlockArgumentProvenance {
                parameter: None,
                reference_projection: reference
                    .map(crate::boundary::values::StructuralRuntimePlace::from),
            },
        ],
        None,
    )
}

fn retained_owner_parameters(owner: StructuralMultiplicity) -> [StructuralParameterDeclaration; 2] {
    let mut loan = parameter(2, 1, 3, StructuralMultiplicity::Unrestricted);
    loan.access = StructuralAccess::MutableBorrow;
    [parameter(1, 0, 3, owner), loan]
}

fn bind_saved_reference(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
    reference: Option<TerminalStructuralValue>,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    super::bind_existing_structural_places(
        parameters,
        arguments,
        &[
            StructuralArgument {
                place: place(5),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
            StructuralArgument {
                place: place(9),
                path: vec![StructuralPathSegment::Referent],
                access: StructuralAccess::MutableBorrow,
            },
        ],
        &[
            super::BlockArgumentProvenance::default(),
            super::BlockArgumentProvenance {
                parameter: None,
                reference_projection: reference
                    .as_ref()
                    .map(crate::boundary::values::StructuralRuntimePlace::from),
            },
        ],
        None,
    )
}
