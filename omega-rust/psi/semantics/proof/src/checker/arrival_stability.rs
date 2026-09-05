use super::*;

/// An arrival premise may refine a value only while its dependencies survive.
/// Include the consuming expression's call effects, but not its subsequent
/// destination write. Unknown effects and control flow remain conservative.
pub(super) fn prefix_preserves_reads<'program>(
    proof_plan: &ProofPlan<'program>,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
    premise_reads: &[Vec<String>],
    value_reads: &[Vec<String>],
    call_frames: &validation::CallFrameResolver<'program>,
) -> bool {
    let statements = proof_plan
        .program
        .statement_table
        .statements(state.statement_nodes);
    if statement_index >= statements.len() {
        return false;
    }
    for (index, statement) in statements.iter().enumerate().take(statement_index + 1) {
        let Some(written) = call_frames.statement_value_may_write_paths(machine, statement) else {
            return false;
        };
        if resolved_writes_overlap_reads(&written, value_reads) {
            return false;
        }
        if let StatementNode::Call(call) = statement {
            let Some(written) = call_frames.may_write_paths(machine, call) else {
                return false;
            };
            if resolved_writes_overlap_reads(&written, value_reads) {
                return false;
            }
        }
        if index == statement_index {
            return true;
        }
        match statement {
            StatementNode::Assignment(_) => {
                let Some(written) = call_frames
                    .assignment_write_frame(machine, statement)
                    .into_complete_paths()
                else {
                    return false;
                };
                if resolved_writes_overlap_reads(&written, value_reads) {
                    return false;
                }
            }
            StatementNode::LocalData(local) => {
                // A fresh value local may define a hoisted operand. A binding
                // shadowing an arrival premise must not revive that premise.
                let written = vec![local.name.as_str().to_owned()];
                if premise_reads
                    .iter()
                    .any(|read| member_paths_may_alias(read, &written))
                {
                    return false;
                }
            }
            StatementNode::Call(_) | StatementNode::Expression(_) => {}
            _ => return false,
        }
    }
    false
}
