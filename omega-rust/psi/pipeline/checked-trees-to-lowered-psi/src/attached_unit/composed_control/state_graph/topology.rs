//! Acyclic topology admission is independent of descriptor bindings.

use super::*;

pub(super) fn validate(plan: &CheckedComposedUnitControlMachinePlan) -> Result<(), LoweringError> {
    let mut incoming = vec![0_usize; plan.states.len()];
    for state in &plan.states {
        for successor in successors(state) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph edge has no target"))?;
            incoming[target] += 1;
        }
    }
    if incoming[0] != 0 || incoming[1..].contains(&0) {
        return unsupported("Unit graph is cyclic or has unreachable states");
    }
    let mut ready = vec![0];
    let mut next = 0;
    while let Some(source) = ready.get(next).copied() {
        next += 1;
        for successor in successors(&plan.states[source]) {
            let target = plan
                .states
                .iter()
                .position(|state| state.state == successor.target_state)
                .ok_or(LoweringError::Unsupported("Unit graph target disappeared"))?;
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push(target);
            }
        }
    }
    if next != plan.states.len() {
        return unsupported("Unit graph cyclic safety is not retained");
    }
    Ok(())
}
