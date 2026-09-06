//! Frozen cases and reference slots cannot survive untracked replacement.
//! Payload/referent writes do not replace their containing binding. This is an
//! opacity fence, not a source of access permission or replacement identity.

use super::{
    FramePathPrecision, FramePlaceOrigin, StoredLocalOrigins, projections::prefix_matches,
};
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
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    use crate::calls::write_frames::statement_value_expression_roots;
    use typed_trees::statement::StatementNode;
    if let StatementNode::Call(call) = statement {
        return call_exposes_frozen_binding(program, machine, state, call, stored, aliases);
    }
    statement_value_expression_roots(program, statement)
        .into_iter()
        .any(|expression| {
            expression_exposes_frozen_binding(program, machine, state, expression, stored, aliases)
        })
}

pub(in crate::calls::write_frames) fn call_exposes_frozen_binding(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    (receiver_allows_replacement(program, call.target_symbol)
        && statement_receiver_exposes_binding(program, state, call, stored, aliases))
        || program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .any(|expression| {
                expression_exposes_frozen_binding(
                    program,
                    machine,
                    state,
                    *expression,
                    stored,
                    aliases,
                )
            })
}

pub(in crate::calls::write_frames) fn expression_exposes_frozen_binding(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    use crate::calls::write_frames::caller_aliases::expression_any;
    use typed_trees::expression::ExpressionNode;
    super::expression_borrows_carrier_binding(program, machine, state, expression, stored, aliases)
        || expression_any(program, expression, |expression| {
            matches!(program.expression_table.expression(expression),
                    ExpressionNode::Call(call) if receiver_allows_replacement(program, call.target_symbol)
                        && (target_replaces_case_binding(program, call.receiver, stored, aliases)
                            || target_replaces_reference_binding(program, call.receiver, stored, aliases, false)))
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
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    let members = program.statement_table.name_path_members(call.receiver);
    let Some(root_name) = members.first() else {
        return false;
    };
    let alias = aliases.iter().find(|(name, _)| name == root_name.as_str());
    if alias.is_some_and(|(_, origin)| {
        origin.precision != FramePathPrecision::Exact || !origin.source.root.is_valid()
    }) {
        return stored
            .iter()
            .any(|local| !local.references.is_empty() || !local.cases.is_empty());
    }
    let prefix = alias
        .map(|(_, origin)| origin.source.segments.as_slice())
        .unwrap_or(&[]);
    let prefix_fields = prefix
        .iter()
        .filter_map(|segment| match segment {
            PlaceSegment::Field { symbol } => Some(program.symbols.name(*symbol)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let receiver_fields = prefix_fields
        .into_iter()
        .chain(members[1..].iter().map(|member| member.as_str()))
        .collect::<Vec<_>>();
    stored
        .iter()
        .filter(|local| {
            if let Some((_, origin)) = alias {
                return local.local_symbol == origin.source.root;
            }
            let declaration = program.symbols.get(local.local_symbol);
            declaration.parent == state.symbol
                && matches!(
                    declaration.kind,
                    symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                )
                && program.symbols.name(local.local_symbol) == root_name.as_str()
        })
        .any(|local| {
            if alias.is_none() && members.len() == 1 && call.receiver_symbol != local.local_symbol {
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
                    receiver_fields.len() <= fields.len()
                        && (include_endpoint || receiver_fields.len() < fields.len())
                        && receiver_fields
                            .iter()
                            .copied()
                            .zip(fields)
                            .all(|(member, field)| member == field)
                })
        })
}

/// An implicit method receiver at a reference leaf borrows the referent, not
/// the slot. Explicit exclusive borrows can expose the slot itself as well.
pub(super) fn target_replaces_reference_binding(
    program: &TypedTrees,
    target: ExpressionHandle,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    include_endpoint: bool,
) -> bool {
    let Some(source) = binding_source(program, target, aliases) else {
        return stored.iter().any(|local| !local.references.is_empty());
    };
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
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    target_replaces_case_binding(program, assignment.target, stored, aliases)
}

/// Replacing a carrier through its root reference can overwrite its leaf
/// bindings without rebinding that root reference. Writes at the leaf itself
/// may instead replace referent contents; the ordinary assignment classifier
/// distinguishes those from reference-valued slot replacement.
pub(in crate::calls::write_frames) fn assignment_replaces_reference_ancestor(
    program: &TypedTrees,
    assignment: &TableAssignment,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    target_replaces_reference_binding(program, assignment.target, stored, aliases, false)
}

pub(super) fn target_replaces_case_binding(
    program: &TypedTrees,
    target: ExpressionHandle,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    let Some(source) = binding_source(program, target, aliases) else {
        return stored.iter().any(|local| !local.cases.is_empty());
    };
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

/// Alias origins were frozen when their bindings were established. Consult
/// those origins for interference, without replaying an initializer by name.
pub(in crate::calls::write_frames) fn binding_source(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
) -> Option<FrameSourcePlace> {
    let source = FrameSourcePlace::from_expression(program, expression);
    let Some((_, origin)) = aliases
        .iter()
        .find(|(name, _)| name == program.symbols.name(source.root))
    else {
        return Some(source);
    };
    (origin.precision == FramePathPrecision::Exact && origin.source.root.is_valid())
        .then(|| origin.source.append_source(&source))
}
