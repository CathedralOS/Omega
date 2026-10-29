//! Borrow operational facts for the source-independent proof-application walk.

use super::operational::{OperationalPlan, StaticMachineCallBinding};
use arena::HandleSpan;
use semantic_vocabulary::proof_applications::{ProofApplicationCall, ProofApplicationCalls};
use symbols::SymbolHandle;

impl ProofApplicationCalls<SymbolHandle> for OperationalPlan {
    type Bindings = HandleSpan<StaticMachineCallBinding>;
    type Order = (u64, u64);

    fn declaration_order(declaration: SymbolHandle) -> (u64, u64) {
        (
            u64::from(declaration.arena_index()),
            u64::from(declaration.generation()),
        )
    }

    fn visit_calls(
        &self,
        machine: SymbolHandle,
        mut visit: impl FnMut(ProofApplicationCall<SymbolHandle, Self::Bindings>),
    ) -> bool {
        let Some(summary) = self.machine_by_symbol(machine) else {
            return false;
        };
        for state in self.states.span_or_empty(summary.states) {
            for call in self.calls.span_or_empty(state.calls) {
                visit(ProofApplicationCall {
                    binder: if call.static_machine_parameter.is_valid() {
                        call.static_machine_parameter
                    } else {
                        call.target_state_symbol
                    },
                    callee: call.target_machine_symbol,
                    state: call.target_state_symbol,
                    operator: call.target_operator_symbol,
                    arguments: call.static_machine_bindings,
                });
            }
        }
        true
    }

    fn visit_bindings(
        &self,
        bindings: Self::Bindings,
        mut visit: impl FnMut(SymbolHandle, SymbolHandle, Self::Bindings),
    ) {
        for binding in self.static_machine_bindings.span_or_empty(bindings) {
            visit(binding.parameter, binding.selected, binding.arguments);
        }
    }

    fn visit_cleanup(&self, machine: SymbolHandle, mut visit: impl FnMut(SymbolHandle)) {
        for (_, cleanup) in self.cleanup_dependencies.iter() {
            if cleanup.caller_machine == machine {
                visit(cleanup.target_machine);
            }
        }
    }
}
