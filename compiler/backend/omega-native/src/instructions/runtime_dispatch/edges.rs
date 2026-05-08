use crate::runtime_dispatch::loop_plan::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};

use super::super::model::{SelectedInstruction, SelectedInstructionKind};

pub(super) fn select_runtime_dispatch_edge(
    edge: &RuntimeDispatchLoopEdge,
    source_machine: &str,
    source_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator: edge.guard_operator,
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: edge.order,
    });

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}
