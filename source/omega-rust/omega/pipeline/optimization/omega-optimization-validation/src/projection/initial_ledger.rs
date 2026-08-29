//! Initial-unit reconstruction and transformation-ledger custody.

use super::custody::validate_source_custody;
use super::*;

pub(super) fn validate_initial_and_ledger(
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    ledger: &PsiTransformationLedger,
) -> Result<OptimizationUnitIdentity, OptimizedAbstractPlanProjectionError> {
    validate_transformed_psi_optimization_unit(input, final_unit)
        .map_err(OptimizedAbstractPlanProjectionError::FinalUnit)?;

    let initial = omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input.clone(),
        final_unit.fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractPlanProjectionError::InitialUnitProjection)?;
    validate_verified_psi_optimization_unit(&initial)
        .map_err(|_| OptimizedAbstractPlanProjectionError::InitialUnitProjection)?;
    let initial_identity = initial.unit().identity;

    let replayed_ledger = PsiTransformationLedger::new(
        ledger.psi(),
        ledger.fuel_schedule(),
        ledger.input(),
        ledger.output(),
        ledger.records().to_vec(),
    )
    .map_err(OptimizedAbstractPlanProjectionError::LedgerReplay)?;
    if &replayed_ledger != ledger {
        return Err(OptimizedAbstractPlanProjectionError::LedgerIdentityMismatch);
    }
    if ledger.psi() != input.plan().psi {
        return Err(OptimizedAbstractPlanProjectionError::LedgerTerminalMismatch);
    }
    if ledger.fuel_schedule() != final_unit.fuel_schedule {
        return Err(OptimizedAbstractPlanProjectionError::LedgerFuelMismatch);
    }
    if ledger.input() != initial_identity {
        return Err(OptimizedAbstractPlanProjectionError::LedgerInitialMismatch);
    }
    if ledger.output() != final_unit.identity {
        return Err(OptimizedAbstractPlanProjectionError::LedgerFinalMismatch);
    }
    validate_source_custody(initial.unit(), final_unit, ledger)?;
    Ok(initial_identity)
}
