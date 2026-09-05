//! Independent source-closure reconstruction. This module does not import target lowering.

use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

use super::layout;
use super::model::{
    StructuralCallReturnProjectedQualificationValidationError as Error,
    StructuralCallReturnRosterLocation as Location, StructuralCallReturnSource, is_canonical,
};

pub(super) fn reconstruct(
    source: &AbstractOperationPlan,
) -> Result<StructuralCallReturnSource, Error> {
    if source.functions.len() != 2 {
        return Err(Error::SourceShape);
    }
    let caller = source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .ok_or(Error::SourceShape)?;
    let [caller_parameter] = caller.structural_parameters.as_slice() else {
        return Err(Error::SourceShape);
    };
    let caller_result = caller.result.structural().ok_or(Error::SourceShape)?;
    let [
        AbstractOperation::CallStructural {
            result: operation_result,
            callee,
            structural_arguments,
            ..
        },
        AbstractOperation::ReturnStructural {
            source: caller_return_source,
            ..
        },
    ] = caller.operations.as_slice()
    else {
        return Err(Error::SourceShape);
    };
    let [argument] = structural_arguments.as_slice() else {
        return Err(Error::SourceShape);
    };
    let [caller_claim] = caller.entry_claims.as_slice() else {
        return Err(Error::SourceShape);
    };
    let AbstractOperation::CallStructural {
        claim_transfers,
        returned_claim_transfers,
        ..
    } = &caller.operations[0]
    else {
        unreachable!("the source grammar was matched above")
    };
    let [claim_transfer] = claim_transfers.as_slice() else {
        return Err(Error::SourceShape);
    };
    let [returned_transfer] = returned_claim_transfers.as_slice() else {
        return Err(Error::SourceShape);
    };
    let [operation_claim] = operation_result.claims.as_slice() else {
        return Err(Error::SourceShape);
    };
    let callee_function = source
        .functions
        .iter()
        .find(|function| function.machine == *callee)
        .ok_or(Error::SourceShape)?;
    let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
        return Err(Error::SourceShape);
    };
    let callee_result = callee_function
        .result
        .structural()
        .ok_or(Error::SourceShape)?;
    let [
        AbstractOperation::ReturnStructural {
            source: callee_source,
            returned_claims: callee_returned_claims,
            trivial_affine_locals: callee_locals,
            trivial_affine_discards: callee_discards,
            ..
        },
    ] = callee_function.operations.as_slice()
    else {
        return Err(Error::SourceShape);
    };
    let [callee_claim] = callee_function.entry_claims.as_slice() else {
        return Err(Error::SourceShape);
    };
    let AbstractOperation::ReturnStructural {
        returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
        ..
    } = &caller.operations[1]
    else {
        unreachable!("the source grammar was matched above")
    };
    let exact_block = |function: &abstract_operations::AbstractFunction| {
        matches!(function.block_entries.as_slice(), [entry]
            if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0)
    };
    if !exact_block(caller)
        || !exact_block(callee_function)
        || !caller.parameters.is_empty()
        || !caller.published_service_ceiling.is_empty()
        || caller_parameter.position != 0
        || caller_parameter.is_self
        || caller_parameter.multiplicity != StructuralMultiplicity::Linear
        || caller_parameter.access != StructuralAccess::Owned
        || argument.place != caller_parameter.place
        || argument.access != StructuralAccess::Owned
        || !argument.path.is_empty()
        || claim_transfer.argument_index != 0
        || claim_transfer.claim != caller_claim.claim
        || caller_claim.input != caller_parameter.place
        || !caller_claim.path.is_empty()
        || operation_result.place != *caller_return_source
        || operation_result.structural_type != caller_result.structural_type
        || operation_result.multiplicity != StructuralMultiplicity::Linear
        || operation_result.qualifications != caller_result.qualifications
        || operation_claim.claim != caller_claim.claim
        || !operation_claim.path.is_empty()
        || returned_transfer.caller_claim != caller_claim.claim
        || returned_claims.as_slice() != [caller_claim.claim]
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
        || !callee_function.parameters.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
        || callee_parameter.position != 0
        || callee_parameter.is_self
        || callee_parameter.multiplicity != StructuralMultiplicity::Linear
        || callee_parameter.access != StructuralAccess::Owned
        || callee_parameter.structural_type != caller_parameter.structural_type
        || callee_parameter.qualifications != caller_parameter.qualifications
        || callee_result.structural_type != caller_result.structural_type
        || callee_result.multiplicity != StructuralMultiplicity::Linear
        || callee_result.qualifications != caller_result.qualifications
        || callee_claim.input != callee_parameter.place
        || !callee_claim.path.is_empty()
        || *callee_source != callee_parameter.place
        || callee_returned_claims.as_slice() != [callee_claim.claim]
        || returned_transfer.callee_claim != callee_claim.claim
        || !callee_locals.is_empty()
        || !callee_discards.is_empty()
    {
        return Err(Error::SourceShape);
    }

    let roster = &caller_parameter.projected_qualifications;
    if !is_canonical(roster) {
        return Err(Error::SourceRosterNotCanonical(Location::CallerParameter));
    }
    require_source_roster(
        roster,
        &caller_result.projected_qualifications,
        Location::CallerFunctionResult,
    )?;
    require_source_roster(
        roster,
        &operation_result.projected_qualifications,
        Location::CallerOperationResult,
    )?;
    require_source_roster(
        roster,
        &callee_parameter.projected_qualifications,
        Location::CalleeParameter,
    )?;
    require_source_roster(
        roster,
        &callee_result.projected_qualifications,
        Location::CalleeFunctionResult,
    )?;

    let structural_types = source
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration.clone()))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let shape = layout::reconstruct(caller_parameter.structural_type, &structural_types)?;
    if shape.class != calling_conventions::ValueClass::Integer
        || !((shape.byte_size == 8 && shape.alignment == 8) || (9..=16).contains(&shape.byte_size))
    {
        return Err(Error::SourceShape);
    }
    Ok(StructuralCallReturnSource {
        caller: caller.machine,
        callee: callee_function.machine,
        roster: roster.clone(),
        caller_parameter: caller_parameter.clone(),
        caller_operation_result: operation_result.clone(),
        caller_result: caller_result.clone(),
        callee_parameter: callee_parameter.clone(),
        callee_result: callee_result.clone(),
        structural_types,
        shape,
    })
}

fn require_source_roster(
    expected: &[terminal_psi::StructuralPathQualification],
    actual: &[terminal_psi::StructuralPathQualification],
    location: Location,
) -> Result<(), Error> {
    if !is_canonical(actual) {
        return Err(Error::SourceRosterNotCanonical(location));
    }
    if actual != expected {
        return Err(Error::SourceRosterMismatch(location));
    }
    Ok(())
}
