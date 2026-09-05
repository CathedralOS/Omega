//! Frozen case evidence cannot survive replacement of its discriminant storage.
//! Payload scalar writes do not replace an enclosing case. This is an opacity
//! fence, not a mutable case-state transfer or a source of access permission.

use super::{StoredLocalOrigins, projections::prefix_matches};
use crate::calls::write_frames::{FrameSourcePlace, coarse_place_path, split_place_root};
use psi_facts::PlaceSegment;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::statement::TableAssignment;

pub(in crate::calls::write_frames) fn statement_exposes_frozen_binding(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement: &psi_typed_trees::statement::StatementNode,
    stored: &[StoredLocalOrigins],
) -> bool {
    use crate::calls::write_frames::{
        caller_aliases::expression_any, statement_value_expression_roots,
    };
    use psi_typed_trees::{expression::ExpressionNode, statement::StatementNode};
    if let StatementNode::Call(call) = statement
        && receiver_allows_replacement(program, call.target_symbol)
        && statement_receiver_replaces_case(program, state, call, stored)
    {
        return true;
    }
    statement_value_expression_roots(program, statement).into_iter().any(|expression| {
        super::expression_borrows_carrier_binding(program, machine, state, expression, stored)
            || expression_any(program, expression, |expression| {
                matches!(program.expression_table.expression(expression),
                    ExpressionNode::Call(call) if receiver_allows_replacement(program, call.target_symbol)
                        && target_replaces_case_binding(program, call.receiver, stored))
            })
    })
}

fn receiver_allows_replacement(program: &TypedTrees, target: psi_symbols::SymbolHandle) -> bool {
    let Some((_, state)) = crate::calls::write_frames::machine_state_by_symbol(program, target)
    else {
        return true;
    };
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
    else {
        return false;
    };
    let mut reference = parameter.type_reference;
    loop {
        match program.type_reference_table.type_reference(reference) {
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            psi_typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                return access.is_exclusive();
            }
            _ => return false,
        }
    }
}

fn statement_receiver_replaces_case(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
    stored: &[StoredLocalOrigins],
) -> bool {
    let members = program.statement_table.name_path_members(call.receiver);
    let Some(root_name) = members.first() else {
        return false;
    };
    stored
        .iter()
        .filter(|local| {
            let declaration = program.symbols.get(local.local_symbol);
            declaration.parent == state.symbol
                && declaration.kind == psi_symbols::SymbolKind::Local
                && program.symbols.name(local.local_symbol) == root_name.as_str()
        })
        .any(|local| {
            if members.len() == 1 && call.receiver_symbol != local.local_symbol {
                // Unresolved or substituted receiver identity cannot recover a
                // permissive spelling fallback for a case-bearing local.
                return !local.cases.is_empty();
            }
            local.cases.iter().any(|case| {
                let Some((PlaceSegment::Case { .. }, container)) = case.split_last() else {
                    return false;
                };
                let fields = container
                    .iter()
                    .filter_map(|segment| match segment {
                        PlaceSegment::Field { symbol } => Some(program.symbols.name(*symbol)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                members.len() - 1 <= fields.len()
                    && members[1..]
                        .iter()
                        .zip(fields)
                        .all(|(member, field)| member.as_str() == field)
            })
        })
}

pub(in crate::calls::write_frames) fn assignment_replaces_case_binding(
    program: &TypedTrees,
    assignment: &TableAssignment,
    stored: &[StoredLocalOrigins],
) -> bool {
    target_replaces_case_binding(program, assignment.target, stored)
}

pub(super) fn target_replaces_case_binding(
    program: &TypedTrees,
    target: ExpressionHandle,
    stored: &[StoredLocalOrigins],
) -> bool {
    let source = FrameSourcePlace::from_expression(program, target);
    if !source.root.is_valid() {
        // Failure to normalize a spelling already known to carry frozen cases
        // cannot make its replacement harmless.
        return coarse_place_path(program, target).is_some_and(|path| {
            let (root, _) = split_place_root(&path);
            stored.iter().any(|local| {
                !local.cases.is_empty() && program.symbols.name(local.local_symbol) == root
            })
        });
    }
    stored
        .iter()
        .filter(|local| local.local_symbol == source.root)
        .any(|local| {
            local.cases.iter().any(|case| {
                let Some((PlaceSegment::Case { .. }, container)) = case.split_last() else {
                    return false;
                };
                prefix_matches(&source.segments, container)
            })
        })
}
