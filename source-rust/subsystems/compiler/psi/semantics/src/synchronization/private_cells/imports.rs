//! A loss of caller storage facts cannot discharge an independent supplier's
//! obligation. Look only in the unfinished suffix of the selected call stack.

use super::super::facts::Facts;
use super::super::progress::{PrivateWaitFailure, ProgressFailure};
use super::super::selected::SelectedCall;
use super::activation::Frame;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{TargetMachineCallable, TerminalAffineCleanupAction, TerminalModule};

pub(super) fn unproved_continuation(
    module: &TerminalModule,
    frames: &[Frame<'_>],
    boundary: &impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
    computed: &super::computed::Invocations,
) -> Option<PrivateWaitFailure> {
    let mut pending = frames
        .iter()
        .map(|frame| (frame.machine.id, frame.block.id, frame.position))
        .collect::<Vec<_>>();
    let mut visited = Facts::default();
    while let Some((machine_id, block_id, first)) = pending.pop() {
        // A prior visit starting earlier also inspected this suffix.
        let start = visited.get_or_insert_with((machine_id, block_id), || usize::MAX);
        if *start <= first {
            continue;
        }
        *start = first;
        let machine = module.machine(machine_id)?;
        let block = machine.block(&block_id)?;
        let mut enter = |callee: MachineId| {
            if let Some(body) = module.machine(callee) {
                pending.push((body.id, body.entry, 0));
            }
        };
        for op in block.operations.iter().skip(first) {
            if let Some(call) = op.kind.callable_invocation() {
                let targets = computed.site(module, boundary, machine_id, op.id);
                if let Some(row) = targets {
                    row.selected_machines.iter().copied().for_each(&mut enter);
                }
                if !targets.is_some_and(|row| row.targets_are_resolved()) {
                    let envelope = module
                        .execution
                        .callables
                        .contract(call.contract)
                        .map(|c| &c.envelope);
                    if envelope
                        .is_none_or(|e| !e.invocations.is_empty() || !e.service_reach.is_empty())
                    {
                        return Some(PrivateWaitFailure {
                            machine: machine_id,
                            operation: op.id,
                            cause: ProgressFailure::UnprovedImport,
                        });
                    }
                }
                continue;
            }
            let Some(call) = op.declared_invocation() else {
                continue;
            };
            match SelectedCall::resolve(call.callee, boundary) {
                Some(SelectedCall::ImportedWait(_) | SelectedCall::UnresolvedImport) => {
                    return Some(PrivateWaitFailure {
                        machine: machine.id,
                        operation: op.id,
                        cause: ProgressFailure::UnprovedImport,
                    });
                }
                Some(SelectedCall::Local(id)) => enter(id),
                None => {}
            }
        }
        if let Some((_, call)) = block.terminator.declared_invocation()
            && let TargetMachineCallable::Machine(id) = call.callee
        {
            enter(id);
        }
        for (_, actions) in block.terminator.cleanup_rosters() {
            for action in actions {
                if let TerminalAffineCleanupAction::InvokeOwned(invocation) = action {
                    enter(invocation.cleanup_machine);
                }
            }
        }
        pending.extend(
            block
                .terminator
                .successor_targets()
                .into_iter()
                .map(|target| (machine_id, target, 0)),
        );
    }
    None
}
