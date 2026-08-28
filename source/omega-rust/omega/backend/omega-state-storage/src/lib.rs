mod collection;
mod model;
mod mutation_kind;

pub use collection::{
    build_state_storage_plan, build_state_storage_plan_owned, build_state_storage_plan_with_workers,
};
pub use model::{
    StateLocalStorage, StateMutation, StateMutationKind, StateMutationLowering, StateStoragePlan,
};
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_state_calls::StateCallPlan;
use omega_state_graph::RuntimeFlowPlan;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStoragePlanningContext {
    pub control_flow: Arc<ControlFlowPlan>,
    pub runtime_flow: Arc<RuntimeFlowPlan>,
    pub state_calls: Arc<StateCallPlan>,
}

impl StateStoragePlanningContext {
    pub fn state_flow_by_key(&self, state_key: StateKey) -> Option<&omega_control_flow::StateFlow> {
        self.control_flow.state_by_key(state_key).or_else(|| {
            self.control_flow.states.iter().find_map(|(_, state)| {
                (state.key.machine == state_key.machine && state.key.state == state_key.state)
                    .then_some(state)
            })
        })
    }

    pub fn borrow_root_kind_by_symbol(
        &self,
        state_key: StateKey,
        symbol: psi_symbols::SymbolHandle,
    ) -> Option<omega_control_flow::StateBorrowRootKind> {
        if !symbol.is_valid() {
            return None;
        }

        self.state_flow_by_key(state_key)
            .and_then(|state| {
                self.control_flow
                    .semantics
                    .borrow
                    .writable_roots
                    .span(state.borrow.writable_roots)
            })
            .and_then(|roots| roots.iter().find(|root| root.symbol == symbol))
            .map(|root| root.kind.clone())
    }

    pub fn state_is_required_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
            || self.state_calls.required_state(state_key)
    }

    pub fn state_uses_runtime_flow_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
    }

    pub fn state_mutation_is_already_lowered_by_key(
        &self,
        state_key: StateKey,
        statement_index: usize,
    ) -> bool {
        self.control_flow
            .state_by_key(state_key)
            .and_then(|state| self.control_flow.operations.span(state.operations))
            .is_some_and(|operations| {
                operations.iter().any(|operation| {
                    operation.statement_index == statement_index
                        && matches!(
                            operation.kind,
                            omega_control_flow::OperationKind::ConstantIntegerAssignment
                                | omega_control_flow::OperationKind::StaticAssignment
                        )
                })
            })
    }
}
