//! Optimizer module role: executable entrance. Independent liveness replay, exact comparison, and receipt admission.
mod receipt;
mod replay;

pub(crate) use receipt::admit_validated_liveness;
