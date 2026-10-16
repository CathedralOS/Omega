//! Optimizer module role: executable entrance. Independent liveness replay, exact comparison, and receipt admission.
mod receipt;
pub(crate) use receipt::admit_validated_liveness;

mod counts;
pub(crate) use counts::FunctionCounts;
