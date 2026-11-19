pub use build_scalar_evaluation::{AdmittedImportRealizations, TargetImportRealizations};
use semantic_vocabulary::ScalarApplicationId;
use terminal_installation::{
    ScalarApplicationClosure, ScalarApplicationObservations, ScalarConditionSelection,
};
use terminal_work::WorkMeter;

pub fn evaluate_scalar_applications<'artifact, 'scope>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: impl Into<Option<&'scope str>>,
    requested: &[ScalarApplicationId],
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    build_scalar_evaluation::evaluate_scalar_applications(
        observations,
        value_scope,
        requested,
        None,
        meter,
    )
}

pub fn evaluate_scalar_applications_with_address_model<'artifact>(
    observations: &mut ScalarApplicationObservations<'artifact>,
    value_scope: &str,
    requested: &[ScalarApplicationId],
    address_model: Option<&terminal_installation::ScalarAddressModel>,
    meter: &mut WorkMeter,
) -> Result<ScalarConditionSelection<'artifact>, String> {
    build_scalar_evaluation::evaluate_scalar_applications(
        observations,
        value_scope,
        requested,
        address_model,
        meter,
    )
}
