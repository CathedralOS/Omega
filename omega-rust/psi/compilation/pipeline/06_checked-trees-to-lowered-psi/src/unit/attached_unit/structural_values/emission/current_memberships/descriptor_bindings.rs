//! Live facts about a source path can describe a captured endpoint only while
//! every descriptor crossed before its selectors keeps its binding. Content
//! writes are checked separately at the final operand's own observation cut.
use arena::Handle;
use checked_trees::{CheckedTrees, FlowOperandFact};

pub(super) fn unchanged(checked: &CheckedTrees, operand: Handle<FlowOperandFact>) -> bool {
    let control = &checked.facts.flow.control;
    if !control.operands.is_valid(operand) {
        return false;
    }
    let completed = control.operands.get(operand);
    if completed.entry_binding_unknown_write {
        return false;
    }
    let Some(writes) = control
        .operand_origin_writes
        .span(completed.entry_binding_writes)
    else {
        return false;
    };
    let mut receiver = completed.backing;
    let mut visited = vec![operand];
    while receiver.is_valid() {
        if !control.operands.is_valid(receiver) || visited.contains(&receiver) {
            return false;
        }
        visited.push(receiver);
        let captured = control.operands.get(receiver);
        // These are statement-local prefixes; equal lengths in another
        // statement do not establish an unchanged descriptor association.
        if captured.state_symbol != completed.state_symbol
            || captured.statement_index != completed.statement_index
        {
            return false;
        }
        if validation::reference_shell(&checked.typed, captured.type_reference).is_some() {
            if captured.entry_binding_unknown_write {
                return false;
            }
            let Some(before) = control
                .operand_origin_writes
                .span(captured.entry_binding_writes)
            else {
                return false;
            };
            if !writes.starts_with(before) {
                return false;
            }
            let Some(subject) = validation::canonical_place_from_expression_in_state(
                &checked.typed,
                captured.state_symbol,
                captured.statement_index,
                captured.expression,
            ) else {
                return false;
            };
            for write in &writes[before.len()..] {
                if write.root != subject.root {
                    continue;
                }
                let Some(segments) = control.operand_referent_segments.span(write.segments) else {
                    return false;
                };
                if validation::storage_path_relations::binding_overwrite(
                    &checked.typed,
                    &subject,
                    segments,
                ) {
                    return false;
                }
            }
        }
        receiver = captured.backing;
    }
    true
}
