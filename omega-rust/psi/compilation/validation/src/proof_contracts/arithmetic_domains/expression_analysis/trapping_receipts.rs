//! Optional trap-free output follows its actual contract consumers.
//!
//! Mandatory formation and published-ceiling checks remain in the arithmetic
//! walk. This plan only asks which inferred private summaries need precision:
//! restricted published ceilings and open callable uses may consume it, while
//! an unconditional Trap ceiling cannot. Reuse the existing selected private
//! invocation/use inventory, including escaping static selections. No path fact
//! is a planning premise; every requested receipt still proves all arrivals.
//! Unrequested summaries keep the conservative physical Trap possibility.

use super::trap_coverage::{TrapCeiling, trap_ceiling};
use symbols::{SymbolHandle, SymbolKeyMap};
use typed_trees::machine::Machine;

pub(crate) struct TrappingReceiptDemand {
    required: SymbolKeyMap<SymbolHandle, ()>,
}

impl TrappingReceiptDemand {
    fn build(program: &dyn crate::CheckingInput) -> Self {
        let invocations = crate::proof_contracts::contract_entailment::structural_prefix::private_invocation_index(program);
        let mut required = SymbolKeyMap::default();
        let mut pending = Vec::new();
        for machine in program.machines() {
            let precise = match trap_ceiling(program, machine) {
                TrapCeiling::Guarded(_) => true,
                TrapCeiling::Permitted => {
                    !program.machine_publishes_crash_ceiling(machine)
                        && !invocations.has_closed_private_uses(program, machine)
                }
            };
            if precise {
                required.insert(machine.symbol, ());
                pending.push(machine.symbol);
            }
        }
        while let Some(caller) = pending.pop() {
            for &callee in invocations.private_callees(caller) {
                let Some(machine) = program.machine_by_symbol(callee) else {
                    continue;
                };
                // A published callee keeps its authored interface. Its own
                // restricted ceiling was independently seeded above; caller
                // precision cannot narrow an unconditional published Trap.
                if !program.machine_publishes_crash_ceiling(machine)
                    && required.insert(callee, ()).is_none()
                {
                    pending.push(callee);
                }
            }
        }
        Self { required }
    }
}

pub(super) fn required(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    let demand = crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.trapping_receipt_demand,
        || TrappingReceiptDemand::build(program),
    );
    demand.required.contains_key(&machine.symbol)
}
