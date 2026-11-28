//! Visit each selected export and its local callees once, callee first.
use super::NormalEffects;
use crate::synchronization::{facts::Facts, selected::SelectedCall};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{TargetMachineCallable, TerminalModule};

/// Compute each locally called body after its callees. Recursive calls have
/// no completed summary and therefore cannot establish a normal return.
pub(in crate::synchronization) fn normal_effects(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<NormalEffects> {
    let mut seen = Facts::default();
    let mut completed = Facts::default();
    let mut pending = vec![(entry, false)];
    let mut program_size = 0usize;
    let mut order = Vec::new();
    while let Some((id, expanded)) = pending.pop() {
        if expanded {
            order.push(id);
            continue;
        }
        if seen.insert(id, ()).is_some() {
            continue;
        }
        let body = module.machine(id)?;
        pending.push((id, true));
        for op in body.operations() {
            program_size += 1;
            if let Some(call) = op.declared_invocation()
                && !matches!(op.result, terminal_psi::OperationResult::Structural(_))
                && let TargetMachineCallable::Machine(callee) = call.callee
            {
                pending.push((callee, false));
            }
        }
    }
    for id in order {
        let body = module.machine(id)?;
        let effects =
            super::sequence::effects_in_body(module, body, &boundary, &completed, program_size);
        completed.insert(id, effects);
    }
    completed.remove(&entry)?
}
