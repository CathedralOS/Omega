//! Admit control cycles only after exact independent verified-source replay.
use super::{AbstractOperationPlan, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
