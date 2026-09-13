//! Selected successors partition existing local obligations, not value storage.

use super::*;

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
    if !local.symbol.is_valid()
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || statements
            .iter()
            .filter(|statement| {
                matches!(statement,
            StatementNode::LocalData(candidate) if candidate.symbol == local.symbol)
            })
            .count()
            != 1
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == local.symbol)
        || result.type_identity
            != program
                .normalized_type_identity(local.type_reference)
                .as_str()
        || result.multiplicity != program.type_multiplicity(local.type_reference)
        || !validation::has_plain_owned_contents_with_numeric_constraints(
            program,
            local.type_reference,
        )
        || type_graph_requires_nominal_drop(program, local.type_reference)
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
        let count = successor.transfers.iter().filter(|transfer| matches!(transfer.source,
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
                if binding_ordinal == result.binding_ordinal)).count();
        match count {
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
