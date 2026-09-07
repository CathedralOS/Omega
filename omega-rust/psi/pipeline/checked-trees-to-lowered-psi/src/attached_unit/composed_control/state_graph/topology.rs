//! Source-state reachability is independent of descriptor bindings.

use super::*;

pub(super) fn validate(plan: &CheckedComposedUnitControlMachinePlan) -> Result<(), LoweringError> {
    let mut ready = vec![0];
    let mut visited = vec![false; plan.states.len()];
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
    if next != plan.states.len() {
        return unsupported("Unit graph has unreachable states");
    }
    Ok(())
}
