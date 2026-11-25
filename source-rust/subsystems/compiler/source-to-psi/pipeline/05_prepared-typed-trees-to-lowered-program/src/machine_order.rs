//! Plan the machines of a module on the invocation's workers while keeping
//! exactly what planning and applying them one at a time, in order, yields.
//!
//! Storage and claim publication plan machine `k` against the module that
//! already holds the plans applied to the machines before it, so the loop
//! looks sequential. But applying a plan changes only the machine it was made
//! for, and most applications leave that machine as it was. A plan made in
//! advance against the starting module is the plan the loop would make as long
//! as no machine it read has changed by the time its turn comes.

use semantic_vocabulary::MachineId;
use terminal_psi::{TerminalMachine, TerminalModule};

/// Plan every machine of `module` and apply each plan to its machine in
/// machine order, with the result of the loop that plans machine `k` only
/// after applying the plans of the machines before it.
///
/// The machines are planned in advance on the invocation's workers against
/// the module as it stands, then applied in order. `reads` names the other
/// machines whose state, as `apply` can change it, a plan read; `None` when
/// that is unknown, which counts as every machine. When one of them has
/// changed before the plan's turn, the machine is planned again against the
/// module as it is then. `apply` may change only the machine it is given. The
/// first error `apply` returns ends the loop, as it would in order.
pub(crate) fn plan_then_apply<Plan: Send, Error>(
    module: &mut TerminalModule,
    plan: impl Fn(&TerminalModule, &TerminalMachine) -> Plan + Sync,
    reads: impl Fn(&Plan) -> Option<&[MachineId]>,
    mut apply: impl FnMut(&mut TerminalMachine, Plan) -> Result<(), Error>,
) -> Result<(), Error> {
    if worker_budget::current_workers() == 1 || module.machines.len() < 2 {
        for ordinal in 0..module.machines.len() {
            let next = plan(module, &module.machines[ordinal]);
            apply(&mut module.machines[ordinal], next)?;
        }
        return Ok(());
    }
    let planned = {
        use rayon::prelude::*;
        let module = &*module;
        module
            .machines
            .par_iter()
            .with_max_len(1)
            .map(|machine| plan(module, machine))
            .collect::<Vec<_>>()
    };
    // The machines an application changed so far, sorted.
    let mut changed: Vec<MachineId> = Vec::new();
    for (ordinal, advance) in planned.into_iter().enumerate() {
        // A plan finds its own machine by identity too, so that identity
        // counts among what it read.
        let unchanged = |machine: &MachineId| changed.binary_search(machine).is_err();
        let current = changed.is_empty()
            || unchanged(&module.machines[ordinal].id)
                && reads(&advance).is_some_and(|read| read.iter().all(unchanged));
        let next = if current {
            advance
        } else {
            plan(module, &module.machines[ordinal])
        };
        let before = module.machines[ordinal].clone();
        apply(&mut module.machines[ordinal], next)?;
        let machine = &module.machines[ordinal];
        if *machine != before
            && let Err(position) = changed.binary_search(&machine.id)
        {
            changed.insert(position, machine.id);
        }
    }
    Ok(())
}
