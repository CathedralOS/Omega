//! Caller-prefix alias closure for public write-frame demand.
//!
//! The state walker owns alias transfer. This leaf locates a query's statement
//! and closes its writes over the canonical storage paths and live local names
//! used by fact consumers. It never publishes prefix writes as call writes.

use super::stored_origins::{StoredLocalOrigins, expand_write_path, place_suffix};
use super::{
    ExpressionHandle, ExpressionNode, FrameInference, FramePathPrecision, FramePlaceOrigin,
    Machine, StatementNode, SymbolHandle, TableCall, TopLevelSymbols, TypedTrees,
    append_place_suffix, statement_value_expression_roots, type_is_caller_isolated_local,
    type_may_carry_write,
};
use crate::machine_calls::calls::write_frames::state_write_walk::{
    StateWriteQuery, walk_state_write_prefix,
};
use typed_trees::signature::StateParameter;

pub(super) fn caller_binding_type(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<super::TypeReferenceHandle> {
    let ExpressionNode::Name(name) = program.expression_table.expression(argument) else {
        return None;
    };
    let [_] = program.expression_table.name_path_members(name.members) else {
        return None;
    };
    caller_name_root_type(program, current_machine, argument)
}

/// Validate the root declaration of a retained name path without treating a
/// projected member as a standalone reference binding.
pub(super) fn caller_name_root_type(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<super::TypeReferenceHandle> {
    let ExpressionNode::Name(name) = program.expression_table.expression(argument) else {
        return None;
    };
    let members = program.expression_table.name_path_members(name.members);
    let member = members.first()?;
    let root = name.head_symbol;
    if !root.is_valid() || (members.len() == 1 && root != name.symbol) {
        return None;
    }
    let (state, _, index) = caller_statement_at_site(
        program,
        current_machine,
        CallerWriteSite::Expression(argument),
    )?;
    let declaration = program.symbols.get(root);
    // Typed `self` paths retain the owning machine identity, not the synthetic
    // state parameter identity. Only that exact machine may select this state's
    // unique receiver declaration.
    if member.as_str() == "self"
        && root == current_machine.symbol
        && declaration.kind == symbols::SymbolKind::Machine
    {
        let mut receivers = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.is_self);
        let receiver = receivers.next()?;
        return (receivers.next().is_none() && receiver.type_reference.is_valid())
            .then_some(receiver.type_reference);
    }
    if declaration.parent != state.symbol || program.symbols.name(root) != member.as_str() {
        return None;
    }
    let reference = match declaration.kind {
        symbols::SymbolKind::Parameter => {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == root)?
                .type_reference
        }
        symbols::SymbolKind::Local => {
            let local = program.statement_table.statements(state.statement_nodes)[..index]
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == root => Some(local),
                    _ => None,
                })?;
            local.type_reference
        }
        _ => return None,
    };
    reference.is_valid().then_some(reference)
}

