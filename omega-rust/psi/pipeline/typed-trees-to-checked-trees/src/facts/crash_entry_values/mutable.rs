//! Whether a mutable binding's current storage still holds the value its
//! binding position established.
//!
//! A `mut` parameter is bound by the invocation (entry state) or by each
//! named transition edge into its state; a `let mut` local is bound by its
//! initializer. A read sees that bound snapshot only while no intervening
//! statement can overwrite the storage or lend it exclusive access. The scan
//! is deliberately syntactic and conservative: anything it cannot rule out
//! dirties the snapshot, so provenance stays unknown rather than treating a
//! later write as the saved actual.
//!
//! Field versions refine the same rule below the binding root: a read of
//! `rec.value` keeps its bound snapshot across writes, exclusive borrows, and
//! receiver-mutating calls confined to sibling fields such as `rec.other`.
//! The rooted place path of every write is compared against the read path;
//! disjoint sibling projections do not interfere — a case payload field is
//! disjoint from its siblings in the same variant, and a write spelled under
//! a different variant cannot execute while the bound snapshot's case still
//! holds — and the same rule reaches statically fixed element selections: a
//! `collection[1]` or `collection[1..3]` read survives a write provably
//! outside its window. A dynamic index or any projection the scan cannot
//! separate stays opaque and reaches everything at or below it.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};

/// Whether a mutable receiver's storage below `field_path` still holds the
/// value the invocation bound it to. `self` is bound once and transitions
/// never rebind it, so a field read names the entry field while no statement
/// that can execute before the read can write through it. The escape window
/// is reachability-bounded rather than machine-wide: an earlier arrival can
/// only have visited states that still reach the read's state, and the read's
/// own state contributes its whole statement list only when it can re-enter
/// — otherwise just the prefix through the containing statement has run.
/// Receiver storage is reached under two rooted
/// spellings — a `self.<field>` place roots at the field symbol, while a
/// whole-receiver place (`&mut self`, a `&mut self` receiver call, an
/// exclusive `self` borrow passed on) roots at the machine or attached-data
/// symbol — so every statement is scanned under both. A `self.<sibling>`
/// receiver call keeps its own field prefix through the receiver path the
/// statement carries, so mutating one field's attached machine does not
/// dirty the read field. A bare `self` read is the receiver's whole storage
/// and keeps the entry identity only while every receiver root is pristine —
/// both the whole-receiver spellings and each attached-data member the write
/// side can root at. A first step that is neither a field nor whole storage
/// still names no separable field and keeps no entry identity.
pub(super) fn receiver_field_holds_entry_value(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_symbol: SymbolHandle,
    before_statement: usize,
    field_path: &[PlaceSegment],
) -> bool {
    // An empty `field_path` is the bare `self` whole-storage read: pristine
    // there means no write or exclusive borrow under any receiver root at
    // all — including the member symbols `self.<member>` spellings root at,
    // which are each scanned with the empty read path. An unresolvable
    // receiver declaration cannot enumerate those roots and stays
    // conservative.
    let (field, member_roots) = match field_path.first() {
        Some(&PlaceSegment::Field(field)) => (Some(field), Vec::new()),
        None => match receiver_member_symbols(program, machine) {
            Some(symbols) => (None, symbols),
            None => return false,
        },
        Some(_) => return false,
    };
    // A whole-receiver place roots at whichever symbol `self` resolved to —
    // the machine, its attached data, or a state's own receiver parameter —
    // so the escape scan tracks every one of them.
    let states = program.machine_states(machine);
    let mut receiver_roots = vec![machine.symbol, machine.attached_data_symbol];
    receiver_roots.extend(states.iter().flat_map(|state| {
        program
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol)
    }));
    receiver_roots.retain(|root| root.is_valid());
    receiver_roots.dedup();
    let Some(read_index) = states.iter().position(|state| state.symbol == state_symbol) else {
        return false;
    };
    // State graph edges: the named and self targets of each transition
    // statement. A state can only have run before the read when a path from
    // it back to the read's state exists.
    let adjacency: Vec<Vec<usize>> = states
        .iter()
        .enumerate()
        .map(|(source_index, source)| {
            let mut outgoing = Vec::new();
            for statement in program.statement_table.statements(source.statement_nodes) {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    let target_index = match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { path, .. } => {
                            crate::checks::termination::named_transition_target_state_index(
                                program,
                                machine,
                                path.symbol,
                            )
                        }
                        TransitionTargetNode::SelfTarget => Some(source_index),
                        TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => None,
                    };
                    if let Some(target_index) = target_index {
                        outgoing.push(target_index);
                    }
                }
            }
            outgoing
        })
        .collect();
    // Every state that still reaches the read's state — the read's own state
    // included — may have run on an earlier arrival.
    let mut may_precede = vec![false; states.len()];
    may_precede[read_index] = true;
    let mut pending = vec![read_index];
    while let Some(index) = pending.pop() {
        for (source, outgoing) in adjacency.iter().enumerate() {
            if !may_precede[source] && outgoing.contains(&index) {
                may_precede[source] = true;
                pending.push(source);
            }
        }
    }
    // Without a path back to itself the read's own state ran each statement
    // at most once, so only the prefix through the containing statement can
    // have executed before the read.
    let mut reentrant = false;
    let mut visited = vec![false; states.len()];
    let mut pending = adjacency[read_index].clone();
    while let Some(index) = pending.pop() {
        if index == read_index {
            reentrant = true;
            break;
        }
        if visited[index] {
            continue;
        }
        visited[index] = true;
        pending.extend(adjacency[index].iter().copied());
    }
    states.iter().enumerate().all(|(index, state)| {
        if !may_precede[index] {
            return true;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        let window = if index == read_index && !reentrant {
            let Some(window) = statements.get(..before_statement.saturating_add(1)) else {
                return false;
            };
            window
        } else {
            statements
        };
        window.iter().all(|statement| {
            field.is_none_or(|field| {
                !statement_may_overwrite(
                    program,
                    machine.symbol,
                    statement,
                    field,
                    &field_path[1..],
                )
            }) && member_roots.iter().all(|root| {
                !statement_may_overwrite(program, machine.symbol, statement, *root, &[])
            }) && receiver_roots.iter().all(|root| {
                !statement_may_overwrite(program, machine.symbol, statement, *root, field_path)
            })
        })
    })
}

