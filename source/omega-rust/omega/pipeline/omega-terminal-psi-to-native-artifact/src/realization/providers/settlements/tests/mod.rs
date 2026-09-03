//! Optimizer module role: stage group. Settlement tests grouped by exact evidence and retained import shape.

pub(super) use super::{
    exact_plan::selected_plan_from_exact_evidence,
    normalized_foreign_call::rejoin_normalized_foreign_call,
};
pub(super) use omega_effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};

mod exact_evidence;
mod fixtures;
mod literal_arguments;
mod runtime_home_arguments;
mod runtime_scalar_home_plan;
mod scalar_result;
mod syscall_identity;
mod zero_argument_import;

use fixtures::*;
use runtime_scalar_home_plan::runtime_argument_abstract_plan;
