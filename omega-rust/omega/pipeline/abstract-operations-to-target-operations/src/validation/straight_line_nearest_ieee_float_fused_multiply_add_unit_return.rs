//! Independent replay of three raw IEEE constants, one settled nearest-even FMA, and Unit return.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use effects::provider_plan::ProviderBinding;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, OperationId, ValueId};
use target::{Architecture, NativeTarget, X86ScalarFmaSlot};
use target_operations::{
    TargetFunction, TargetIeeeFloatFmaOperand, TargetOperation, TargetUnitOperation,
};

use super::{
    IeeeFloatFusedMultiplyAddOperandReceipt, IeeeFloatLiteralSequenceMember,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
};

type SourceLiteral = (OperationId, ValueId, IeeeFloatValue);

pub(in crate::validation) fn is_candidate(function: &AbstractFunction) -> bool {
    let [
        AbstractOperation::IeeeFloatConstant {
            result: left_result,
            value: left_value,
            ..
        },
        AbstractOperation::IeeeFloatConstant {
            result: right_result,
            value: right_value,
            ..
        },
        AbstractOperation::IeeeFloatConstant {
            result: addend_result,
            value: addend_value,
            ..
        },
        AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
            format,
            left,
            right,
            addend,
            ..
        },
        AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        },
    ] = function.operations.as_slice()
    else {
        return false;
    };
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
        && left == left_result
        && right == right_result
        && addend == addend_result
        && left_value.format() == *format
        && right_value.format() == *format
        && addend_value.format() == *format
        && cleanup_actions.is_empty()
}

#[allow(clippy::too_many_lines)]
pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    settlements: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
> {
    use StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError as Error;

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
        left_literal,
        right_literal,
        addend_literal,
        source_fma,
        source_return,
    ] = source.operations.as_slice()
    else {
        return Err(Error::SourceOperationRoster);
    };
    let literals: [SourceLiteral; 3] = [
        source_literal(left_literal).ok_or(Error::SourceOperationRoster)?,
        source_literal(right_literal).ok_or(Error::SourceOperationRoster)?,
        source_literal(addend_literal).ok_or(Error::SourceOperationRoster)?,
    ];
    let AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: fma_operation,
        result: fma_result,
        format,
        left,
        right,
        addend,
    } = source_fma
    else {
        return Err(Error::SourceOperationRoster);
    };
    if [*left, *right, *addend] != [literals[0].1, literals[1].1, literals[2].1]
        || literals.iter().any(|literal| literal.2.format() != *format)
    {
        return Err(Error::SourceOperand);
    }
    let AbstractOperation::ReturnUnit {
        psi_edge,
        cleanup_actions,
    } = source_return
    else {
        return Err(Error::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(Error::SourceCleanupActions);
    }
    if expected_target.architecture != Architecture::X86_64 {
        return Err(Error::TargetArchitecture);
    }
    let mut matching_settlements = settlements
        .iter()
        .filter(|settlement| settlement.terminal_operation == *fma_operation);
    let Some(settlement) = matching_settlements.next() else {
        return Err(Error::SettlementRoster);
    };
    if matching_settlements.next().is_some() {
        return Err(Error::SettlementRoster);
    }
    if settlement.format != *format {
        return Err(Error::SettlementFormat);
    }
    let expected_slot = slot_for_format(*format);
    if settlement.slot != expected_slot {
        return Err(Error::SettlementSlot);
    }
    let provider = settlement.provider;
    if !provider.has_canonical_identity()
        || provider.profile().native_target() != expected_target
        || !provider.admits(provider.requirement(), expected_slot)
    {
        return Err(Error::SettlementProvider);
    }
    let provider_plan = settlement.provider_plan;
    if provider_plan.target != provider.profile().target_name() {
        return Err(Error::SettlementPlanTarget);
    }
    if !matches!(provider_plan.rows.as_slice(), [row]
        if row.requirement_identity == expected_slot.selected_plan_requirement_identity()
            && matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }))
    {
        return Err(Error::SettlementPlanRow);
    }
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(Error::TargetFixedIntegerScalarAbi);
    }
    let expected_operations = [literals[0].0, literals[1].0, literals[2].0, *fma_operation];
    if target.provenance.operations.as_slice() != expected_operations
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
        target_left,
        target_right,
        target_addend,
        target_fma,
        target_return,
    ] = body.operations.as_slice()
    else {
        return Err(Error::TargetOperationRoster);
    };
    for (source_literal, target_literal) in
        literals
            .iter()
            .zip([target_left, target_right, target_addend])
    {
        if !matches!(target_literal,
            TargetUnitOperation::IeeeFloatConstant { psi_operation, result, value }
                if (*psi_operation, *result, *value) == *source_literal)
        {
            return Err(Error::TargetConstant);
        }
    }
    let TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: target_fma_operation,
        result: target_fma_result,
        format: target_format,
        left: target_left,
        right: target_right,
        addend: target_addend,
        settlement: target_settlement,
    } = target_fma
    else {
        return Err(Error::TargetOperationRoster);
    };
    let expected_operands = literals.map(target_operand);
    if *target_fma_operation != *fma_operation
        || *target_fma_result != *fma_result
        || *target_format != *format
        || *target_left != expected_operands[0]
        || *target_right != expected_operands[1]
        || *target_addend != expected_operands[2]
    {
        return Err(Error::TargetFusedMultiplyAdd);
    }
    let expected_report_identity = provider_plan.report_fingerprint();
    let expected_digest = *provider_plan.identity_digest().as_bytes();
    if target_settlement.terminal_operation != *fma_operation
        || target_settlement.provider_plan_report_identity != expected_report_identity
        || target_settlement.provider_plan_digest != expected_digest
        || target_settlement.format != *format
        || target_settlement.slot != expected_slot
        || target_settlement.provider != provider
    {
        return Err(Error::TargetSettlement);
    }
    let TargetUnitOperation::Return {
        psi_edge: target_edge,
        cleanup_actions: target_cleanup,
    } = target_return
    else {
        return Err(Error::TargetOperationRoster);
    };
    if target_edge != psi_edge || target_cleanup != cleanup_actions {
        return Err(Error::TargetReturn);
    }

    let literal_receipts = literals
        .map(|literal| IeeeFloatLiteralSequenceMember::new(literal.0, literal.1, literal.2));
    let operand_receipts = literals.map(|literal| {
        IeeeFloatFusedMultiplyAddOperandReceipt::new(literal.0, literal.1, literal.2)
    });
    Ok(
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt::new(
            source.machine,
            literal_receipts,
            *fma_operation,
            *fma_result,
            *format,
            operand_receipts,
            expected_report_identity,
            expected_digest,
            expected_slot,
            provider,
            *psi_edge,
        ),
    )
}

fn source_literal(operation: &AbstractOperation) -> Option<SourceLiteral> {
    let AbstractOperation::IeeeFloatConstant {
        psi_operation,
        result,
        value,
    } = operation
    else {
        return None;
    };
    Some((*psi_operation, *result, *value))
}

const fn slot_for_format(format: IeeeFloatFormat) -> X86ScalarFmaSlot {
    match format {
        IeeeFloatFormat::Binary32 => X86ScalarFmaSlot::Binary32,
        IeeeFloatFormat::Binary64 => X86ScalarFmaSlot::Binary64,
    }
}

const fn target_operand(literal: SourceLiteral) -> TargetIeeeFloatFmaOperand {
    TargetIeeeFloatFmaOperand {
        defining_operation: literal.0,
        source_value: literal.1,
        value: literal.2,
    }
}