/// Ordinals of `state`'s transition statements that re-enter it through
/// `-> self`, forwarding the current parameter values as a fresh arrival.
/// For a mutable parameter that arrival binds whatever its storage holds at
/// the edge, so each ordinal needs its own pristine prefix.
pub(super) fn self_target_ordinals(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Vec<usize> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
                .any(|target| {
                    matches!(
                        program.statement_table.transition_target(target),
                        TransitionTargetNode::SelfTarget
                    )
                })
                .then_some(ordinal)
        })
        .collect()
}

/// One projection step below a binding's root. `Case` marks a sum's variant
/// hop — the payload field itself is the following `Field` step, matching the
/// canonical place spelling `facts::payload_variant_for_field` produces.
/// `FixedIndex` and `FixedRange` carry the canonical algebra's normalized
/// element identity for a statically known `collection[i]` or
/// `collection[start..end]` selection, so a write confined to a provably
/// disjoint element does not dirty the read. `Opaque` marks a position the
/// scan cannot separate — a dynamic index or an unresolvable member — which
/// interferes with every read at or below it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaceSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    /// A statically selected `collection[i]` element.
    FixedIndex(usize),
    /// A statically selected `collection[start..end]` window — half-open like
    /// the canonical `facts::PlaceSegment::FixedRange`; `start == end`
    /// selects no element.
    FixedRange {
        start: usize,
        end: usize,
    },
    Opaque,
}

/// The local scan's segment spelling of a canonical `facts::PlaceSegment`.
/// The canonical producer normalizes fixed selections itself; an `Index`
/// that still folds to a literal keeps the same element identity here, and a
/// genuinely dynamic index stays opaque.
pub(super) fn canonical_place_segment(
    program: &TypedTrees,
    segment: &facts::PlaceSegment,
) -> PlaceSegment {
    match segment {
        facts::PlaceSegment::Field { symbol } => PlaceSegment::Field(*symbol),
        facts::PlaceSegment::Case { variant } => PlaceSegment::Case(*variant),
        facts::PlaceSegment::FixedIndex { index } => PlaceSegment::FixedIndex(*index),
        facts::PlaceSegment::FixedRange { start, end } => PlaceSegment::FixedRange {
            start: *start,
            end: *end,
        },
        facts::PlaceSegment::Index { expression } => fixed_index_segment(program, *expression),
    }
}

