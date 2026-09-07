//! Optimizer module role: stage group. Settlement tests grouped by exact evidence and retained import shape.

pub(super) use super::exact_plan::selected_plan_from_exact_evidence;
pub(super) use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};

mod exact_evidence;
mod fixtures;
mod syscall_identity;

use fixtures::*;
