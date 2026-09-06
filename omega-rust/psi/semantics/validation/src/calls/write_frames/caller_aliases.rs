//! Caller-prefix alias closure for public write-frame demand.
//!
//! The state walker owns alias transfer. This leaf locates a query's statement
//! and closes its writes over the canonical storage paths and live local names
//! used by fact consumers. It never publishes prefix writes as call writes.

use super::stored_origins::{StoredLocalOrigins, expand_write_path, place_suffix};
use super::{
    ExpressionHandle, ExpressionNode, FrameInference, FramePathPrecision, FramePlaceOrigin,
    Machine, StateWriteQuery, StatementNode, SymbolHandle, TableCall, TopLevelSymbols, TypedTrees,
    append_place_suffix, statement_value_expression_roots, type_is_caller_isolated_local,
    type_may_carry_write, walk_state_write_prefix,
};

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
pub(super) enum CallerWriteSite<'query> {
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

pub(super) fn assignment_write_target(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<AssignmentWriteTarget> {
    let StatementNode::Assignment(assignment) = statement else {
        return None;
    };
    let (aliases, stored) = caller_aliases_at_site(
        program,
        machine,
        symbols,
        CallerWriteSite::Statement(statement),
    )?;
    if aliases.is_empty()
        && stored.is_empty()
        && let Some(path) = super::coarse_place_path(program, assignment.target)
    {
        return Some(AssignmentWriteTarget::Storage { paths: vec![path] });
    }
    let state = program.machine_states(machine).iter().find(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|candidate| std::ptr::eq(statement, candidate))
    })?;
    walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        &mut FrameInference::default(),
        &mut Vec::new(),
        Some(StateWriteQuery::Assignment(statement)),
    )?
    .assignment
}

pub(super) fn local_write_origins_before_statement(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<Vec<LocalWriteOrigin>> {
    let (aliases, stored) = caller_aliases_at_site(
        program,
        machine,
        symbols,
        CallerWriteSite::Statement(statement),
    )?;
    let mut origins = stored
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
    if aliases.is_empty() {
        return Some(origins);
    }
    let state = program.machine_states(machine).iter().find(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|candidate| std::ptr::eq(statement, candidate))
    })?;
    let aliases = aliases
        .into_iter()
        .map(|(name, origin)| {
            let local_symbol = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take_while(|candidate| !std::ptr::eq(statement, *candidate))
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

pub(super) fn close_caller_aliases(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    written: Vec<String>,
) -> Option<Vec<String>> {
    if written.is_empty() {
        return Some(written);
    }
    let (aliases, stored) = caller_aliases_at_site(program, machine, symbols, site)?;
    Some(close_over_origins(written, &aliases, &stored))
}

/// Freeze the caller prefix once before resolving a demand, then use exactly
/// that evidence for both contextual case selection and storage closure.
pub(super) fn with_caller_origins(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    resolve: impl FnOnce(&mut FrameInference) -> Option<Vec<String>>,
) -> Option<Vec<String>> {
    let (aliases, stored) = caller_aliases_at_site(program, machine, symbols, site)?;
    let mut inference = FrameInference::default();
    for local in &stored {
        inference.record_local(local);
    }
    let written = resolve(&mut inference)?;
    Some(close_over_origins(written, &aliases, &stored))
}

fn close_over_origins(
    written: Vec<String>,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<String> {
    let canonical = written
        .iter()
        .flat_map(|path| expand_write_path(path, aliases, stored))
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

fn caller_aliases_at_site(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
) -> Option<(Vec<(String, FramePlaceOrigin)>, Vec<StoredLocalOrigins>)> {
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
        return Some((Vec::new(), Vec::new()));
    }
    let (state, statement, index) = caller_statement_at_site(program, machine, site)?;
    if !has_incoming_carrier(state)
        && !program.statement_table.statements(state.statement_nodes)[..index]
            .iter()
            .any(may_declare_origins)
    {
        return Some((Vec::new(), Vec::new()));
    }
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
    Some((prefix.aliases, prefix.stored))
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
