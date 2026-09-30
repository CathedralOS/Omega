//! Each direct-call mode spells exactly one Terminal operation, and the read
//! accessors recover the facts from every spelling.

use super::{
    DirectCall, DirectCallResult, DirectCallRole, ScalarCallFrame, call_structural_arguments,
    call_structural_arguments_mut, direct_call_parts,
};
use crate::terminal_identities::{
    boundary_machine_id, claim_id, machine_id, obligation_id, place_id, value_id,
};
use semantic_vocabulary::{ScalarTerm, ScalarType};
use terminal_psi::{
    ClaimTransfer, OperationKind, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralResultClaimTransfer,
};

fn structural_argument(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: place_id(place),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn boolean_term(value: u64) -> ScalarTerm {
    ScalarTerm::Value {
        id: value_id(value),
        scalar_type: ScalarType::Boolean,
    }
}

/// A call with no operand lanes, to which each test adds the lanes it needs.
fn bare_call(result: DirectCallResult) -> DirectCall {
    DirectCall {
        callee: machine_id(7),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: vec![obligation_id(3)],
        crash_continuations: Vec::new(),
        result,
    }
}

#[test]
fn the_scalar_frame_is_structural_when_any_custody_fact_crosses() {
    for structural_argument in [false, true] {
        for claim_transfer in [false, true] {
            for callee_frame in [false, true] {
                let expected = if structural_argument || claim_transfer || callee_frame {
                    ScalarCallFrame::Structural
                } else {
                    ScalarCallFrame::ScalarOnly
                };
                assert_eq!(
                    ScalarCallFrame::for_call(structural_argument, claim_transfer, callee_frame),
                    expected
                );
            }
        }
    }
}

#[test]
fn a_scalar_only_frame_spells_an_ordinary_call() {
    let mut call = bare_call(DirectCallResult::Scalar(ScalarCallFrame::ScalarOnly));
    call.arguments = vec![value_id(1), value_id(2)];
    call.erased_arguments = vec![boolean_term(1)];
    assert_eq!(
        call.into_operation_kind(),
        OperationKind::Call {
            callee: machine_id(7),
            arguments: vec![value_id(1), value_id(2)],
            erased_arguments: vec![boolean_term(1)],
            erased_proof_arguments: Vec::new(),
            requirement_obligations: vec![obligation_id(3)],
            crash_continuations: Vec::new(),
        }
    );
}

/// A structural frame keeps its form without any structural operand: the
/// callee's body, or the route, owns the frame.
#[test]
fn a_structural_frame_spells_a_structural_scalar_call_even_without_operands() {
    assert_eq!(
        bare_call(DirectCallResult::Scalar(ScalarCallFrame::Structural)).into_operation_kind(),
        OperationKind::CallStructuralScalar {
            callee: machine_id(7),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: vec![obligation_id(3)],
            crash_continuations: Vec::new(),
        }
    );
    let mut call = bare_call(DirectCallResult::Scalar(ScalarCallFrame::Structural));
    call.structural_arguments = vec![structural_argument(4)];
    call.claim_transfers = vec![ClaimTransfer {
        claim: claim_id(1),
        argument_index: 0,
    }];
    assert!(matches!(
        call.into_operation_kind(),
        OperationKind::CallStructuralScalar { structural_arguments, claim_transfers, .. }
            if structural_arguments == [structural_argument(4)] && claim_transfers.len() == 1
    ));
}

#[test]
fn a_unit_result_spells_a_unit_call_with_every_lane() {
    let mut call = bare_call(DirectCallResult::Unit);
    call.arguments = vec![value_id(5)];
    call.structural_arguments = vec![structural_argument(4)];
    assert_eq!(
        call.into_operation_kind(),
        OperationKind::CallUnit {
            callee: machine_id(7),
            arguments: vec![value_id(5)],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![structural_argument(4)],
            claim_transfers: Vec::new(),
            requirement_obligations: vec![obligation_id(3)],
            crash_continuations: Vec::new(),
        }
    );
}

fn structural_result(multiplicity: StructuralMultiplicity) -> DirectCallResult {
    DirectCallResult::Structural {
        multiplicity,
        returned_claim_transfers: vec![StructuralResultClaimTransfer {
            callee_claim: claim_id(1),
            caller_claim: claim_id(2),
        }],
    }
}

#[test]
fn only_a_scalar_free_linear_result_spells_the_claim_bearing_structural_call() {
    let mut call = bare_call(structural_result(StructuralMultiplicity::Linear));
    call.structural_arguments = vec![structural_argument(4)];
    assert_eq!(
        call.into_operation_kind(),
        OperationKind::CallStructural {
            callee: machine_id(7),
            structural_arguments: vec![structural_argument(4)],
            claim_transfers: Vec::new(),
            returned_claim_transfers: vec![StructuralResultClaimTransfer {
                callee_claim: claim_id(1),
                caller_claim: claim_id(2),
            }],
            requirement_obligations: vec![obligation_id(3)],
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        }
    );

    // A runtime scalar actual, a proof-only actual, or a non-linear result
    // each select the scalar-argument form.
    let mut with_argument = bare_call(structural_result(StructuralMultiplicity::Linear));
    with_argument.arguments = vec![value_id(5)];
    let mut with_erased = bare_call(structural_result(StructuralMultiplicity::Linear));
    with_erased.erased_arguments = vec![boolean_term(5)];
    for call in [
        with_argument,
        with_erased,
        bare_call(structural_result(StructuralMultiplicity::Affine)),
        bare_call(structural_result(StructuralMultiplicity::Unrestricted)),
    ] {
        assert!(matches!(
            call.into_operation_kind(),
            OperationKind::CallStructuralWithScalarArguments { returned_claim_transfers, .. }
                if returned_claim_transfers.len() == 1
        ));
    }
}

#[test]
fn the_payloadless_case_spells_the_bounded_structural_call() {
    let mut call = bare_call(DirectCallResult::PayloadlessCase);
    call.requirement_obligations = Vec::new();
    assert_eq!(
        call.into_operation_kind(),
        OperationKind::CallStructural {
            callee: machine_id(7),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        }
    );
}

#[test]
fn the_read_accessors_recover_every_spelling() {
    // (mode, role it reads back as, whether it carries a scalar actual,
    // whether it carries a structural argument)
    let spellings = [
        (
            DirectCallResult::Scalar(ScalarCallFrame::ScalarOnly),
            DirectCallRole::Scalar,
            true,
            false,
        ),
        (
            DirectCallResult::Scalar(ScalarCallFrame::Structural),
            DirectCallRole::Scalar,
            true,
            true,
        ),
        (DirectCallResult::Unit, DirectCallRole::Unit, true, true),
        (
            structural_result(StructuralMultiplicity::Linear),
            DirectCallRole::Structural,
            false,
            true,
        ),
        (
            structural_result(StructuralMultiplicity::Affine),
            DirectCallRole::Structural,
            true,
            true,
        ),
    ];
    for (result, role, carries_argument, carries_structural_argument) in spellings {
        let arguments = if carries_argument {
            vec![value_id(5)]
        } else {
            Vec::new()
        };
        let structural_arguments = if carries_structural_argument {
            vec![structural_argument(4)]
        } else {
            Vec::new()
        };
        let mut call = bare_call(result);
        call.arguments = arguments.clone();
        call.structural_arguments = structural_arguments.clone();
        let mut kind = call.into_operation_kind();
        let parts = direct_call_parts(&kind).expect("a direct call spelling");
        assert_eq!(parts.callee, machine_id(7));
        assert_eq!(parts.role, role);
        assert_eq!(parts.arguments, arguments.as_slice());
        assert_eq!(parts.structural_arguments, structural_arguments.as_slice());
        assert_eq!(
            call_structural_arguments(&kind),
            structural_arguments.as_slice()
        );
        assert_eq!(
            call_structural_arguments_mut(&mut kind).is_some(),
            carries_structural_argument
        );
    }

    let mut boundary = OperationKind::BoundaryCall {
        boundary: boundary_machine_id(2),
        arguments: vec![value_id(5)],
        structural_arguments: vec![structural_argument(4)],
        completion_receipts: Vec::new(),
        requirement_obligations: Vec::new(),
    };
    assert!(direct_call_parts(&boundary).is_none());
    assert_eq!(
        call_structural_arguments(&boundary),
        [structural_argument(4)]
    );
    call_structural_arguments_mut(&mut boundary).expect("a boundary call has a structural lane")
        [0]
    .place = place_id(9);
    assert_eq!(call_structural_arguments(&boundary)[0].place, place_id(9));
    assert!(direct_call_parts(&OperationKind::BooleanConstant { value: true }).is_none());
}
