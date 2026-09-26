//! Optimizer module role: executable entrance. Instruction selection: propose one bounded selected CFG, then validate it independently.
//!
//! Validate the immutable target/register/catalog join once at this entrance.
//! Construction and independent replay borrow it across functions; rebuilding
//! the same catalog for every function or replay mutation adds no evidence.
//! The public raw validation entrance still validates its own supplied join.

mod address_join_input;
mod aggregate_result_input;
mod block_order;
mod byte_view_homes;
mod constraints;
mod construction;
mod edge_transfers;
mod element_view_homes;
mod established_view_input;
mod identity;
mod integer_conversion;
mod literal_compare_input;
mod literal_storage_input;
mod model;
mod parameter_use;
pub(crate) mod primitive_local_input;
mod read_result_input;
mod record_input;
mod scalar_array_input;
mod scalar_call_abi;
mod silent_boundary_input;
mod validation;
pub(crate) mod value_transport;

pub use crate::selected_instructions::selected_instruction_plan_identity;
pub use model::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
};
pub use validation::validate_selected_instructions;

use crate::register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use crate::selected_instructions::SelectedSelectionConstraints;

use crate::legalization::ValidatedLegalizedOperations;
use construction::build_plan;

pub fn select_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let environment = target_register_environment(legalized.plan().target, physical, catalog)
        .map_err(|_| SelectedInstructionError::custody())?;
    let plan = build_plan(legalized, constraints, &environment)?;
    validation::validate_with_environment(legalized, constraints, &environment, plan)
}

/// Environments already joined in this process, keyed by the target and the
/// content identities of the validated physical model and constraint catalog.
type EnvironmentKey = (
    target::NativeTarget,
    crate::register_model::PhysicalRegisterModelIdentity,
    crate::register_model::RegisterConstraintCatalogIdentity,
);

/// The joined register environment is a pure function of the target and the
/// validated physical model and constraint catalog, which their content
/// identities name. Selection, scalar-graph construction and selection
/// validation each cloned both and re-validated the join for every function;
/// each distinct environment is now joined once per process. A failed join is
/// not remembered, so it fails again on every request.
pub(crate) fn target_register_environment(
    target: target::NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<
    crate::register_environment::ValidatedTargetRegisterEnvironment,
    crate::register_environment::TargetRegisterEnvironmentValidationError,
> {
    static ENVIRONMENTS: std::sync::OnceLock<
        std::sync::Mutex<
            Vec<(
                EnvironmentKey,
                crate::register_environment::ValidatedTargetRegisterEnvironment,
            )>,
        >,
    > = std::sync::OnceLock::new();
    let environments = ENVIRONMENTS.get_or_init(std::sync::Mutex::default);
    let key = (target, physical.identity(), catalog.identity());
    if let Ok(environments) = environments.lock()
        && let Some((_, environment)) = environments.iter().find(|(candidate, _)| *candidate == key)
    {
        return Ok(environment.clone());
    }
    let environment = crate::register_environment::validate_target_register_environment(
        target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )?;
    if let Ok(mut environments) = environments.lock()
        && !environments.iter().any(|(candidate, _)| *candidate == key)
    {
        environments.push((key, environment.clone()));
    }
    Ok(environment)
}
