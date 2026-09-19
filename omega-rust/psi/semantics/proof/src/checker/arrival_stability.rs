use crate::checker::assignment_stability::resolved_writes_overlap_reads;
use crate::obligations::ProofPlan;
use typed_trees::statement::StatementNode;

/// Effects before one consumption point in an immutable typed state. Guard
/// conjuncts may have different dependencies, but they cross the same writes.
/// Keep local definitions separate: they can supply value operands while still
/// shadowing names used by an incoming premise.
pub(super) struct ArrivalPrefix<'program> {
    written_paths: Vec<String>,
    local_definitions: Vec<&'program str>,
}

impl ArrivalPrefix<'_> {
    pub(super) fn preserves_reads(
        &self,
        premise_reads: &[Vec<String>],
        value_reads: &[Vec<String>],
    ) -> bool {
        !resolved_writes_overlap_reads(&self.written_paths, value_reads)
            && !self.local_definitions.iter().any(|name| {
                premise_reads
                    .iter()
                    .any(|read| read.first().is_none_or(|root| root == name))
            })
    }
}

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
    prepare_prefix(proof_plan, machine, state, statement_index, call_frames)
        .is_some_and(|prefix| prefix.preserves_reads(premise_reads, value_reads))
}

/// Resolve each effect once, independently of how many premises will query it.
/// The consuming expression's effects count; its destination write does not.
pub(super) fn prepare_prefix<'program>(
    proof_plan: &ProofPlan<'program>,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
    call_frames: &validation::CallFrameResolver<'program>,
) -> Option<ArrivalPrefix<'program>> {
    let statements = proof_plan
        .program
        .statement_table
        .statements(state.statement_nodes);
    if statement_index >= statements.len() {
        return None;
    }
    let mut prefix = ArrivalPrefix {
        written_paths: Vec::new(),
        local_definitions: Vec::new(),
    };
    for (index, statement) in statements.iter().enumerate().take(statement_index + 1) {
        prefix
            .written_paths
            .extend(call_frames.statement_value_may_write_paths(machine, statement)?);
        if let StatementNode::Call(call) = statement {
            prefix
                .written_paths
                .extend(call_frames.may_write_paths(machine, call)?);
        }
        if index == statement_index {
            return Some(prefix);
        }
        match statement {
            StatementNode::Assignment(_) => {
                prefix.written_paths.extend(
                    call_frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths()?,
                );
            }
            StatementNode::LocalData(local) => {
                // A fresh value local may define a hoisted operand. A binding
                // shadowing an arrival premise must not revive that premise.
                prefix.local_definitions.push(local.name.as_str());
            }
            StatementNode::Call(_) | StatementNode::Expression(_) => {}
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::ArrivalPrefix;

    fn path(members: &[&str]) -> Vec<String> {
        members.iter().map(|member| (*member).to_owned()).collect()
    }

    #[test]
    fn prepared_prefix_keeps_independent_guard_conjuncts() {
        let prefix = ArrivalPrefix {
            written_paths: vec!["self.scratch".into()],
            local_definitions: Vec::new(),
        };
        let scratch = [path(&["self", "scratch"])];
        let direction = [path(&["self", "direction"])];
        assert!(!prefix.preserves_reads(&scratch, &scratch));
        assert!(prefix.preserves_reads(&direction, &direction));
        assert!(!prefix.preserves_reads(&direction, &scratch));
        assert!(prefix.preserves_reads(&direction, &direction));
    }

    #[test]
    fn local_definition_can_supply_a_value_but_cannot_shadow_a_premise() {
        let prefix = ArrivalPrefix {
            written_paths: Vec::new(),
            local_definitions: vec!["operand"],
        };
        let operand = [path(&["operand"])];
        assert!(prefix.preserves_reads(&[], &operand));
        assert!(!prefix.preserves_reads(&operand, &operand));
        assert!(!prefix.preserves_reads(&[Vec::new()], &[]));
    }

    #[test]
    fn prepared_prefix_preserves_parent_child_alias_rejection() {
        let prefix = ArrivalPrefix {
            written_paths: vec!["self.record.value".into()],
            local_definitions: Vec::new(),
        };
        assert!(!prefix.preserves_reads(&[], &[path(&["self", "record"])]));
        assert!(!prefix.preserves_reads(&[], &[path(&["self", "record", "value"])]));
        assert!(prefix.preserves_reads(&[], &[path(&["self", "record", "other"])]));
    }
}
