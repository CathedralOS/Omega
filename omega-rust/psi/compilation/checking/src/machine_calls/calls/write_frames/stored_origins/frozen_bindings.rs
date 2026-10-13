//! Frozen cases and reference slots cannot survive untracked replacement.
//! Payload/referent writes do not replace their containing binding. This is an
//! opacity fence, not a source of access permission or replacement identity.

use super::{
    FramePathPrecision, FramePlaceOrigin, StoredLocalOrigins, projections::prefix_matches,
};
use crate::machine_calls::calls::write_frames::{
    FrameInference, FrameSourcePlace, TopLevelSymbols, coarse_place_path, split_place_root,
};
use facts::PlaceSegment;
use symbols::JoinedPath;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::TableAssignment;

pub(in crate::machine_calls::calls::write_frames) fn statement_exposes_frozen_binding(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement: &typed_trees::statement::StatementNode,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> bool {
    use typed_trees::statement::StatementRootRole;

    if let Some(call) = statement.as_call() {
        return call_exposes_frozen_binding(
            program, machine, state, call, stored, aliases, symbols, inference,
        );
    }
    // Producing a value (including a returned descriptor) does not invoke the
    // authority it carries. Declaration and result transfers retain its
    // backing for later consumers; those consumers own their write effects.
    // Nested calls still contribute their own frames below; an unknown call
    // cannot hide behind this empty consumer effect.
    program
        .statement_table
        .expression_roots(statement)
        .into_iter()
        .any(|(role, expression)| {
            // A named transition passes authority into another activation;
            // a returned value or observed guard does not. Keep each root's
            // consumer instead of flattening the whole transition to one
            // unknown effect ceiling. Assignment targets retain that ceiling;
            // their separate store transfer owns the destination replacement.
            let consumer_writes: Option<&[String]> = match role {
                StatementRootRole::LocalInitializer
                | StatementRootRole::AssignmentValue
                | StatementRootRole::Expression
                | StatementRootRole::TransitionGuard
                | StatementRootRole::TransitionValue => Some(&[]),
                _ => None,
            };
            expression_exposes(
                program,
                machine,
                state,
                expression,
                stored,
                aliases,
                symbols,
                inference,
                consumer_writes,
            )
        })
}

pub(in crate::machine_calls::calls::write_frames) fn call_exposes_frozen_binding(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> bool {
    let arguments = program.statement_table.expression_handles(call.arguments);
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .joined_path_with(".");
    let receiver = (!receiver.is_empty()).then_some(FramePlaceOrigin {
        path: receiver,
        precision: FramePathPrecision::Exact,
        source: Default::default(),
    });
    let writes = call_written_paths(
        program,
        machine,
        call.target_symbol,
        receiver.as_ref(),
        arguments,
        symbols,
        inference,
    );
    (receiver_target_is_unresolved(program, call.target_symbol)
        && statement_receiver_exposes_binding(program, state, call, stored, aliases)
        && (!statement_receiver_has_identity(program, call, stored, aliases)
            || writes_may_replace_binding(program, stored, aliases, writes.as_deref())))
        || arguments.iter().any(|expression| {
            expression_exposes(
                program,
                machine,
                state,
                *expression,
                stored,
                aliases,
                symbols,
                inference,
                writes.as_deref(),
            )
        })
}

pub(in crate::machine_calls::calls::write_frames) fn expression_exposes_frozen_binding(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> bool {
    expression_exposes(
        program, machine, state, expression, stored, aliases, symbols, inference, None,
    )
}

/// A borrow grants an access ceiling; a call's complete frame says which of
/// that storage it may actually replace. Traverse call operands in that call's
/// context, never flatten nested calls into their enclosing call's ceiling.
fn expression_exposes(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    writes: Option<&[String]>,
) -> bool {
    use typed_trees::expression::ExpressionNode;
    if !expression.is_valid() {
        return false;
    }
    if let Some(call) = program.expression_table.call_node(expression) {
        let arguments = program.expression_table.expression_handles(call.arguments);
        let receiver = super::super::frame_place_path(program, call.receiver);
        let mut call_writes = call_written_paths(
            program,
            machine,
            call.target_symbol,
            receiver.as_ref(),
            arguments,
            symbols,
            inference,
        );
        // A returned reference or reference-bearing value can hand the same
        // replacement authority to its consumer. Scalar/Unit results cannot.
        // Keep both invocations' effects rather than hiding an opaque outer
        // call behind a pure identity helper.
        let carries_authority = super::super::machine_state_by_symbol(program, call.target_symbol)
            .is_none_or(|(_, callee)| {
                callee.return_type.is_valid()
                    && super::super::type_may_carry_write(program, callee.return_type)
            });
        if carries_authority {
            call_writes = match (call_writes, writes) {
                (Some(mut own), Some(consumer)) => {
                    own.extend_from_slice(consumer);
                    Some(own)
                }
                _ => None,
            };
        }
        let writes = call_writes.as_deref();
        if receiver_target_is_unresolved(program, call.target_symbol)
            && (target_replaces_case_binding(program, call.receiver, stored, aliases)
                || target_replaces_reference_binding(
                    program,
                    call.receiver,
                    stored,
                    aliases,
                    false,
                ))
            && (binding_source(program, call.receiver, aliases)
                .is_none_or(|source| !source.root.is_valid())
                || writes_may_replace_binding(program, stored, aliases, writes))
        {
            return true;
        }
        return std::iter::once(call.receiver)
            .chain(arguments.iter().copied())
            .any(|child| {
                expression_exposes(
                    program, machine, state, child, stored, aliases, symbols, inference, writes,
                )
            });
    }
    let mut children = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            if borrow.access.is_exclusive()
                && super::borrow_target_exposes_carrier_binding(
                    program,
                    machine,
                    state,
                    borrow.target,
                    stored,
                    aliases,
                )
                && (binding_source(program, borrow.target, aliases)
                    .is_none_or(|source| !source.root.is_valid())
                    || writes_may_replace_binding(program, stored, aliases, writes))
            {
                return true;
            }
            children.push(borrow.target);
        }
        ExpressionNode::Call(_) => {}
        other => crate::expression_traversal::children(program, other, |child| {
            children.push(child);
        }),
    }
    children.into_iter().any(|child| {
        expression_exposes(
            program, machine, state, child, stored, aliases, symbols, inference, writes,
        )
    })
}

/// A complete may-write frame can prove disjointness from frozen slots, but
/// cannot supply a replacement's new origin. Alias substitution happens before
/// comparing paths, and a coarse collection path retains its whole ceiling.
fn writes_may_replace_binding(
    program: &dyn crate::CheckingInput,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    writes: Option<&[String]>,
) -> bool {
    let Some(writes) = writes else {
        return true;
    };
    writes.iter().any(|path| {
        let (root, _) = split_place_root(path);
        if aliases
            .iter()
            .any(|(name, origin)| name == root && !origin.source.root.is_valid())
        {
            return true;
        }
        let path = super::super::rebase_local_alias_path(program, path, aliases);
        let (root, suffix) = split_place_root(&path);
        // Incoming by-value carriers can have an exposure ceiling without a
        // retained local inventory in this query. Missing inventory cannot
        // prove that a complete call's writes are disjoint from their slots.
        if !stored
            .iter()
            .any(|local| program.symbols.name(local.local_symbol) == root)
        {
            return true;
        }
        stored
            .iter()
            .filter(|local| program.symbols.name(local.local_symbol) == root)
            .any(|local| {
                let mut frozen = local
                    .references
                    .iter()
                    .map(|leaf| leaf.local_segments.as_slice())
                    .chain(
                        local
                            .cases
                            .iter()
                            .filter_map(|case| match case.split_last() {
                                Some((PlaceSegment::Case { .. }, container)) => Some(container),
                                _ => None,
                            }),
                    );
                frozen.any(|segments| {
                    let mut binding = root.to_owned();
                    for segment in segments {
                        if let PlaceSegment::Field { symbol } = segment {
                            binding.push('.');
                            binding.push_str(program.symbols.name(*symbol));
                        }
                    }
                    // Collection selectors in the may-write vocabulary are coarse;
                    // never interpret them as proof that a frozen slot is disjoint.
                    let written = suffix
                        .find('[')
                        .map_or(path.as_str(), |offset| &path[..root.len() + offset]);
                    super::place_suffix(written, &binding).is_some()
                })
            })
    })
}

/// An exclusive operand grants a ceiling, not evidence that every slot is
/// replaced. Use the same complete callee frame as ordinary effect checking.
/// A write at or above a frozen slot may replace it; disjoint scalar writes
/// preserve its identity. Opaque, external and recursive effects retain the
/// full ceiling. These paths conservatively include both slot and referent
/// writes, so they can prove disjointness but never a replacement's new origin.
fn call_written_paths(
    program: &dyn crate::CheckingInput,
    caller: &typed_trees::machine::Machine,
    target: symbols::SymbolHandle,
    receiver: Option<&FramePlaceOrigin>,
    arguments: &[ExpressionHandle],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let (machine, state) = super::super::machine_state_by_symbol(program, target)?;
    if !machine.supply_mode.is_checked_body()
        || !machine.body_is_present
        || inference.active_states.contains(&state.symbol)
    {
        return None;
    }
    inference.active_states.push(state.symbol);
    let written = super::super::state_write_walk::summarize_complete_state_written_paths(
        program, machine, state, symbols, inference,
    );
    inference.active_states.pop();
    let mut paths = Vec::new();
    for relative in written? {
        paths.extend(
            super::super::path_instantiation::instantiate_written_path_for_origins(
                program,
                caller,
                &relative,
                receiver,
                program.state_parameters(state),
                arguments,
                &[],
                symbols,
                inference,
                None,
            )?,
        );
    }
    Some(paths)
}

fn receiver_target_is_unresolved(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> bool {
    crate::machine_calls::calls::write_frames::machine_state_by_symbol(program, target).is_none()
}

fn statement_receiver_has_identity(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::statement::TableCall,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    let members = program.statement_table.name_path_members(call.receiver);
    let Some(root) = members.first() else {
        return false;
    };
    if let Some((_, origin)) = aliases.iter().find(|(name, _)| name == root.as_str()) {
        return origin.precision == FramePathPrecision::Exact && origin.source.root.is_valid();
    }
    members.len() != 1
        || stored
            .iter()
            .filter(|local| program.symbols.name(local.local_symbol) == root.as_str())
            .all(|local| call.receiver_symbol == local.local_symbol)
}

fn statement_receiver_exposes_binding(
    program: &dyn crate::CheckingInput,
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
            receiver_contains_binding(program, local, &receiver_fields)
        })
}

fn receiver_contains_binding(
    program: &dyn crate::CheckingInput,
    local: &StoredLocalOrigins,
    receiver_fields: &[&str],
) -> bool {
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
                    .zip(fields.iter().copied())
                    .all(|(member, field)| member == field)
        })
}

/// An implicit method receiver at a reference leaf borrows the referent, not
/// the slot. Explicit exclusive borrows can expose the slot itself as well.
pub(super) fn target_replaces_reference_binding(
    program: &dyn crate::CheckingInput,
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

pub(in crate::machine_calls::calls::write_frames) fn assignment_replaces_case_binding(
    program: &dyn crate::CheckingInput,
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
pub(in crate::machine_calls::calls::write_frames) fn assignment_replaces_reference_ancestor(
    program: &dyn crate::CheckingInput,
    assignment: &TableAssignment,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    target_replaces_reference_binding(program, assignment.target, stored, aliases, false)
}

pub(super) fn target_replaces_case_binding(
    program: &dyn crate::CheckingInput,
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
pub(in crate::machine_calls::calls::write_frames) fn binding_source(
    program: &dyn crate::CheckingInput,
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