/// The element segment an `Indexed` step contributes to a rooted path. The
/// index is folded by the same `constant_integer_value` normalization the
/// canonical `FixedIndex`/`FixedRange` segments use; a `start..end` range
/// index keeps its bounds as one half-open window. Anything not statically
/// known stays `Opaque` — conservative, since element writes cannot be
/// separated below the collection root.
pub(crate) fn fixed_index_segment(program: &TypedTrees, index: ExpressionHandle) -> PlaceSegment {
    let constant = |expression: ExpressionHandle| {
        program
            .expression_table
            .constant_integer_value(expression)
            .and_then(|value| usize::try_from(value).ok())
    };
    if let ExpressionNode::Range(range) = program.expression_table.expression(index)
        && let (Some(start), Some(end)) = (constant(range.start), constant(range.end))
        && let Some(end) = end.checked_add(usize::from(range.end_inclusive))
    {
        return PlaceSegment::FixedRange { start, end };
    }
    constant(index)
        .map(PlaceSegment::FixedIndex)
        .unwrap_or(PlaceSegment::Opaque)
}

/// A write to `write` reaches a read of `read` only when the two paths cannot
/// be separated: equal segments until one path ends (either covers the
/// other), or an opaque step. Distinct sibling fields never interfere. Two
/// different variants share the payload slot but a case-qualified write only
/// executes while the scrutinee holds its case — reaching a read under a
/// different variant requires re-seating the whole binding, whose root write
/// interferes on its own — so distinct `Case` hops separate the same way
/// `canonical_place_segment_pair_may_overlap` rules them non-overlapping.
/// Fixed element positions separate by the canonical window rules too:
/// disjoint indices and disjoint half-open windows never overlap, and a
/// fixed index outside a fixed window is unreachable. A segment kind the
/// scan cannot order against its counterpart — a dynamic index, an opaque
/// step, or a heterogeneous pair — stays interfering.
fn paths_interfere(write: &[PlaceSegment], read: &[PlaceSegment]) -> bool {
    for (write, read) in write.iter().zip(read.iter()) {
        match (write, read) {
            (PlaceSegment::Field(write), PlaceSegment::Field(read)) => {
                if write != read {
                    return false;
                }
            }
            (PlaceSegment::Case(write), PlaceSegment::Case(read)) => {
                if write != read {
                    return false;
                }
            }
            (PlaceSegment::FixedIndex(write), PlaceSegment::FixedIndex(read)) => {
                if write != read {
                    return false;
                }
            }
            (
                PlaceSegment::FixedRange {
                    start: write_start,
                    end: write_end,
                },
                PlaceSegment::FixedRange {
                    start: read_start,
                    end: read_end,
                },
            ) => {
                if !(write_start < write_end
                    && read_start < read_end
                    && write_start < read_end
                    && read_start < write_end)
                {
                    return false;
                }
            }
            (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex(index))
            | (PlaceSegment::FixedIndex(index), PlaceSegment::FixedRange { start, end }) => {
                if !(start < end && start <= index && index < end) {
                    return false;
                }
            }
            _ => return true,
        }
    }
    true
}

/// The binding root plus projection steps of a place expression, mirroring
/// `expression_root_symbol`'s rooting — `self.<field>` roots at the machine
/// field — while retaining the segments a whole-symbol scan discards. A
/// non-place expression has no root.
fn rooted_place_path(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<PlaceSegment>)> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            rooted_place_path(program, machine_symbol, indexed.collection).map(
                |(root, mut path)| {
                    path.push(fixed_index_segment(program, indexed.index));
                    (root, path)
                },
            )
        }
        ExpressionNode::Borrow(borrow) => rooted_place_path(program, machine_symbol, borrow.target),
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
                && path.members.count() == 1
                && path.symbol.is_valid()
                && path.symbol == machine_symbol
            {
                let symbol = crate::flow::effective_member_symbol(program, member.receiver, member);
                return symbol.is_valid().then_some((symbol, Vec::new()));
            }
            rooted_place_path(program, machine_symbol, member.receiver).map(|(root, mut path)| {
                // A case payload field crosses its variant first — the
                // canonical place spelling — so sibling payload fields of one
                // case stay separable from each other and from every other
                // case's. Synthesized members retain no `member_symbol`, so
                // identity comes from the same contextual resolver the
                // operand walks use; an unresolvable member stays opaque.
                match super::member_hop_path(program, member) {
                    Some((_, hop)) => path.extend(hop),
                    None => path.push(PlaceSegment::Opaque),
                }
                (root, path)
            })
        }
        ExpressionNode::Name(path) => {
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
                .map(|root| (root, Vec::new()))
        }
        _ => None,
    }
}

