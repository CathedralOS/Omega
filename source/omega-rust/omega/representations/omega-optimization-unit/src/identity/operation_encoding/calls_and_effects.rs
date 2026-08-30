//! Ordinary, structural, boundary, and service-effect call tags.

use super::*;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::CallUnit {
            psi_operation,
            callee,
            structural_arguments,
            claim_transfers,
        } => {
            bytes.u8(3);
            bytes.id(*psi_operation);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
        }
        O::CallStructuralScalar {
            psi_operation,
            result,
            callee,
            structural_arguments,
            claim_transfers,
        } => {
            bytes.u8(4);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
        }
        O::CallStructural {
            psi_operation,
            result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => {
            bytes.u8(5);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*callee);
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
            encode_optional(bytes, result.as_ref(), |bytes, result| {
                encode_abstract_result(bytes, *result)
            });
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
        } => {
            bytes.u8(8);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            bytes.id(*callee);
            encode_ids(bytes, arguments);
        }
        _ => unreachable!("operation family routing admitted a non-call operation"),
    }
}
