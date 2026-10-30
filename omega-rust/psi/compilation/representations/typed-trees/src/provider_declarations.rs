//! Authored provider coordinates, before checking or target selection.

/// Retain every authored target origin in one exact dependency-purpose scope.
/// Target choice remains outside this source query, including for early Build.
pub fn provider_target_declarations(
    typed: &crate::TypedTrees,
    scope: source::DependencyScope,
) -> Vec<language_semantics::provider_declarations::SelectedTargetMachineOrigin> {
    typed
        .machines()
        .iter()
        .filter(|machine| {
            typed
                .symbols
                .symbol_is_in_dependency_scope(machine.symbol, scope)
                && (scope == source::DependencyScope::Product || machine.target.is_none())
        })
        .filter_map(|machine| {
            machine.authored_target.as_ref().map(|target| {
                language_semantics::provider_declarations::SelectedTargetMachineOrigin {
                    machine: machine.symbol,
                    target: target.as_str().to_owned(),
                }
            })
        })
        .collect()
}
