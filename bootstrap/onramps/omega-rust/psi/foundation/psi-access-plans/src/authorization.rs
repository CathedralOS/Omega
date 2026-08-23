use psi_language_core::atomic::AtomicOrderingPlan;

use super::{
    AccessOperation, AccessPlanDiagnostic, AtomicAccessOperation, BorrowPolarity, EffectFootprint,
    FieldAccessDescriptor, ObservationModel,
};

pub const fn effect_footprints_conflict(
    left: EffectFootprint,
    left_operation: AccessOperation,
    right: EffectFootprint,
    right_operation: AccessOperation,
) -> bool {
    if !left.overlaps(right) {
        return false;
    }
    match (left_operation, right_operation) {
        (AccessOperation::Read, AccessOperation::Read) => false,
        (AccessOperation::Atomic(_), AccessOperation::Atomic(_)) => {
            left.address != right.address || left.length_bytes != right.length_bytes
        }
        _ => true,
    }
}

pub(super) fn authorize_descriptor(
    descriptor: &FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
) -> Result<(), AccessPlanDiagnostic> {
    validate_operation_ordering(operation)?;
    let permitted = match operation {
        AccessOperation::Read => descriptor.permissions.read,
        AccessOperation::Take => {
            descriptor.permissions.take
                && current_borrow == BorrowPolarity::Exclusive
                && source_loan == BorrowPolarity::Exclusive
        }
        AccessOperation::Write => {
            descriptor.permissions.write
                && current_borrow == BorrowPolarity::Exclusive
                && source_loan == BorrowPolarity::Exclusive
        }
        AccessOperation::CompoundMutation => {
            descriptor.observation == ObservationModel::Stable
                && descriptor.permissions.read
                && descriptor.permissions.write
                && current_borrow == BorrowPolarity::Exclusive
                && source_loan == BorrowPolarity::Exclusive
        }
        AccessOperation::Atomic(AtomicAccessOperation::Load(_)) => {
            descriptor.permissions.atomic.load
        }
        AccessOperation::Atomic(AtomicAccessOperation::Store(_)) => {
            descriptor.permissions.atomic.store
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(_)) => {
            descriptor.permissions.atomic.fetch_add
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchSub(_)) => {
            descriptor.permissions.atomic.fetch_sub
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchXor(_)) => {
            descriptor.permissions.atomic.fetch_xor
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchOr(_)) => {
            descriptor.permissions.atomic.fetch_or
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(_)) => {
            descriptor.permissions.atomic.fetch_and
        }
        AccessOperation::Atomic(AtomicAccessOperation::Swap(_)) => {
            descriptor.permissions.atomic.swap
        }
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchange { .. }) => {
            descriptor.permissions.atomic.compare_exchange
        }
    };
    if permitted {
        Ok(())
    } else {
        Err(AccessPlanDiagnostic(format!(
            "field `{}` does not permit {operation:?} through a {current_borrow:?} current borrow over a {source_loan:?} source loan",
            descriptor.field,
        )))
    }
}

pub(super) fn validate_operation_ordering(
    operation: AccessOperation,
) -> Result<(), AccessPlanDiagnostic> {
    let AccessOperation::Atomic(operation) = operation else {
        return Ok(());
    };
    let ordering = operation.ordering_plan();
    let legal = match ordering {
        AtomicOrderingPlan::Load(ordering) => ordering.valid_for_load(),
        AtomicOrderingPlan::Store(ordering) => ordering.valid_for_store(),
        AtomicOrderingPlan::ReadModifyWrite(_) | AtomicOrderingPlan::Swap(_) => true,
        AtomicOrderingPlan::CompareExchange { success, failure } => {
            failure.valid_compare_exchange_failure(success)
        }
    };
    if legal {
        Ok(())
    } else {
        Err(AccessPlanDiagnostic(format!(
            "atomic access carries an invalid ordering plan: {ordering:?}"
        )))
    }
}
