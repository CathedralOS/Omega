//! U32 ABI normalization and cyclic transport are independently selected facts.

use selected_instructions::SelectedInstructionKind;
use target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;

pub(super) fn reject_substituted_transport(selected: &StagedOptimizedSelectedInstructions) {
    let environment = selected.register_environment();
    let constraints = target_operations_to_selected_instructions::selection_constraints(
        selected.legalized(),
        environment,
    );
    let validate = |candidate| {
        target_operations_to_selected_instructions::validate_selected_instructions(
            selected.legalized(),
            &constraints,
            environment.physical(),
            environment.constraints(),
            candidate,
        )
    };
    let plan = selected.selected().plan();
    validate(plan.clone()).expect("unchanged ordinary ranked graph replays");
    let mut changed = plan.clone();
    let normalization = changed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find(|instruction| instruction.kind == SelectedInstructionKind::ZeroExtendU32)
        .expect("ranked U32 input is normalized instead of reading unspecified high bits");
    normalization.kind = SelectedInstructionKind::CopyI64;
    assert!(
        validate(changed).is_err(),
        "a full-width copy is not U32 ABI normalization"
    );

    let mut changed = plan.clone();
    let function = &mut changed.functions[0];
    let rank = function.ranked.as_ref().expect("retained ranking custody");
    let backedge = rank.ranked_scc.covered_cyclic_edges[0].edge;
    let transfer = function
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            selected_instructions::SelectedTerminator::Jump { successor, .. }
                if successor.psi_edge == backedge =>
            {
                Some(successor)
            }
            _ => None,
        })
        .expect("ordinary graph retains its authenticated backedge");
    transfer.source_target = rank.graph.done_block;
    assert!(
        validate(changed).is_err(),
        "a substituted exit cannot impersonate the ranked backedge"
    );
}
