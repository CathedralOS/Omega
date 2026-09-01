//! Optimizer module role: executable entrance. Synthetic reload-value namespace binding.
//!
//! This compiler-private boundary gives each validated reload a deterministic
//! synthetic identity. It does not create a selected virtual register,
//! instruction, memory effect, frame address, trap claim, or publication path.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::synthetic_reload_value_plan_identity;
pub use model::*;
pub use validate::validate_synthetic_reload_values;

use crate::{ValidatedAbstractSpillInsertion, ValidatedReloadValueHomes};

pub fn bind_synthetic_reload_values(
    insertion: &ValidatedAbstractSpillInsertion,
    homes: &ValidatedReloadValueHomes,
    policy: SyntheticReloadValuePolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSyntheticReloadValues, SyntheticReloadValueError> {
    let plan = compute::compute(insertion, homes, policy, budget)?;
    validate_synthetic_reload_values(insertion, homes, plan)
}
