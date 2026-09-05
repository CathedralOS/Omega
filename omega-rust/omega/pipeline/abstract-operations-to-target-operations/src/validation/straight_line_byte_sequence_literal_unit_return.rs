//! Independent replay of one borrowed byte-sequence literal and its Unit return.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use semantic_vocabulary::StructuralPlaceKind;
use target::NativeTarget;
use target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

use super::{
    StraightLineByteSequenceLiteralUnitReturnTranslationError,
    StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
};

pub(in crate::validation) fn is_candidate(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && function.result == AbstractFunctionResult::Unit
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::EstablishByteSequenceLiteral {
                    place,
                    structural_type,
                    ..
                },
                AbstractOperation::ReturnUnit { cleanup_actions, .. },
            ] if matches!(
                    &place.kind,
                    StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: 0,
                        structural_type: place_type,
                    } if *place_type == structural_type.id
                )
                && matches!(
                    &structural_type.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                )
                && cleanup_actions.is_empty()
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
    StraightLineByteSequenceLiteralUnitReturnTranslationError,
> {
    use StraightLineByteSequenceLiteralUnitReturnTranslationError as Error;

    if !source.parameters.is_empty() {
        return Err(Error::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(Error::SourceStructuralParameters);
    }
    if source.result != AbstractFunctionResult::Unit {
        return Err(Error::SourceResult);
    }
    if !source.entry_claims.is_empty() {
        return Err(Error::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(Error::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(Error::SourceBlockRoster);
    }
    let [
        AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes,
        },
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(Error::SourceOperationRoster);
    };
    if !matches!(
        &place.kind,
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: place_type,
        } if *place_type == structural_type.id
    ) {
        return Err(Error::SourcePlace);
    }
    if !matches!(
        &structural_type.shape,
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
    ) {
        return Err(Error::SourceStructuralType);
    }
    if !cleanup_actions.is_empty() {
        return Err(Error::SourceCleanupActions);
    }
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(Error::TargetFixedIntegerScalarAbi);
    }
    if target.provenance.operations.as_slice() != [*psi_operation]
        || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(Error::TargetProvenance);
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::TargetOperation);
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &CallSignature::default(),
    )
    .map_err(|_| Error::TargetCallPlan)?;
    if body.call_plan != expected_call_plan {
        return Err(Error::TargetCallPlan);
    }
    if !body.parameters.is_empty() {
        return Err(Error::TargetParameters);
    }
    let [
        TargetUnitOperation::EstablishByteSequenceLiteral {
            psi_operation: target_operation,
            place: target_place,
            structural_type: target_structural_type,
            bytes: target_bytes,
        },
        TargetUnitOperation::Return {
            psi_edge: target_edge,
            cleanup_actions: target_cleanup_actions,
        },
    ] = body.operations.as_slice()
    else {
        return Err(Error::TargetOperationRoster);
    };
    if target_operation != psi_operation
        || target_place != place
        || target_structural_type != structural_type
        || target_bytes != bytes
    {
        return Err(Error::TargetEstablishment);
    }
    if target_edge != psi_edge || target_cleanup_actions != cleanup_actions {
        return Err(Error::TargetReturn);
    }
    Ok(
        StraightLineByteSequenceLiteralUnitReturnTranslationReceipt::new(
            source.machine,
            *psi_operation,
            *place,
            structural_type.clone(),
            bytes.clone(),
            *psi_edge,
        ),
    )
}
