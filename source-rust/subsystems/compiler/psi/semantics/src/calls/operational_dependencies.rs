//! Separate an applied machine's own waits from those of the workers it invokes.
use terminal_psi::{ContractDependency, TerminalMachine, TerminalModule};

pub fn direct(
    module: &TerminalModule,
    machine: &TerminalMachine,
    binder_bounds: impl Fn(u32) -> Option<[bool; 2]>,
) -> Result<[ContractDependency<bool, Vec<u32>>; 2], &'static str> {
    let mut terms: [ContractDependency<bool, Vec<u32>>; 2] = Default::default();
    let mut fixed = [false; 2];
    let mut include = |suspend, block| {
        fixed[0] |= suspend;
        fixed[1] |= block;
    };
    for call in machine.module_invocations() {
        if let Some(binder) = call.static_reach_binding {
            let bound =
                binder_bounds(binder).ok_or("worker call lost its declared callable binder")?;
            for (term, may) in terms.iter_mut().zip(bound) {
                if may {
                    term.parameters.push(binder);
                }
            }
        } else {
            let header = module
                .invocation_machine_contract(call.inputs.callee, call.inputs.required_contract)
                .ok_or("fixed call has no public operational contract")?;
            include(header.published_may_suspend, header.published_may_block);
        }
    }
    for callee in machine
        .call_formations
        .iter()
        .filter_map(|call| call.tail_callee)
    {
        let header = module
            .machine(callee)
            .ok_or("tail call has no public operational contract")?;
        include(header.published_may_suspend, header.published_may_block);
    }
    for call in machine.boundary_invocations() {
        let header = module
            .execution
            .callables
            .boundary_envelope(call.boundary)
            .ok_or("boundary call has no public operational contract")?;
        include(header.suspends, header.blocks);
    }
    for call in machine.callable_invocations() {
        let contract = module
            .execution
            .callables
            .contract(call.inputs.contract)
            .ok_or("runtime call has no public operational contract")?;
        include(contract.envelope.suspends, contract.envelope.blocks);
    }
    for ((term, fixed), published) in terms
        .iter_mut()
        .zip(fixed)
        .zip([machine.published_may_suspend, machine.published_may_block])
    {
        term.parameters.sort_unstable();
        term.parameters.dedup();
        term.concrete = fixed || (term.parameters.is_empty() && published);
    }
    Ok(terms)
}
