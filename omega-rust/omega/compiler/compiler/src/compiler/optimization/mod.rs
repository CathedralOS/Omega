//! Optimization selection controls; native production is owned by native-realization.

#[cfg(any(test, feature = "experimental-external-optimization-policy"))]
pub(crate) mod external_policy;
