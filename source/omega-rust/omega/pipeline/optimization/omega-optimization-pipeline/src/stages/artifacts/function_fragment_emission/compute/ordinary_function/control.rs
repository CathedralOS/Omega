use omega_machine_code::{FunctionFragmentControlProvenance, FunctionFragmentSuccessorProvenance};
use omega_selected_instructions::{SelectedBlock, SelectedInstructionId, SelectedTerminator};

pub(super) fn provenance(
    block: &SelectedBlock,
    instruction: SelectedInstructionId,
) -> FunctionFragmentControlProvenance {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction => FunctionFragmentControlProvenance::ConditionalBranch {
            when_nonzero: FunctionFragmentSuccessorProvenance {
                psi_edge: when_nonzero.psi_edge,
                block: when_nonzero.block,
                source_target: when_nonzero.source_target,
                bindings: when_nonzero.bindings.clone(),
                fuel: when_nonzero.fuel.clone(),
            },
            when_zero: FunctionFragmentSuccessorProvenance {
                psi_edge: when_zero.psi_edge,
                block: when_zero.block,
                source_target: when_zero.source_target,
                bindings: when_zero.bindings.clone(),
                fuel: when_zero.fuel.clone(),
            },
        },
        SelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction => FunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => FunctionFragmentControlProvenance::None,
    }
}
