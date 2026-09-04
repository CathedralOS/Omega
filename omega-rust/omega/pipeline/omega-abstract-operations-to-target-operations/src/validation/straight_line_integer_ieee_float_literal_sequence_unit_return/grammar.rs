//! Closed source grammar and ordered typed-literal reconstruction.

use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use psi_core::{EdgeId, MachineId, ScalarType};

use super::super::{
    IntegerIeeeFloatLiteralSequenceMember,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
};

use StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError as Error;

pub(super) struct SourceSequence {
    machine: MachineId,
    literals: Vec<IntegerIeeeFloatLiteralSequenceMember>,
    return_edge: EdgeId,
}

impl SourceSequence {
    pub(super) fn literals(&self) -> &[IntegerIeeeFloatLiteralSequenceMember] {
        &self.literals
    }

    pub(super) const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }

    pub(super) fn into_receipt(
        self,
    ) -> StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt {
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt::new(
            self.machine,
            self.literals,
            self.return_edge,
        )
    }
}

pub(super) fn is_candidate(function: &AbstractFunction) -> bool {
    let Some((last, literals)) = function.operations.split_last() else {
        return false;
    };
    let mut has_integer = false;
    let mut has_ieee_float = false;
    for literal in literals {
        match literal {
            AbstractOperation::IntegerConstant { .. } => has_integer = true,
            AbstractOperation::IeeeFloatConstant { .. } => has_ieee_float = true,
            _ => return false,
        }
    }
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
        && has_integer
        && has_ieee_float
        && matches!(
            last,
            AbstractOperation::ReturnUnit { cleanup_actions, .. }
                if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(source: &AbstractFunction) -> Result<SourceSequence, Error> {
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
    let Some((source_return, source_literals)) = source.operations.split_last() else {
        return Err(Error::SourceOperationRoster);
    };
    let mut has_integer = false;
    let mut has_ieee_float = false;
    let mut literals = Vec::with_capacity(source_literals.len());
    for literal in source_literals {
        match literal {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } => {
                has_integer = true;
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return Err(Error::SourceConstantType);
                };
                if !integer_type.admits(*value) {
                    return Err(Error::SourceConstantOutsideType);
                }
                literals.push(IntegerIeeeFloatLiteralSequenceMember::integer(
                    *psi_operation,
                    *result,
                    *integer_type,
                    *value,
                ));
            }
            AbstractOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            } => {
                has_ieee_float = true;
                literals.push(IntegerIeeeFloatLiteralSequenceMember::ieee_float(
                    *psi_operation,
                    *result,
                    *value,
                ));
            }
            _ => return Err(Error::SourceOperationRoster),
        }
    }
    if !has_integer || !has_ieee_float {
        return Err(Error::SourceOperationRoster);
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
    Ok(SourceSequence {
        machine: source.machine,
        literals,
        return_edge: *psi_edge,
    })
}
