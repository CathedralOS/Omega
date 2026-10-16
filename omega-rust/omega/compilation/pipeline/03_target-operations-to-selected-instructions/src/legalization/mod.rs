//! Optimizer module role: stage group. Mandatory target legalization: construct the canonical plan.
//!
//! `source` projects ordinary instruction graphs. The legalized plan is
//! consumed in the same compile, so no verifier replays it.

mod model;
mod runtime_indices;
mod scalar_graph_input;
mod source;
mod source_input;
pub(crate) use source_input::LegalizationSource;

pub use model::LegalizationError;
pub(crate) use model::legalization_validator_identity;
pub(crate) use model::{LegalizationValidationReceipt, ValidatedLegalizedOperations};

use abstract_operations::AbstractOperationPlan;
use legalized_operations::{LegalizedOperationPlan, legalized_operation_plan_identity};
use target_operations::TargetOperationPlan;

use source::derive_source_function_rosters;

pub(crate) fn legalize_target_operations<'source>(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    source: impl Into<LegalizationSource<'source>>,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    let source = source.into();
    let unit = source.unit;
    scalar_graph_input::call_origin::validate_boundary_origins(
        target,
        unit,
        source.verified_input,
    )?;
    let rosters = derive_source_function_rosters(target, abstract_plan, unit)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
    };
    Ok(seal(target, unit, plan))
}

fn seal(
    target: &TargetOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    plan: LegalizedOperationPlan,
) -> ValidatedLegalizedOperations {
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.scalar_functions.len(),
    };
    ValidatedLegalizedOperations { plan, receipt }
}
pub(crate) use scalar_graph_input::exact_cast_has_native_carriers;

/// Independently check retained target rows against their source products.
/// This is an audit entrypoint for corpus and foreign-artifact review, not
/// another phase of instruction construction in the producing compile.
/// Callers supply validated current source products, including exact receiving
/// obligations and current-graph byte invariants. This checks target/source
/// correspondence; it does not admit an arbitrary optimization unit.
pub fn verify_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    scalar_graph_input::call_origin::validate_boundary_origins(target, unit, None)?;
    audit_target_operation_plan(target, abstract_plan, unit)
}

/// Audit selected family calls with their immutable portable catalog and
/// admitted deployment-tag selection, rather than reconstructing installation.
/// Source admission remains the caller's responsibility, as for
/// [`verify_target_operation_plan`].
pub fn verify_target_operation_plan_with_context(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
) -> Result<(), LegalizationError> {
    scalar_graph_input::call_origin::validate_boundary_origins(target, unit, Some(input))?;
    audit_target_operation_plan(target, abstract_plan, unit)
}

fn audit_target_operation_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    source::validate_source_custody(target, abstract_plan, unit)?;
    scalar_graph_input::normalized_foreign::validate_native_callback_roster(target)?;
    for function in &target.functions {
        let mut abstracted = abstract_plan
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let mut optimized = unit
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let (Some(abstracted_function), Some(optimized_function), None, None) = (
            abstracted.next(),
            optimized.next(),
            abstracted.next(),
            optimized.next(),
        ) else {
            return Err(LegalizationError::custody());
        };
        scalar_graph_input::match_input(
            function,
            abstracted_function,
            optimized_function,
            target,
            abstract_plan,
            unit,
        )
        .map_err(|error| error.attributed(function.machine, None))?;
    }
    Ok(())
}
