//! Evaluating source-authored layouts: layout plans, plan-laid value types,
//! wire plans, access views and placed-access policies. Schema reflection is
//! shared by those operations.

pub(crate) mod access_lifecycles;
pub(crate) mod access_plans;
pub(crate) mod access_views;
pub(crate) mod layout_plans;
pub(crate) mod plan_laid;
mod policy_arguments;
pub(crate) mod schema_reflection;
pub(crate) mod wire_plans;
