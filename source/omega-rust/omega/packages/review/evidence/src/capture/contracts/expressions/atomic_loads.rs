use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::{PackageReviewAtomicLoadOrdering, PackageReviewContractExpression};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::expression::{ExpressionHandle, TableAtomicExpression};

pub(crate) fn project_contract_atomic_load(
    context: &ContractProjectionContext<'_>,
    atomic: &TableAtomicExpression,
    project_child: impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    if !atomic.value.is_valid() || atomic.result.is_valid() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contains an inconsistent atomic-load expression",
            context.subject_kind, context.subject_name
        ))]);
    }
    let ordering = match atomic.ordering {
        psi_language_core::atomic::AtomicOrderingPlan::Load(
            psi_language_core::atomic::MemoryOrdering::NoOrdering,
        ) => PackageReviewAtomicLoadOrdering::NoOrdering,
        psi_language_core::atomic::AtomicOrderingPlan::Load(
            psi_language_core::atomic::MemoryOrdering::Receive,
        ) => PackageReviewAtomicLoadOrdering::Receive,
        psi_language_core::atomic::AtomicOrderingPlan::Load(
            psi_language_core::atomic::MemoryOrdering::GlobalOrder,
        ) => PackageReviewAtomicLoadOrdering::GlobalOrder,
        psi_language_core::atomic::AtomicOrderingPlan::Load(_) => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contains an atomic load with an invalid ordering",
                context.subject_kind, context.subject_name
            ))]);
        }
        _ => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contains a mutation-bearing atomic contract expression",
                context.subject_kind, context.subject_name
            ))]);
        }
    };
    Ok(PackageReviewContractExpression::AtomicLoad {
        value: Box::new(project_child(atomic.value)?),
        ordering,
    })
}
