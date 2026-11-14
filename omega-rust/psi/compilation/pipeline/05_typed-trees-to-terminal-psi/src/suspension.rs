//! Suspension crossings of the lowered module.
//!
//! A call whose callee may suspend can park its caller. calls_and_outcomes.md,
//! "Suspension": "A suspension site binds the exact call, checked crossing,
//! callee, and commitment to the complete ordered live frontier." So every
//! such call operation is given a crossing, and the module states one site
//! and one plan for it. The frontier is read back from the finished module
//! (`terminal_semantics::suspension_frontier`), after every operation has
//! been emitted, so a plan describes the operations that exist and nothing
//! this lowering only intended.
//!
//! A terminator that calls is given none: the checker refuses a possibly
//! suspending call there by name, since a crossing is a property of an
//! operation.

use semantic_vocabulary::SuspensionCrossingId;
use terminal_psi::TerminalModule;
use terminal_semantics::suspension_frontier::{call_may_suspend, call_plan, call_site};

/// Marks every possibly suspending call operation of `module` with a
/// crossing, in machine, block and operation order, and publishes its site
/// and plan.
pub(crate) fn publish_crossings(module: &mut TerminalModule) -> Result<(), &'static str> {
    let mut crossings = Vec::new();
    for (machine_position, machine) in module.machines.iter().enumerate() {
        for (block_position, block) in machine.blocks.iter().enumerate() {
            for (position, operation) in block.operations.iter().enumerate() {
                if call_may_suspend(module, operation) == Some(true) {
                    crossings.push((machine_position, block_position, position));
                }
            }
        }
    }
    if crossings.is_empty() {
        return Ok(());
    }
    for (ordinal, (machine, block, position)) in crossings.iter().enumerate() {
        let crossing = u64::try_from(ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(1))
            .and_then(SuspensionCrossingId::new)
            .ok_or("a module has more suspension crossings than identities")?;
        module.machines[*machine].blocks[*block].operations[*position].suspension_crossing =
            Some(crossing);
    }
    let mut plans = Vec::with_capacity(crossings.len());
    for (machine, block, position) in &crossings {
        let machine = &module.machines[*machine];
        let operation = &machine.blocks[*block].operations[*position];
        let crossing = operation
            .suspension_crossing
            .ok_or("a suspension crossing was not recorded on its operation")?;
        plans.push(call_plan(module, machine, operation.id, crossing)?);
    }
    plans.sort_by_key(|plan| (plan.operation, plan.crossing));
    let sites = plans.iter().map(|plan| call_site(module, plan)).collect();
    module.suspension_call_plan_count = u32::try_from(plans.len())
        .map_err(|_| "a module has more suspension plans than it can count")?;
    module.suspension_call_sites = sites;
    module.suspension_call_plans = plans;
    Ok(())
}
