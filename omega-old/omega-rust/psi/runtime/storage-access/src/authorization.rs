//! Authorize an operation against the admitted geometry and both live borrows.
use access_admission::{AccessFieldKey, ValidatedAccessFields};
use language_core::atomic::AtomicOrderingPlan;

use access_admission::FieldAccessDescriptor;
use access_vocabulary::{
    AccessOperation, AccessPlanDiagnostic, AtomicAccessOperation, BorrowPolarity, ObservationModel,
};

pub(crate) fn authorize_descriptor(
    descriptor: &FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
) -> Result<(), AccessPlanDiagnostic> {
    validate_operation_ordering(operation)?;
    let borrow_permits = match operation.receiver_polarity() {
        BorrowPolarity::Shared => true,
        BorrowPolarity::Exclusive => {
            current_borrow == BorrowPolarity::Exclusive && source_loan == BorrowPolarity::Exclusive
        }
    };
    let permission_admits = match operation {
        AccessOperation::Read => descriptor.permissions().read,
        AccessOperation::Take => descriptor.permissions().take,
        AccessOperation::Write => descriptor.permissions().write,
        AccessOperation::CompoundMutation => {
            descriptor.observation() == ObservationModel::Stable
                && descriptor.permissions().read
                && descriptor.permissions().write
        }
        AccessOperation::Atomic(AtomicAccessOperation::Load(_)) => {
            descriptor.permissions().atomic.load
        }
        AccessOperation::Atomic(AtomicAccessOperation::Store(_)) => {
            descriptor.permissions().atomic.store
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(_)) => {
            descriptor.permissions().atomic.fetch_add
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchSub(_)) => {
            descriptor.permissions().atomic.fetch_sub
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchXor(_)) => {
            descriptor.permissions().atomic.fetch_xor
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchOr(_)) => {
            descriptor.permissions().atomic.fetch_or
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(_)) => {
            descriptor.permissions().atomic.fetch_and
        }
        AccessOperation::Atomic(AtomicAccessOperation::Swap(_)) => {
            descriptor.permissions().atomic.swap
        }
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchange { .. }) => {
            descriptor.permissions().atomic.compare_exchange
        }
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchangeOnce { .. }) => {
            descriptor.permissions().atomic.compare_exchange_once
        }
        AccessOperation::Atomic(AtomicAccessOperation::TryExchangeOnce { .. }) => {
            descriptor.permissions().atomic.try_exchange_once
        }
    };
    if borrow_permits && permission_admits {
        Ok(())
    } else {
        Err(AccessPlanDiagnostic(format!(
            "field `{}` does not permit {operation:?} through a {current_borrow:?} current borrow over a {source_loan:?} source loan",
            descriptor.field(),
        )))
    }
}

pub(crate) fn validate_operation_ordering(
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
        AtomicOrderingPlan::CompareExchange { success, failure }
        | AtomicOrderingPlan::CompareExchangeOnce { success, failure } => {
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedFieldAccess {
    descriptor: FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
}

impl AuthorizedFieldAccess {
    pub const fn descriptor(&self) -> &FieldAccessDescriptor {
        &self.descriptor
    }

    pub const fn current_borrow(&self) -> BorrowPolarity {
        self.current_borrow
    }

    pub const fn source_loan(&self) -> BorrowPolarity {
        self.source_loan
    }

    pub const fn operation(&self) -> AccessOperation {
        self.operation
    }
}

pub trait AccessAuthorization {
    fn authorize(
        &self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic>;
    fn authorize_region_projection(
        &self,
        key: AccessFieldKey,
        byte_offset: u64,
        transfer_width_bits: u16,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic>;
}

impl AccessAuthorization for ValidatedAccessFields {
    fn authorize(
        &self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic> {
        let entry = self.field(key).ok_or_else(|| {
            AccessPlanDiagnostic("field key does not belong to the validated access plan".into())
        })?;
        let descriptor = self.field_descriptor(key).ok_or_else(|| {
            AccessPlanDiagnostic(if self.field_region(key).is_some() {
                format!(
                    "field `{}` requires a checked structural projection before primitive access",
                    entry.field()
                )
            } else {
                format!("field `{}` is inaccessible", entry.field())
            })
        })?;
        authorize_descriptor(descriptor, current_borrow, source_loan, operation)?;
        Ok(AuthorizedFieldAccess {
            descriptor: descriptor.clone(),
            current_borrow,
            source_loan,
            operation,
        })
    }

    fn authorize_region_projection(
        &self,
        key: AccessFieldKey,
        byte_offset: u64,
        transfer_width_bits: u16,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic> {
        let descriptor = self.validate_region_projection(key, byte_offset, transfer_width_bits)?;
        authorize_descriptor(&descriptor, current_borrow, source_loan, operation)?;
        Ok(AuthorizedFieldAccess {
            descriptor,
            current_borrow,
            source_loan,
            operation,
        })
    }
}
