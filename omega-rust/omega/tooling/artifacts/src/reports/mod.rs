//! The report owners: each keeps its records beside its validation and
//! rendering operations, sharing the canonical JSON projection for calling plans.

pub(crate) mod calling_plan_json;
#[cfg(any(test, feature = "external-root-report"))]
pub(crate) mod external_root_report;
pub(crate) mod timing_report;
pub(crate) mod trust_report;
pub(crate) mod wire_report;
