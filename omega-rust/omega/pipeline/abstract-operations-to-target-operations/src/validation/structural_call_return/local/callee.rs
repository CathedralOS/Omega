//! Local callee grammar and target-carrier replay.

use abstract_operations::{AbstractFunction, AbstractOperation};
use target_operations::{TargetFunction, TargetOperation};

use super::super::{
    StructuralCallReturnProjectedQualificationValidationError as Error,
    StructuralParameterReturnCalleeTranslationReceipt,
};

pub(crate) fn is_candidate(source: &AbstractFunction) -> bool {
    matches!((source.structural_parameters.as_slice(), source.operations.as_slice()),
        ([parameter], [AbstractOperation::ReturnStructural { source: returned, .. }])
            if *returned == parameter.place && !parameter.projected_qualifications.is_empty())
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<StructuralParameterReturnCalleeTranslationReceipt, Error> {
    let ([parameter], Some(result)) = (
        source.structural_parameters.as_slice(),
        source.result.structural(),
    ) else {
        return Err(Error::SourceShape);
    };
    let [
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
    let TargetOperation::ReturnStructuralParameter {
        scalar_parameters,
        parameters,
        source: target_source,
        result: target_result,
        psi_edge: target_edge,
        returned_claims: target_claims,
        trivial_affine_locals: target_locals,
        trivial_affine_discards: target_discards,
        ..
    } = &target.operation
    else {
        return Err(Error::TargetShape);
    };
    if !scalar_parameters.is_empty()
        || target.machine != source.machine
        || target.provenance.operations
            != trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect::<Vec<_>>()
        || target.provenance.edges.as_slice() != [*psi_edge]
        || parameters != &source.structural_parameters
        || target_source != parameter
        || target_result != result
        || target_edge != psi_edge
        || target_claims != returned_claims
        || target_locals != trivial_affine_locals
        || target_discards != trivial_affine_discards
        || *returned_source != parameter.place
    {
        return Err(Error::TargetShape);
    }
    Ok(StructuralParameterReturnCalleeTranslationReceipt::new(
        source.machine,
        parameter.projected_qualifications.clone(),
    ))
}
