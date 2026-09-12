//! Rejoin local establishment and the authored per-edge ownership partition.
//!
//! StateExit records the lexical affine remainder even when a selected edge
//! transfers that local. It is not an instruction to drop on every successor.
//! Reconstruct the transfer roster from source, check its exact permission rows,
//! then subtract the selected edge's transfers. The resulting reverse-ordered
//! local roots use Terminal's existing edge disposal, after argument evaluation.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{StatementNode, TransitionTargetNode};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};

pub(super) fn result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
    match operation {
        CheckedUnitEffectOperationPlan::StructuralCall {
            result,
            discard_result_on_return: false,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            result,
            discard_result_on_return: false,
            ..
        }
        | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return: false,
            ..
        } => Some(result),
        _ => None,
    }
}

/// Validate the complete roster, not a requirement to transfer on every branch.
pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    local: &checked_trees::statement::TableLocalData,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    if checked.type_multiplicity(local.type_reference) != result.multiplicity
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
    {
        return unsupported("Unit graph local disposition type drifted");
    }
    let events = || {
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == source.symbol
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
            })
    };
    // Plain copy payloads retain producer dominance, not affine receipts. The
    // ordinary body rejoin establishes that producer before any edge use.
    if result.multiplicity == Multiplicity::Unrestricted {
        return if events().next().is_none() {
            Ok(())
        } else {
            unsupported("Unit graph copy local acquired ownership debt")
        };
    }
    let mut expected = vec![(
        PermissionEventKind::Establish,
        PermissionEventSource::Statement {
            statement_index: result.statement_index as usize,
        },
    )];
    if result.multiplicity == Multiplicity::Affine {
        expected.push((
            PermissionEventKind::AffineDrop,
            PermissionEventSource::StateExit,
        ));
    } else {
        return unsupported("Unit graph local requires an explicit linear disposition");
    }
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = checked.statement_table.transition_target(transition.target)
        else {
            continue;
        };
        for expression in checked.statement_table.expression_handles(*arguments) {
            if matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(name)
                if name.symbol == local.symbol && name.head_symbol == local.symbol
                    && checked.expression_table.name_path_members(name.members).len() == 1)
            {
                expected.push((
                    PermissionEventKind::Transfer,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal: 0,
                        target_symbol: path.symbol,
                    },
                ));
            }
        }
    }
    let provenance = PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: source.symbol,
        source: PermissionEventSource::Statement {
            statement_index: result.statement_index as usize,
        },
    };
    if events().count() != expected.len() {
        return unsupported("Unit graph local ownership roster drifted");
    }
    for (kind, source) in expected {
        let mut matching = events().filter(|event| event.kind == kind && event.source == source);
        let event = matching.next().ok_or(LoweringError::Unsupported(
            "Unit graph local ownership evidence missing",
        ))?;
        if matching.next().is_some()
            || event.multiplicity != result.multiplicity
            || event.access != PermissionAccess::Owned
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != provenance
            || event.obligation_live
            || !event.segments.is_empty()
        {
            return unsupported("Unit graph local ownership origin or disposition drifted");
        }
    }
    Ok(())
}

pub(super) fn successor_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
) -> Result<Vec<u32>, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let mut discards = Vec::new();
    for result in state.operations.iter().rev().filter_map(result) {
        let Some(StatementNode::LocalData(local)) = statements.get(result.statement_index as usize)
        else {
            return unsupported("Unit graph edge result has no authored local");
        };
        if result.statement_index >= edge.statement_ordinal {
            return unsupported("Unit graph edge precedes local establishment");
        }
        validate(checked, machine, source, local, result)?;
        let transferred = edge.transfers.iter().filter(|transfer| matches!(transfer.source,
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
                if binding_ordinal == result.binding_ordinal)).count();
        if transferred > 1 {
            return unsupported("Unit graph edge duplicates local ownership");
        }
        if transferred == 0 && result.multiplicity == Multiplicity::Affine {
            discards.push(result.binding_ordinal);
        }
    }
    Ok(discards)
}
