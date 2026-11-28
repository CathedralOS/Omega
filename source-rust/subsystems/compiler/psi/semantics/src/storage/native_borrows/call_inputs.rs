//! Follow a borrowed address back to callers that had to supply that address.
//! A scalar can be forwarded, compared or discarded after its owner returns.
//! Only the operation that forms a borrow imposes a live-storage requirement.
use super::{NativeAddressOrigin, address_origin};
use semantic_vocabulary::{BlockId, InvocationSite, MachineId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralArgument, TerminalMachine, TerminalModule};

fn sole_parameter(origin: &NativeAddressOrigin<StructuralArgument>) -> Option<ValueId> {
    let [formal] = origin.parameters.as_slice() else {
        return None;
    };
    (origin.exact && !origin.opaque && !origin.expired && origin.sources.is_empty())
        .then_some(*formal)
}

// An address may be irrelevant on a branch that returns, crashes or loops.
// A predecessor requires the borrow only after all of its successors do.
fn necessary_block(machine: &TerminalMachine, block: BlockId) -> bool {
    if machine.entry == block {
        return true;
    }
    let mut predecessors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut remaining = BTreeMap::new();
    for body in &machine.blocks {
        let successors = body.terminator.successor_targets();
        remaining.insert(body.id, successors.len());
        for target in successors {
            predecessors.entry(target).or_default().push(body.id);
        }
    }
    let mut required = vec![block];
    while let Some(known) = required.pop() {
        for predecessor in predecessors.remove(&known).into_iter().flatten() {
            let count = remaining
                .get_mut(&predecessor)
                .expect("indexed predecessor");
            *count -= 1;
            if *count == 0 {
                if predecessor == machine.entry {
                    return true;
                }
                required.push(predecessor);
            }
        }
    }
    false
}

pub fn receives_expired_address(
    module: &TerminalModule,
    borrower: &TerminalMachine,
    block: BlockId,
    origin: &NativeAddressOrigin<StructuralArgument>,
) -> bool {
    let Some(formal) = sole_parameter(origin) else {
        return false;
    };
    if !necessary_block(borrower, block) {
        return false;
    }
    let mut incoming = BTreeMap::<MachineId, Vec<_>>::new();
    for caller in &module.machines {
        for block in &caller.blocks {
            for call in block.module_invocations() {
                if matches!(
                    call.site,
                    InvocationSite::Operation(_) | InvocationSite::InvokeNoNormal(_)
                ) {
                    incoming.entry(call.inputs.callee).or_default().push((
                        caller,
                        block.id,
                        call.inputs.arguments,
                    ));
                }
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![(borrower, formal)];
    while let Some((callee, formal)) = pending.pop() {
        if !visited.insert((callee.id, formal)) {
            continue;
        }
        let Some(ordinal) = callee.parameters.iter().position(|p| p.id == formal) else {
            continue;
        };
        for &(caller, block, arguments) in incoming.get(&callee.id).into_iter().flatten() {
            let Some(&value) = arguments.get(ordinal) else {
                continue;
            };
            let actual = address_origin(module, caller, value);
            if actual.only_expired_storage() {
                return true;
            }
            if let Some(input) = sole_parameter(&actual)
                && necessary_block(caller, block)
            {
                pending.push((caller, input));
            }
        }
    }
    false
}
