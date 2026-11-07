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

fn mutable_lender_pair(
    root_shape: terminal_psi::StructuralTypeShape,
    child_path: Vec<StructuralPathSegment>,
    admitted_parent_access: StructuralAccess,
) -> Result<RuntimeRows<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    let declarations = [
        terminal_psi::StructuralTypeDeclaration {
            quotient_carrier: None,
            id: structural_type(1),
            identity: terminal_psi::TerminalIdentityIndex(1),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            shape: root_shape,
            scalar_restriction: None,
        },
        terminal_psi::StructuralTypeDeclaration {
            quotient_carrier: None,
            id: structural_type(2),
            identity: terminal_psi::TerminalIdentityIndex(2),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Boolean,
            ),
            scalar_restriction: None,
        },
    ];
    let mut parameters = [
        parameter(1, 0, 1, StructuralMultiplicity::Unrestricted),
        parameter(2, 1, 2, StructuralMultiplicity::Unrestricted),
    ];
    for parameter in &mut parameters {
        parameter.access = StructuralAccess::MutableBorrow;
    }
    let values = [value(7, 1, Vec::new()), value(7, 2, child_path)];
    let sources = [50, 51].map(|raw| StructuralArgument {
        place: place(raw),
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    });
    let mut source_parameters = parameters.clone();
    source_parameters[0].place = place(50);
    source_parameters[0].access = admitted_parent_access;
    source_parameters[1].place = place(51);
    let provenance =
        source_parameters
            .each_ref()
            .map(|parameter| super::StructuralArgumentProvenance {
                parameter: Some(parameter),
                reference_projection: None,
            });
    // External input cannot turn these equal backing identities into lineage.
    assert_eq!(
        bind_structural_arguments(&parameters, &values, None).unwrap_err(),
        TerminalInterpretError::StructuralArgumentAliasing(7),
    );
    super::bind_active_retained_arguments(
        &parameters,
        &sources,
        &values,
        &provenance,
        &crate::embedding::program::ProgramDeclarations::new(&declarations),
        &BTreeMap::new(),
        None,
    )
}

fn boolean_array() -> terminal_psi::StructuralTypeShape {
    terminal_psi::StructuralTypeShape::FixedArray {
        element: structural_type(2),
        length: terminal_psi::ArrayExtent::Literal(2),
    }
}

#[test]
fn internal_call_retains_admitted_mutable_parent_and_inline_child() {
    let values = mutable_lender_pair(
        boolean_array(),
        vec![StructuralPathSegment::FixedIndex(0)],
        StructuralAccess::MutableBorrow,
    )
    .unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(
        values.get(&place(2)).unwrap().path,
        vec![StructuralPathSegment::FixedIndex(0)]
    );
}

#[test]
fn retained_mutable_parent_does_not_admit_duplicate_extent_or_shared_source() {
    for (path, access) in [
        (Vec::new(), StructuralAccess::MutableBorrow),
        (
            vec![StructuralPathSegment::FixedIndex(0)],
            StructuralAccess::SharedBorrow,
        ),
    ] {
        assert_eq!(
            mutable_lender_pair(boolean_array(), path, access).unwrap_err(),
            TerminalInterpretError::StructuralArgumentAliasing(7),
        );
    }
}

#[test]
fn stored_pointer_cannot_supply_inline_child_lineage() {
    assert_eq!(
        mutable_lender_pair(
            terminal_psi::StructuralTypeShape::Reference {
                referent: structural_type(2),
                access: StructuralAccess::MutableBorrow,
            },
            vec![StructuralPathSegment::Referent],
            StructuralAccess::MutableBorrow,
        )
        .unwrap_err(),
        TerminalInterpretError::StructuralArgumentAliasing(7),
    );
}
