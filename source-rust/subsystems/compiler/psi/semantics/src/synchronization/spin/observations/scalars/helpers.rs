//! A scalar helper can compute a retry predicate from the words its caller
//! observed. Follow straight-line checked bodies and scalar tail delegation.
//! Each replacement of the current machine follows a single call edge.
use super::{Expressions, Input};
use crate::synchronization::facts::Facts;
use crate::synchronization::spin::{Operand, constant};
use semantic_vocabulary::{MachineId, ValueId};
use terminal_psi::{
    ModuleInvocationInputs, TargetMachineCallable, TerminalMachine, TerminalModule, Terminator,
};

impl Expressions {
    pub(in super::super) fn helper(
        &mut self,
        module: &TerminalModule,
        call: ModuleInvocationInputs<'_, TargetMachineCallable>,
        args: Vec<Input>,
    ) -> Input {
        let start = self.steps.len();
        if let Some(machine) = direct(call)
            && let Some(result) = self.helper_result(module, machine, args.clone())
        {
            return result;
        }
        self.steps.truncate(start);
        self.unresolved_call(args)
    }

    fn helper_result(
        &mut self,
        module: &TerminalModule,
        mut callee: MachineId,
        mut arguments: Vec<Input>,
    ) -> Option<Input> {
        let mut entered = Facts::default();
        'calls: loop {
            if entered.insert(callee, ()).is_some() {
                return None;
            }
            let machine = module.machine(callee)?;
            let types = crate::MachineScalarIndex::new(machine);
            let mut bindings = bind_parameters(machine, arguments)?;
            let mut visited = Facts::default();
            let mut block = machine.block(&machine.entry)?;
            loop {
                if visited.insert(block.id, ()).is_some() {
                    return None;
                }
                for (position, operation) in block.operations.iter().enumerate() {
                    let result = operation.result.scalar()?.id;
                    if let Some(call) = operation.declared_invocation() {
                        if position + 1 != block.operations.len()
                            || !matches!(&block.terminator, Terminator::Return { value, cleanup_actions, .. }
                                if *value == result && cleanup_actions.is_empty())
                        {
                            return None;
                        }
                        arguments = actuals(&bindings, call.arguments)?;
                        callee = direct(call)?;
                        continue 'calls;
                    }
                    let value = if let Some(word) = constant(operation) {
                        Input::Supplied(Operand::Literal(word))
                    } else {
                        Input::Previous(
                            self.apply(operation, &types, |id| bindings.get(&id).copied())?,
                        )
                    };
                    if bindings.insert(result, value).is_some() {
                        return None;
                    }
                }
                match &block.terminator {
                    Terminator::Return {
                        value,
                        cleanup_actions,
                        ..
                    } if cleanup_actions.is_empty() => return bindings.get(value).copied(),
                    Terminator::Jump {
                        target,
                        arguments,
                        structural_arguments,
                        cleanup_actions,
                        ..
                    } if structural_arguments.is_empty() && cleanup_actions.is_empty() => {
                        let supplied = actuals(&bindings, arguments)?;
                        block = machine.block(target)?;
                        if !block.structural_parameters.is_empty()
                            || block.parameters.len() != supplied.len()
                        {
                            return None;
                        }
                        for (formal, value) in block.parameters.iter().zip(supplied) {
                            if bindings.insert(formal.id, value).is_some() {
                                return None;
                            }
                        }
                    }
                    _ => return None,
                }
            }
        }
    }
}

fn direct(call: ModuleInvocationInputs<'_, TargetMachineCallable>) -> Option<MachineId> {
    if !call.structural_arguments.is_empty()
        || !call.claim_transfers.is_empty()
        || !call.completion_receipts.is_empty()
    {
        return None;
    }
    match call.callee {
        TargetMachineCallable::Machine(callee) => Some(callee),
        TargetMachineCallable::Boundary(_) => None,
    }
}

fn bind_parameters(machine: &TerminalMachine, args: Vec<Input>) -> Option<Facts<ValueId, Input>> {
    if !machine.structural_parameters.is_empty() || machine.parameters.len() != args.len() {
        return None;
    }
    Some(
        machine
            .parameters
            .iter()
            .map(|formal| formal.id)
            .zip(args)
            .collect(),
    )
}

fn actuals(bindings: &Facts<ValueId, Input>, args: &[ValueId]) -> Option<Vec<Input>> {
    args.iter().map(|id| bindings.get(id).copied()).collect()
}
