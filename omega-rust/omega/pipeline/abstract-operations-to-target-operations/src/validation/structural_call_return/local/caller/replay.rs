//! Exact caller source/target reconstruction below the local family entrance.

use abstract_operations::{AbstractFunction, AbstractOperation};
use target_operations::{TargetFunction, TargetOperation};

use super::{Error, StructuralCallReturnCallerTranslationReceipt};

pub(super) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<StructuralCallReturnCallerTranslationReceipt, Error> {
    let ([parameter], Some(result)) = (
        source.structural_parameters.as_slice(),
        source.result.structural(),
    ) else {
        return Err(Error::SourceShape);
    };
    let [
        AbstractOperation::CallStructural {
            psi_operation,
            result: operation_result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        },
        AbstractOperation::ReturnStructural {
            psi_edge,
            source: returned_source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        },
    ] = source.operations.as_slice()
    else {
        return Err(Error::SourceShape);
    };
    let TargetOperation::ReturnStructuralCall {
        psi_edge: target_edge,
        psi_operation: target_operation,
        operation_result: target_operation_result,
        result: target_result,
        callee: target_callee,
        structural_parameters,
        arguments,
        claim_transfers: target_claim_transfers,
        returned_claim_transfers: target_returned_transfers,
        returned_claims: target_returned_claims,
        requirement_obligations: target_requirements,
        crash_continuations: target_crashes,
        ..
    } = &target.operation
    else {
        return Err(Error::TargetShape);
    };
    let ([target_parameter], [argument]) = (structural_parameters.as_slice(), arguments.as_slice())
    else {
        return Err(Error::TargetShape);
    };
    if target.machine != source.machine
        || target.provenance.operations.as_slice() != [*psi_operation]
        || target.provenance.edges.as_slice() != [*psi_edge]
        || target_edge != psi_edge
        || target_operation != psi_operation
        || target_operation_result != operation_result
        || target_result != result
        || target_callee != callee
        || target_parameter.place != parameter.place
        || target_parameter.structural_type != parameter.structural_type
        || target_parameter.multiplicity != parameter.multiplicity
        || target_parameter.access != parameter.access
        || target_parameter.projected_qualifications != parameter.projected_qualifications
        || argument.place != structural_arguments[0].place
        || argument.access != structural_arguments[0].access
        || argument.path != structural_arguments[0].path
        || operation_result.place != *returned_source
        || target_claim_transfers != claim_transfers
        || target_returned_transfers != returned_claim_transfers
        || target_returned_claims != returned_claims
        || target_requirements != requirement_obligations
        || target_crashes != crash_continuations
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return Err(Error::TargetShape);
    }
    Ok(StructuralCallReturnCallerTranslationReceipt::new(
        source.machine,
        *callee,
        parameter.projected_qualifications.clone(),
    ))
}
