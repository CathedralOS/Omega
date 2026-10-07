//! Ordinary, structural, boundary, and service-effect call tags.

use crate::optimization_unit::identity::carrier_encoding::{
    encode_abstract_result, encode_ids, encode_scalar_type, encode_structural_argument,
};
use crate::optimization_unit::identity::structural_encoding::{
    encode_completion_claim_source, encode_crash_route_bucket,
    encode_outcome_specific_call_evidence, encode_structural_operation_result,
};

use super::{AbstractOperation, CanonicalBytes};
pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::CallStructural {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            result,
            requirement_obligations,
            crash_continuations,
        } => {
            use abstract_operations::AbstractStructuralCallResult as R;
            match result {
                R::Unit | R::Scalar(_) => {
                    // Each result keeps the tag and field order of the variant
                    // it replaced, so folding the Unit and scalar-result calls
                    // into one variant moved no identity.
                    bytes.u8(if matches!(result, R::Unit) { 3 } else { 4 });
                    bytes.id(*psi_operation);
                    if let Some(result) = result.scalar() {
                        encode_abstract_result(bytes, result);
                    }
                    bytes.id(*callee);
                    encode_ids(bytes, arguments);
                    bytes.slice(structural_arguments, encode_structural_argument);
                    encode_ordinary_call_tail(
                        bytes,
                        claim_transfers,
                        requirement_obligations,
                        crash_continuations,
                    );
                }
                R::Structural {
                    result,
                    returned_claim_transfers,
                    selected_evidence,
                } => {
                    // A structural result keeps tag 5 and its legacy field
                    // order.
                    bytes.u8(5);
                    bytes.id(*psi_operation);
                    encode_structural_operation_result(bytes, result);
                    bytes.id(*callee);
                    encode_ids(bytes, arguments);
                    bytes.slice(structural_arguments, encode_structural_argument);
                    bytes.slice(claim_transfers, |bytes, transfer| {
                        bytes.id(transfer.claim);
                        bytes.u32(transfer.argument_index);
                    });
                    bytes.slice(returned_claim_transfers, |bytes, transfer| {
                        bytes.id(transfer.callee_claim);
                        bytes.id(transfer.caller_claim);
                    });
                    encode_ids(bytes, requirement_obligations);
                    bytes.slice(crash_continuations, encode_crash_route_bucket);
                    bytes.slice(selected_evidence, encode_outcome_specific_call_evidence);
                }
            }
        }
        O::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.u8(6);
            bytes.id(*psi_operation);
            match result {
                abstract_operations::AbstractBoundaryResult::Unit => bytes.u8(0),
                abstract_operations::AbstractBoundaryResult::Scalar(result) => {
                    bytes.u8(1);
                    encode_abstract_result(bytes, *result);
                }
                abstract_operations::AbstractBoundaryResult::Structural(result) => {
                    bytes.u8(2);
                    encode_structural_operation_result(bytes, result);
                }
            }
            bytes.id(*boundary);
            encode_ids(bytes, arguments);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(completion_claim_sources, encode_completion_claim_source);
            bytes.slice(completion_receipts, |bytes, receipt| {
                bytes.id(receipt.claim);
                bytes.u32(receipt.argument_index);
            });
        }
        O::PortWrite {
            psi_operation,
            service,
            port,
            value,
        } => {
            bytes.u8(7);
            bytes.id(*psi_operation);
            bytes.id(*service);
            bytes.u16(*port);
            bytes.u8(*value);
        }
        O::Call {
            psi_operation,
            result,
            scalar_type,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => {
            bytes.u8(8);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            bytes.id(*callee);
            encode_ids(bytes, arguments);
            encode_ids(bytes, requirement_obligations);
            bytes.slice(crash_continuations, encode_crash_route_bucket);
        }
        _ => unreachable!("operation family routing admitted a non-call operation"),
    }
}

fn encode_ordinary_call_tail(
    bytes: &mut CanonicalBytes,
    claim_transfers: &[terminal_psi::ClaimTransfer],
    requirement_obligations: &[semantic_vocabulary::ObligationId],
    crash_continuations: &[terminal_psi::CrashRouteBucket],
) {
    bytes.slice(claim_transfers, |bytes, transfer| {
        bytes.id(transfer.claim);
        bytes.u32(transfer.argument_index);
    });
    encode_ids(bytes, requirement_obligations);
    bytes.slice(crash_continuations, encode_crash_route_bucket);
}
