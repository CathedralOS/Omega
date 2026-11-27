//! Obtain a computed call's possible bodies from the original executable entry.
//! A helper's formal keeps its caller's selection and checked provider bindings.

use super::super::selected::SelectedCall;
use crate::callable_values::{
    CallableBoundaryImplementation, ReachableCallableInvocation, ReachableCallableInvocations,
    reachable_callable_invocations, terminal_callable_values,
};
use semantic_vocabulary::{
    BoundaryMachineId, CallableInvocationSite, InvocationSite, MachineId, OperationId,
};
use std::cell::OnceCell;
use terminal_psi::TerminalModule;

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
