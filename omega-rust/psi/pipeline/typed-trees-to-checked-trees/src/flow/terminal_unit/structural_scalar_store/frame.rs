//! Replay the complete state frame without confusing it with the local body.

use super::*;

pub(super) fn matches(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    frame: &facts::NormalizedWriteFrame,
) -> bool {
    let Some(resolver) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    // A state's summary includes every reachable successor. Comparing it to
    // only this block's stores rejects legitimate effects in later states.
    // The statement planner separately retains every local store and call;
    // replay the complete summary through its existing semantic owner here.
    frame.complete_paths().is_some()
        && resolver.inferred_state_write_frame(machine, state) == *frame
}
