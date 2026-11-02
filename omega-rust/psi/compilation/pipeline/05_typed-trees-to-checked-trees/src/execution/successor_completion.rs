//! Retain the source invocation boundary when scalar or composed control fuses
//! it into an edge. Unknown completion stays potentially returning; only the
//! resolved callable's checked topology can suppress normal caller cleanup.
use checked_trees::CheckedSuccessorCompletion;

pub(super) fn classify(
    program: &dyn validation::CheckingInput,
    source: &typed_trees::machine::Machine,
    target: symbols::SymbolHandle,
) -> CheckedSuccessorCompletion {
    let Some((callable, _)) = validation::terminal_callable_target(program, source, target) else {
        return CheckedSuccessorCompletion::StateArrival;
    };
    if matches!(
        validation::boundary_requirement_completion(program, callable),
        semantic_vocabulary::BoundaryCompletion::External(_)
    ) || validation::call_target_state(program, callable)
        .is_some_and(|state| validation::normal_completion_possible(program, state) == Some(false))
    {
        CheckedSuccessorCompletion::NoNormalInvocation
    } else {
        CheckedSuccessorCompletion::ReturningInvocation
    }
}
