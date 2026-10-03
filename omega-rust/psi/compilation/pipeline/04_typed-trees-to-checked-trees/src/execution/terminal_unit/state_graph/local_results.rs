//! Selected successors partition existing local obligations, not value storage.

use super::super::{CheckedUnitStructuralResultBindingPlan, SymbolHandle};
use super::{
    CheckedStructuralControlSuccessorPlan, Multiplicity, StatementNode, TypeReferenceNode,
    TypedTrees,
};
use crate::execution::terminal_unit::types::type_graph_requires_nominal_drop;

#[cfg(test)]
mod tests;
/// A state-exit receipt permits disposal only on successors that do not move
/// this local. Keep the operation's global discard flag false: scalar edge
/// operands still observe its original home before selected-edge cleanup.
pub(super) fn permits_disposal(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
    successors: &[&CheckedStructuralControlSuccessorPlan],
    disposable_locals: &[SymbolHandle],
) -> bool {
    validate(program, state, result, successors, disposable_locals).is_some()
}

fn validate(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
    successors: &[&CheckedStructuralControlSuccessorPlan],
    disposable_locals: &[SymbolHandle],
) -> Option<()> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = statements.get(result.statement_index as usize)? else {
        return None;
    };
    // Byte results retain the canonical storage identity their constructors
    // and call signatures publish. Qualifications are separate obligations,
    // not part of that identity; this applies equally to owned buffers and
    // borrowed views. Element views retain their existing carrier identity.
    let expected_identity = if let Some(identity) =
        super::super::types::byte_sequence_type_identity(program, local.type_reference, &[], &[])
    {
        identity
    } else if super::super::types::borrowed_slice_view_element(program, local.type_reference, &[])
        .is_some()
    {
        super::super::types::borrowed_slice_view_type_identity(
            program,
            local.type_reference,
            &[],
            &[],
        )
    } else {
        program
            .normalized_type_identity(local.type_reference)
            .into_string()
    };
    // A reference-loaned result publishes its referent identity — the loan's
    // access mode lives on the root, not the bound value — the same peel the
    // shared-referee admission and call-result storage paths apply. An owned
    // or view-carried binding keeps the declared type's full spelling.
    let referent_identity = {
        let mut cursor = local.type_reference;
        loop {
            match program.type_reference_table.type_reference(cursor) {
                TypeReferenceNode::Constrained { base_type, .. } => cursor = *base_type,
                TypeReferenceNode::Reference { referee, .. } => {
                    break Some(program.normalized_type_identity(*referee).into_string());
                }
                _ => break None,
            }
        }
    };
    if !local.symbol.is_valid()
        || program
            .state_local_declaration(state, local.symbol)
            .is_none()
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == local.symbol)
        || !(result.type_identity == expected_identity
            || referent_identity.as_ref() == Some(&result.type_identity))
        || result.multiplicity != program.type_multiplicity(local.type_reference)
        // Unrestricted results carry no owned custody; the owned-contents and
        // nominal-drop gates only bound results that hold storage. A borrowed
        // view's death is the end of the loan, not a disposal.
        || (result.multiplicity != Multiplicity::Unrestricted
            && (!(validation::has_plain_owned_contents_with_represented_constraints(
                program,
                local.type_reference,
            ) || validation::has_owned_or_shared_view_fields(
                program,
                local.type_reference,
            )) || type_graph_requires_nominal_drop(program, local.type_reference)))
    {
        return None;
    }
    // A normal Unit return has no successor transfer. It still needs the same
    // exact lexical disposal receipt as a non-transferring selected edge.
    let mut has_disposal_edge = successors.is_empty();
    for successor in successors {
        if result.statement_index >= successor.statement_ordinal {
            return None;
        }
        // An edge whose operand call takes the local whole moves it there
        // just as a direct transfer does; only a non-moving edge disposes.
        match successor.result_moves(result) {
            0 => has_disposal_edge = true,
            1 => {}
            _ => return None,
        }
    }
    if !has_disposal_edge {
        return None;
    }
    // Unrestricted payloads need dominance, not a fabricated cleanup receipt.
    if result.multiplicity == Multiplicity::Unrestricted {
        return Some(());
    }
    // Permission eligibility has one producer owner. Here it is rejoined to
    // this operation's result identity and selected successor partition.
    (result.multiplicity == Multiplicity::Affine && disposable_locals.contains(&local.symbol))
        .then_some(())
}
