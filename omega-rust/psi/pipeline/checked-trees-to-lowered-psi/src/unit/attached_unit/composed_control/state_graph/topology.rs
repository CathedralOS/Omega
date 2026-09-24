//! Source-state reachability is independent of descriptor bindings. The live
//! mask marks every state the entry state's successor closure reaches; states
//! outside it are authored but unexecuted, and pruning them keeps their
//! parameter and claim rows from appearing as definitions without sources.

use super::super::LoweringError;
use super::{CheckedComposedUnitControlMachinePlan, successors};

pub(crate) fn live(
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<Vec<bool>, LoweringError> {
    let mut visited = vec![false; plan.states.len()];
    if plan.states.is_empty() {
        return Ok(visited);
    }
    let mut ready = vec![0];
    visited[0] = true;
    let mut next = 0;
    while let Some(source) = ready.get(next).copied() {
        next += 1;
        for successor in successors(&plan.states[source]) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph target disappeared"))?;
            if !visited[target] {
                visited[target] = true;
                ready.push(target);
            }
        }
    }
    Ok(visited)
}