#[derive(Clone, Copy)]
pub(crate) enum CallerWriteSite<'query> {
    Call(&'query TableCall),
    Statement(&'query StatementNode),
    Expression(ExpressionHandle),
}

/// Transient caller-prefix evidence for projecting a local reference into
/// another place representation. Coarse storage paths cannot acquire a field
/// or index suffix when the consumer transports a write through this origin.
pub struct LocalWriteOrigin {
    pub local_symbol: SymbolHandle,
    /// Empty for a bare reference; otherwise the exact reference leaf selector.
    /// A type-derived array leaf uses Index with a zero expression handle to
    /// denote any element. This is may-write evidence, never access authority.
    pub local_segments: Vec<facts::PlaceSegment>,
    pub source_path: String,
    pub collection_coarse: bool,
    /// Structural selectors retained before string-path coarsening. Consumers
    /// must validate the declaration path; runtime selectors are not snapshots.
    /// A zero root withholds primitive-coordinate precision without removing
    /// the coarse source path.
    pub source_root: SymbolHandle,
    pub source_segments: Vec<facts::PlaceSegment>,
}

/// The direct assignment effect, excluding calls evaluated in its operands.
/// A binding replacement changes the local slot, not its previous referent.
pub enum AssignmentWriteTarget {
    LocalBindingReplacement { path: String },
    Storage { paths: Vec<String> },
}

struct AssignmentEvidence {
    target: AssignmentWriteTarget,
    aliases: Vec<(String, FramePlaceOrigin)>,
    divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    stored: Vec<StoredLocalOrigins>,
}

pub(super) fn assignment_write_target(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<AssignmentWriteTarget> {
    assignment_evidence(program, machine, symbols, statement).map(|evidence| evidence.target)
}

pub(super) fn assignment_write_paths(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<Vec<String>> {
    let evidence = assignment_evidence(program, machine, symbols, statement)?;
    match evidence.target {
        AssignmentWriteTarget::LocalBindingReplacement { path } => Some(vec![path]),
        AssignmentWriteTarget::Storage { paths } => Some(close_over_origins(
            paths,
            &evidence.aliases,
            &evidence.divergent,
            &evidence.stored,
        )),
    }
}

fn assignment_evidence(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<AssignmentEvidence> {
    let StatementNode::Assignment(assignment) = statement else {
        return None;
    };
    let site = caller_prefix_site(program, machine, CallerWriteSite::Statement(statement))?;
    if matches!(site, CallerPrefixSite::Untracked)
        && let Some(path) = super::coarse_place_path(program, assignment.target)
    {
        return Some(AssignmentEvidence {
            target: AssignmentWriteTarget::Storage { paths: vec![path] },
            aliases: Vec::new(),
            divergent: Vec::new(),
            stored: Vec::new(),
        });
    }
    let state = match site {
        CallerPrefixSite::Tracked { state, .. } => state,
        CallerPrefixSite::Untracked => program.machine_states(machine).iter().find(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|candidate| std::ptr::eq(statement, candidate))
        })?,
    };
    // One exact prefix supplies both the target and alias closure. Storage
    // writes leave the prefix origins intact; binding replacements return only
    // their slot and must not be closed over the slot's former referent.
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        &mut FrameInference::default(),
        &mut Vec::new(),
        Some(StateWriteQuery::Assignment(statement)),
    )?;
    Some(AssignmentEvidence {
        target: prefix.assignment?,
        aliases: prefix.aliases,
        divergent: prefix.divergent,
        stored: prefix.stored,
    })
}

pub(super) fn local_write_origins_before_statement(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<Vec<LocalWriteOrigin>> {
    let site = caller_prefix_site(program, machine, CallerWriteSite::Statement(statement))?;
    let evidence = caller_aliases_at_prefix(program, machine, symbols, site)?;
    let mut origins = evidence
        .stored
        .into_iter()
        .flat_map(|local| local.references)
        .map(|leaf| LocalWriteOrigin {
            local_symbol: leaf.local_symbol,
            local_segments: leaf.local_segments,
            source_path: leaf.origin.path,
            collection_coarse: leaf.origin.precision == FramePathPrecision::CollectionCoarse,
            source_root: if leaf.origin.source.builtin_coordinates {
                leaf.origin.source.root
            } else {
                SymbolHandle::invalid()
            },
            source_segments: leaf.origin.source.segments,
        })
        .collect::<Vec<_>>();
    if evidence.aliases.is_empty() {
        return Some(origins);
    }
    let CallerPrefixSite::Tracked { state, index, .. } = site else {
        return None;
    };
    let preceding = &program.statement_table.statements(state.statement_nodes)[..index];
    let aliases = evidence
        .aliases
        .into_iter()
        .map(|(name, origin)| {
            let local_symbol = preceding
                .iter()
                .find_map(|candidate| match candidate {
                    StatementNode::LocalData(local) if local.name.as_str() == name => {
                        Some(local.symbol)
                    }
                    _ => None,
                })
                .filter(|symbol| {
                    symbol.is_valid()
                        && program.symbols.get(*symbol).parent == state.symbol
                        && matches!(
                            program.symbols.get(*symbol).kind,
                            symbols::SymbolKind::Local
                        )
                })?;
            Some(LocalWriteOrigin {
                local_symbol,
                local_segments: Vec::new(),
                source_path: origin.path,
                collection_coarse: origin.precision == FramePathPrecision::CollectionCoarse,
                source_root: if origin.source.builtin_coordinates {
                    origin.source.root
                } else {
                    SymbolHandle::invalid()
                },
                source_segments: origin.source.segments,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    origins.extend(aliases);
    Some(origins)
}

#[cfg(test)]
fn close_caller_aliases(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    written: Vec<String>,
) -> Option<Vec<String>> {
    if written.is_empty() {
        return Some(written);
    }
    let evidence = caller_aliases_at_site(program, machine, symbols, site)?;
    Some(close_over_origins(
        written,
        &evidence.aliases,
        &evidence.divergent,
        &evidence.stored,
    ))
}

#[cfg(test)]
mod tests;

/// Alias maps a boundary resolve consults when it spells call-argument
/// origins: the enclosing state's parameters, the prefix's established
/// single-origin bindings and divergent referent sets, and its stored
/// carriers. Every slice is empty for an untracked site.
pub(super) struct StatementCallPrefix<'prefix> {
    pub parameters: &'prefix [StateParameter],
    pub aliases: &'prefix [(String, FramePlaceOrigin)],
    pub divergent: &'prefix [(String, Vec<FramePlaceOrigin>)],
    pub stored: &'prefix [StoredLocalOrigins],
}

/// Freeze the caller prefix once before resolving a demand, then use exactly
/// that evidence for both contextual case selection and storage closure.
pub(super) fn with_caller_origins(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    resolve: impl FnOnce(&mut FrameInference, &StatementCallPrefix<'_>) -> Option<Vec<String>>,
) -> Option<Vec<String>> {
    let evidence = caller_aliases_at_site(program, machine, symbols, site)?;
    let mut inference = FrameInference::default();
    for local in &evidence.stored {
        inference.record_local(local);
    }
    let prefix = StatementCallPrefix {
        parameters: evidence
            .state
            .map(|state| program.state_parameters(state))
            .unwrap_or(&[]),
        aliases: &evidence.aliases,
        divergent: &evidence.divergent,
        stored: &evidence.stored,
    };
    let written = resolve(&mut inference, &prefix)?;
    Some(close_over_origins(
        written,
        &evidence.aliases,
        &evidence.divergent,
        &evidence.stored,
    ))
}

fn close_over_origins(
    written: Vec<String>,
    aliases: &[(String, FramePlaceOrigin)],
    divergent: &[(String, Vec<FramePlaceOrigin>)],
    stored: &[StoredLocalOrigins],
) -> Vec<String> {
    let canonical = written
        .iter()
        .flat_map(|path| expand_write_path(path, aliases, stored))
        // A divergent binding's raw spelling names every referent it may
        // carry: keep the local name for fact invalidation and add each
        // candidate route, composing a projected suffix onto exact
        // candidates while a coarse candidate already covers its whole
        // storage root.
        .chain(written.iter().flat_map(|path| {
            divergent
                .iter()
                .filter(|(name, _)| place_suffix(name, path).is_some())
                .flat_map(|(name, candidates)| {
                    let suffix = place_suffix(name, path).unwrap_or_default().to_owned();
                    candidates
                        .iter()
                        .map(|candidate| {
                            if suffix.is_empty()
                                || candidate.precision == FramePathPrecision::CollectionCoarse
                            {
                                candidate.path.clone()
                            } else {
                                append_place_suffix(&candidate.path, &suffix)
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        }))
        .collect::<Vec<_>>();
    let mut closed = written;
    for path in canonical {
        if !closed.contains(&path) {
            closed.push(path.clone());
        }
        for (alias, origin, local_coarse) in aliases
            .iter()
            .map(|(alias, origin)| (alias, origin, false))
            .chain(
                stored
                    .iter()
                    .flat_map(|local| &local.references)
                    .map(|leaf| {
                        (
                            &leaf.local_path,
                            &leaf.origin,
                            leaf.local_segments.iter().any(|segment| {
                                matches!(
                                    segment,
                                    facts::PlaceSegment::FixedIndex { .. }
                                        | facts::PlaceSegment::Index { .. }
                                )
                            }),
                        )
                    }),
            )
        {
            let spelling = if let Some(suffix) = place_suffix(&origin.path, &path) {
                match (origin.precision, local_coarse) {
                    (FramePathPrecision::Exact, false) => append_place_suffix(alias, suffix),
                    _ => alias.clone(),
                }
            } else if place_suffix(&path, &origin.path).is_some() {
                alias.clone()
            } else {
                continue;
            };
            if !closed.contains(&spelling) {
                closed.push(spelling);
            }
        }
    }
    closed
}

/// The prefix evidence a boundary site established: the enclosing state plus
/// the alias, divergent-referent, and stored-carrier maps the write walk
/// produced before the boundary statement.
struct CallerPrefixEvidence<'program> {
    state: Option<&'program typed_trees::state::State>,
    aliases: Vec<(String, FramePlaceOrigin)>,
    divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    stored: Vec<StoredLocalOrigins>,
}

fn caller_aliases_at_site<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
) -> Option<CallerPrefixEvidence<'program>> {
    let site = caller_prefix_site(program, machine, site)?;
    caller_aliases_at_prefix(program, machine, symbols, site)
}

#[derive(Clone, Copy)]
enum CallerPrefixSite<'program> {
    Untracked,
    Tracked {
        state: &'program typed_trees::state::State,
        statement: &'program StatementNode,
        index: usize,
    },
}

fn caller_prefix_site<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    site: CallerWriteSite<'_>,
) -> Option<CallerPrefixSite<'program>> {
    let may_declare_origins = |statement: &StatementNode| {
        matches!(statement, StatementNode::LocalData(local)
            if super::stored_origins::has_aggregate_case_shape(program, local.type_reference)
                || (type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)))
    };
    let has_incoming_carrier = |state: &typed_trees::state::State| {
        program.state_parameters(state).iter().any(|parameter| {
            !super::type_reference_is_reference(program, parameter.type_reference)
                && type_may_carry_write(program, parameter.type_reference)
                && !type_is_caller_isolated_local(program, parameter.type_reference)
        })
    };
    if !program.machine_states(machine).iter().any(|state| {
        has_incoming_carrier(state)
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(may_declare_origins)
    }) {
        return Some(CallerPrefixSite::Untracked);
    }
    let (state, statement, index) = caller_statement_at_site(program, machine, site)?;
    if !has_incoming_carrier(state)
        && !program.statement_table.statements(state.statement_nodes)[..index]
            .iter()
            .any(may_declare_origins)
    {
        return Some(CallerPrefixSite::Untracked);
    }
    Some(CallerPrefixSite::Tracked {
        state,
        statement,
        index,
    })
}

fn caller_aliases_at_prefix<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerPrefixSite<'program>,
) -> Option<CallerPrefixEvidence<'program>> {
    let CallerPrefixSite::Tracked {
        state, statement, ..
    } = site
    else {
        return Some(CallerPrefixEvidence {
            state: None,
            aliases: Vec::new(),
            divergent: Vec::new(),
            stored: Vec::new(),
        });
    };
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        &mut FrameInference::default(),
        &mut Vec::new(),
        Some(StateWriteQuery::Before(statement)),
    )?;
    if super::stored_origins::statement_exposes_frozen_binding(
        program,
        machine,
        state,
        statement,
        &prefix.stored,
        &prefix.aliases,
    ) {
        return None;
    }
    // A divergent binding's referent set cannot be spelled through the
    // single-origin alias map: a member-read actual, nested call mention, or
    // transport that touches it at this boundary must stay opaque rather than
    // frame the bare local alone. A statement call that lends the whole
    // referent set through a direct exclusive reborrow argument (or passes
    // the binding itself) is admitted; the resolve closure instantiates the
    // callee's writes through that candidate set. Result-relation queries use
    // `Before` directly and resolve the returned name through its own
    // initializer route instead.
    let divergent_roots = prefix
        .divergent
        .iter()
        .map(|(name, _)| name.as_str().to_owned())
        .collect::<Vec<_>>();
    if !divergent_roots.is_empty()
        && super::local_aliases::statement_mentions_place_roots(
            program,
            statement,
            &divergent_roots,
        )
        && !matches!(
            statement,
            StatementNode::Call(call)
                if super::local_aliases::call_mentions_divergent_roots_only_through_reborrow_arguments(
                    program,
                    call,
                    &divergent_roots,
                )
        )
    {
        return None;
    }
    Some(CallerPrefixEvidence {
        state: Some(state),
        aliases: prefix.aliases,
        divergent: prefix.divergent,
        stored: prefix.stored,
    })
}

/// Locate a unique retained occurrence without resolving declarations by name.
pub(super) fn caller_statement_at_site<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    site: CallerWriteSite<'_>,
) -> Option<(
    &'program typed_trees::state::State,
    &'program StatementNode,
    usize,
)> {
    let mut owner = None;
    for state in program.machine_states(machine) {
        let statements = program.statement_table.statements(state.statement_nodes);
        for (index, statement) in statements.iter().enumerate() {
            let matches = match site {
                CallerWriteSite::Call(call) => {
                    matches!(statement, StatementNode::Call(candidate) if std::ptr::eq(call, candidate))
                }
                CallerWriteSite::Statement(candidate) => std::ptr::eq(statement, candidate),
                CallerWriteSite::Expression(expression) => {
                    statement_value_expression_roots(program, statement)
                        .into_iter()
                        .any(|root| contains_expression(program, root, expression))
                }
            };
            if matches {
                if owner.is_some() {
                    return None;
                }
                owner = Some((state, statement, index));
            }
        }
    }
    owner
}

fn contains_expression(
    program: &TypedTrees,
    root: ExpressionHandle,
    target: ExpressionHandle,
) -> bool {
    expression_any(program, root, |expression| expression == target)
}

pub(super) fn expression_has_calls(program: &TypedTrees, root: ExpressionHandle) -> bool {
    expression_any(program, root, |expression| {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        )
    })
}

pub(super) fn expression_any(
    program: &TypedTrees,
    root: ExpressionHandle,
    mut predicate: impl FnMut(ExpressionHandle) -> bool,
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if predicate(expression) {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for arm in program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .rev()
                {
                    pending.push(arm.value);
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            ExpressionNode::Atomic(atomic) => pending.push(atomic.value),
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    false
}
