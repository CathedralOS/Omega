use super::super::{
    FunctionFragmentEmissionSourceKind, StagedOptimizedFunctionFragmentEmissionSource,
};

pub(super) fn of(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> FunctionFragmentEmissionSourceKind {
    source.source_kind()
}