/// Every member symbol of the receiver's attached data — a `self.<member>`
/// write roots at the member's own symbol, so a bare `self` read is pristine
/// only while no statement writes or exclusively borrows under any of them.
/// Variant members and their payload fields are included: a case spelling
/// roots at its member's symbol too. An ambiguous or absent declaration
/// cannot enumerate the write surface and stays conservative.
fn receiver_member_symbols(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<Vec<SymbolHandle>> {
    let mut owners = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol);
    let data = owners.next()?;
    if owners.next().is_some() {
        return None;
    }
    Some(
        program
            .data_members(data)
            .iter()
            .flat_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => vec![field.symbol],
                typed_trees::data::DataMember::Variant(variant) => std::iter::once(variant.symbol)
                    .chain(
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| field.symbol),
                    )
                    .collect(),
            })
            .collect(),
    )
}

/// The written or exclusively borrowed place `expression` reaches `symbol`'s
/// storage at a path the read cannot be separated from.
fn place_interferes(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    rooted_place_path(program, machine_symbol, expression)
        .is_some_and(|(root, write_path)| root == symbol && paths_interfere(&write_path, read_path))
}

/// Whether `statement` may overwrite the canonical `place` at all — a write,
/// an exclusive borrow, or a receiver-mutating call the scan cannot separate
/// from the read. Non-symbol roots cannot be reasoned about and stay
/// conservative. A caller-side value proof uses this to refuse reads whose
/// call-entry context postdates a sibling operand's effects.
pub(crate) fn statement_may_overwrite_place(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return true;
    };
    let read_path: Vec<PlaceSegment> = place
        .segments
        .iter()
        .map(|segment| canonical_place_segment(program, segment))
        .collect();
    statement_may_overwrite(program, machine_symbol, statement, symbol, &read_path)
}

/// No statement in `state[start..end]` may overwrite `symbol`'s storage at
/// `read_path` or lend that projection exclusive access. An empty read path
/// is the whole binding: every write rooted at `symbol` interferes. A
/// malformed window cannot be cleared.
pub(super) fn storage_holds_bound_value(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    start: usize,
    end: usize,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    let Some(window) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(start..end)
    else {
        return false;
    };
    !window.iter().any(|statement| {
        statement_may_overwrite(program, machine_symbol, statement, symbol, read_path)
    })
}

fn statement_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            place_interferes(
                program,
                machine_symbol,
                assignment.target,
                symbol,
                read_path,
            ) || expression_may_overwrite(
                program,
                machine_symbol,
                assignment.target,
                symbol,
                read_path,
            ) || expression_may_overwrite(
                program,
                machine_symbol,
                assignment.value,
                symbol,
                read_path,
            )
        }
        StatementNode::RootBinding(binding) => {
            place_interferes(program, machine_symbol, binding.receiver, symbol, read_path)
                || expression_may_overwrite(
                    program,
                    machine_symbol,
                    binding.receiver,
                    symbol,
                    read_path,
                )
                || (binding.implementation_operand.is_valid()
                    && expression_may_overwrite(
                        program,
                        machine_symbol,
                        binding.implementation_operand,
                        symbol,
                        read_path,
                    ))
        }
        StatementNode::AssemblyFact(fact) => {
            expression_may_overwrite(program, machine_symbol, fact.expression, symbol, read_path)
        }
        StatementNode::Expression(expression) => {
            expression_may_overwrite(program, machine_symbol, *expression, symbol, read_path)
        }
        StatementNode::LocalData(local) => expression_may_overwrite(
            program,
            machine_symbol,
            local.initial_value,
            symbol,
            read_path,
        ),
        StatementNode::Call(call) => {
            (call.receiver_root_symbol == symbol
                && receiver_call_writes(program, call.target_symbol)
                && call_receiver_path_interferes(program, call, read_path))
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_may_overwrite(
                            program,
                            machine_symbol,
                            *argument,
                            symbol,
                            read_path,
                        )
                    })
        }
        StatementNode::Transition(transition) => {
            transition_may_overwrite(program, machine_symbol, transition, symbol, read_path)
        }
    }
}

