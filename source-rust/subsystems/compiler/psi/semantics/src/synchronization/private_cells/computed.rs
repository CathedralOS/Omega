//! Obtain a computed call's possible bodies from the original executable entry.
//! A helper's formal keeps its caller's selection and checked provider bindings.

use super::super::progress::ScalarFact;
use super::super::selected::SelectedCall;
use super::activation::Frame;
use crate::callable_values::{
    CallableBoundaryImplementation, ReachableCallableInvocation, ReachableCallableInvocations,
    reachable_callable_invocations, terminal_callable_values,
};
use semantic_vocabulary::{
    BoundaryMachineId, CallableInvocationSite, InvocationSite, MachineId, OperationId,
};
use std::cell::OnceCell;
use terminal_psi::{CallableInvocationInputs, TerminalModule};

pub(super) struct Invocations {
    entry: MachineId,
    reached: OnceCell<Option<ReachableCallableInvocations>>,
}

impl Invocations {
    pub(super) fn new(entry: MachineId) -> Self {
        Self {
            entry,
            reached: OnceCell::new(),
        }
    }

    /// A value in this activation fixes one target even when the same helper
    /// was called elsewhere with a different descriptor.
    pub(super) fn selected_at(
        &self,
        module: &TerminalModule,
        boundary: &impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
        frame: &Frame<'_>,
        operation: OperationId,
        call: &CallableInvocationInputs<'_>,
    ) -> Option<MachineId> {
        if let Some(ScalarFact::Literal(value)) = frame.bindings.values.get(&call.callee) {
            return value.selected_machine(&module.execution.callables, call.contract);
        }
        let reached = self.site(module, boundary, frame.machine.id, operation)?;
        let [target] = reached.selected_machines.as_slice() else {
            return None;
        };
        reached.targets_are_resolved().then_some(*target)
    }

    pub(super) fn site(
        &self,
        module: &TerminalModule,
        boundary: &impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
        machine: MachineId,
        operation: OperationId,
    ) -> Option<&ReachableCallableInvocation> {
        self.reached
            .get_or_init(|| {
                let supplied = module
                    .interfaces
                    .boundary_machines
                    .iter()
                    .filter_map(|row| {
                        let SelectedCall::Local(body) = boundary(row.id)? else {
                            return None;
                        };
                        Some(CallableBoundaryImplementation {
                            boundary: row.id,
                            machine: body,
                        })
                    })
                    .collect::<Vec<_>>();
                let program = terminal_callable_values(module)
                    .ok()?
                    .with_boundary_implementations(&supplied)
                    .ok()?;
                reachable_callable_invocations(&program, [self.entry]).ok()
            })
            .as_ref()?
            .site(
                machine,
                CallableInvocationSite::Invocation(InvocationSite::Operation(operation)),
            )
    }
}
