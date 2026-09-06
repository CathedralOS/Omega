//! Frozen cases and reference slots cannot survive untracked replacement.
//! Payload/referent writes do not replace their containing binding. This is an
//! opacity fence, not a source of access permission or replacement identity.

use super::{StoredLocalOrigins, projections::prefix_matches};
use crate::calls::write_frames::{FrameSourcePlace, coarse_place_path, split_place_root};
use facts::PlaceSegment;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::TableAssignment;

pub(in crate::calls::write_frames) fn statement_exposes_frozen_binding(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement: &typed_trees::statement::StatementNode,
    stored: &[StoredLocalOrigins],
) -> bool {
    use crate::calls::write_frames::statement_value_expression_roots;
    use typed_trees::statement::StatementNode;
    if let StatementNode::Call(call) = statement {
        return call_exposes_frozen_binding(program, machine, state, call, stored);
    }
    statement_value_expression_roots(program, statement)
        .into_iter()
        .any(|expression| {
            expression_exposes_frozen_binding(program, machine, state, expression, stored)
        })
}

pub(in crate::calls::write_frames) fn call_exposes_frozen_binding(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
    stored: &[StoredLocalOrigins],
) -> bool {
    (receiver_allows_replacement(program, call.target_symbol)
        && statement_receiver_exposes_binding(program, state, call, stored))
        || program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .any(|expression| {
                expression_exposes_frozen_binding(program, machine, state, *expression, stored)
            })
}

pub(in crate::calls::write_frames) fn expression_exposes_frozen_binding(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    stored: &[StoredLocalOrigins],
) -> bool {
    use crate::calls::write_frames::caller_aliases::expression_any;
    use typed_trees::expression::ExpressionNode;
    super::expression_borrows_carrier_binding(program, machine, state, expression, stored)
        || expression_any(program, expression, |expression| {
            matches!(program.expression_table.expression(expression),
                    ExpressionNode::Call(call) if receiver_allows_replacement(program, call.target_symbol)
                        && (target_replaces_case_binding(program, call.receiver, stored)
                            || target_replaces_reference_binding(program, call.receiver, stored, false)))
        })
}

fn receiver_allows_replacement(program: &TypedTrees, target: symbols::SymbolHandle) -> bool {
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
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                return access.is_exclusive();
            }
            _ => return false,
        }
    }
}

fn statement_receiver_exposes_binding(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
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
                && matches!(
                    declaration.kind,
                    symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                )
                && program.symbols.name(local.local_symbol) == root_name.as_str()
        })
        .any(|local| {
            if members.len() == 1 && call.receiver_symbol != local.local_symbol {
                // Unresolved or substituted receiver identity cannot recover a
                // permissive spelling fallback for a case-bearing local.
                return !local.cases.is_empty() || !local.references.is_empty();
            }
            let cases = local
                .cases
                .iter()
                .filter_map(|case| match case.split_last() {
                    Some((PlaceSegment::Case { .. }, container)) => Some((container, true)),
                    _ => None,
                });
            let references = local
                .references
                .iter()
                .map(|leaf| (leaf.local_segments.as_slice(), false));
            cases
                .chain(references)
                .any(|(container, include_endpoint)| {
                    let fields = container
                        .iter()
                        .filter_map(|segment| match segment {
                            PlaceSegment::Field { symbol } => Some(program.symbols.name(*symbol)),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    members.len() - 1 <= fields.len()
                        && (include_endpoint || members.len() - 1 < fields.len())
                        && members[1..]
                            .iter()
                            .zip(fields)
                            .all(|(member, field)| member.as_str() == field)
                })
        })
}

/// An implicit method receiver at a reference leaf borrows the referent, not
/// the slot. Explicit exclusive borrows can expose the slot itself as well.
pub(super) fn target_replaces_reference_binding(
    program: &TypedTrees,
    target: ExpressionHandle,
    stored: &[StoredLocalOrigins],
    include_endpoint: bool,
) -> bool {
    let source = FrameSourcePlace::from_expression(program, target);
    if !source.root.is_valid() {
        return coarse_place_path(program, target).is_some_and(|path| {
            let (root, _) = split_place_root(&path);
            stored.iter().any(|local| {
                !local.references.is_empty() && program.symbols.name(local.local_symbol) == root
            })
        });
    }
    stored
        .iter()
        .filter(|local| local.local_symbol == source.root)
        .any(|local| {
            local.references.iter().any(|leaf| {
                prefix_matches(&source.segments, &leaf.local_segments)
                    && (include_endpoint || source.segments.len() < leaf.local_segments.len())
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