/// A statement call's receiver place covers the read path. The receiver is a
/// name path, not an expression: `x` covers the whole binding, `x.<field>`
/// confines the write to the resolved leaf member, and a deeper or
/// unresolvable projection cannot be separated below its root.
fn call_receiver_path_interferes(
    program: &TypedTrees,
    call: &TableCall,
    read_path: &[PlaceSegment],
) -> bool {
    let write_path: Vec<PlaceSegment> = match program
        .statement_table
        .name_path_members(call.receiver)
        .len()
    {
        0 | 1 => Vec::new(),
        2 if call.receiver_symbol.is_valid() => {
            vec![PlaceSegment::Field(call.receiver_symbol)]
        }
        _ => vec![PlaceSegment::Opaque],
    };
    paths_interfere(&write_path, read_path)
}

/// Guard, named-target arguments, and value targets of one transition. A
/// nested exclusive borrow or receiver-mutating call inside any of them can
/// reach `symbol`'s storage before the edge is taken.
fn transition_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    transition: &TableTransition,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    if let TransitionGuardNode::When(guard) = &transition.guard
        && expression_may_overwrite(program, machine_symbol, *guard, symbol, read_path)
    {
        return true;
    }
    [transition.target, transition.continuation]
        .into_iter()
        .filter(|target| target.is_valid())
        .any(
            |target| match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| {
                        expression_may_overwrite(
                            program,
                            machine_symbol,
                            *argument,
                            symbol,
                            read_path,
                        )
                    }),
                TransitionTargetNode::Value(value) => {
                    expression_may_overwrite(program, machine_symbol, *value, symbol, read_path)
                }
                TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
            },
        )
}

/// `symbol`'s storage at `read_path` is reached by an exclusive borrow or by
/// a call whose callee may write its `self` receiver. Writes to a different
/// binding are not followed; their own `&mut symbol` creation is what dirties
/// `symbol`.
fn expression_may_overwrite(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    root: ExpressionHandle,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            // An unknown subtree cannot be cleared of reaching the storage.
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => {
                if borrow.access.is_exclusive()
                    && place_interferes(program, machine_symbol, borrow.target, symbol, read_path)
                {
                    return true;
                }
                pending.push(borrow.target);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid()
                    && place_interferes(program, machine_symbol, call.receiver, symbol, read_path)
                    && receiver_call_writes(program, call.target_symbol)
                {
                    return true;
                }
                // A receiverless call leaves this handle invalid; only
                // present children are scanned.
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::Atomic(atomic) => {
                if place_interferes(program, machine_symbol, atomic.value, symbol, read_path) {
                    return true;
                }
                pending.push(atomic.value);
                if atomic.result.is_valid() {
                    pending.push(atomic.result);
                }
            }
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Indexed(indexed) => {
                pending.push(indexed.collection);
                pending.push(indexed.index);
            }
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => {
                pending.push(range.start);
                pending.push(range.end);
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    pending.push(field.value);
                }
            }
            _ => {}
        }
    }
    false
}

/// A call whose receiver is rooted at the tracked symbol writes that storage
/// when the callee's `self` parameter is mutable. An unresolvable or
/// self-less target cannot be cleared of writing.
fn receiver_call_writes(program: &TypedTrees, target_symbol: SymbolHandle) -> bool {
    match crate::semantic_calls::call_target_parameters(program, target_symbol) {
        Some(parameters) => parameters
            .iter()
            .find(|parameter| parameter.is_self)
            .is_none_or(|parameter| parameter.is_mutable),
        None => true,
    }
}
