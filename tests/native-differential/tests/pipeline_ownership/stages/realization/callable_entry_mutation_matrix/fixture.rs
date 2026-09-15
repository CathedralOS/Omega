//! Public validated ordinary-callable fixture shared by mutation families.

use super::super::callable_entry::staged_callable_object_artifact;
use crate::tests::{
    NativeTarget, StagedValidatedOptimizedOrdinaryCallableEntry,
    stage_validated_optimized_ordinary_callable_entry,
};

pub(super) fn staged_callable() -> StagedValidatedOptimizedOrdinaryCallableEntry {
    stage_validated_optimized_ordinary_callable_entry(staged_callable_object_artifact(
        NativeTarget::linux_x64(),
    ))
    .unwrap()
}
