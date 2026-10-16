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
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
};

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
    program: &dyn ::validation::CheckingInput,
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
        facts::PlaceSegment::OpaqueRange => PlaceSegment::Opaque,
    }
}

/// The element segment an `Indexed` step contributes to a rooted path. The
/// index is folded by the same `constant_integer_value` normalization the
/// canonical `FixedIndex`/`FixedRange` segments use; a `start..end` range
/// index keeps its bounds as one half-open window. Anything not statically
/// known stays `Opaque` — conservative, since element writes cannot be
/// separated below the collection root.
pub(crate) fn fixed_index_segment(
    program: &dyn ::validation::CheckingInput,
    index: ExpressionHandle,
) -> PlaceSegment {
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
    program: &dyn ::validation::CheckingInput,
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
            if let Some(path) = program.expression_table.name_path_node(member.receiver)
                && path.members.count() == 1
                && path.symbol.is_valid()
                && path.symbol == machine_symbol
            {
                let symbol = validation::effective_member_symbol(program, member.receiver, member);
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
            validation::first_valid_name_path_symbol(path, &program.expression_table)
                .map(|root| (root, Vec::new()))
        }
        _ => None,
    }
}

/// The written or exclusively borrowed place `expression` reaches `symbol`'s
/// storage at a path the read cannot be separated from.
fn place_interferes(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    place: &validation::CanonicalPlace,
) -> bool {
    let Some(symbol) = place.root.as_symbol() else {
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    match statement {
        StatementNode::Assembly(_) => false,
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
        StatementNode::AssemblyFact(fact) => program
            .tables
            .proof_fact_expression_roots(fact.fact)
            .into_iter()
            .any(|expression| {
                expression_may_overwrite(program, machine_symbol, expression, symbol, read_path)
            }),
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
                || call_arguments_may_overwrite(
                    program,
                    machine_symbol,
                    call.target_symbol,
                    program.statement_table.expression_handles(call.arguments),
                    symbol,
                    read_path,
                )
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    transition.targets().any(
        |target| match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let arguments = program.statement_table.expression_handles(*arguments);
                let internal = program
                    .machine_by_symbol(machine_symbol)
                    .is_some_and(|machine| {
                        validation::activation_target_state_index(program, machine, path.symbol)
                            .is_some()
                    });
                (!internal
                    && call_arguments_may_overwrite(
                        program,
                        machine_symbol,
                        path.symbol,
                        arguments,
                        symbol,
                        read_path,
                    ))
                    || arguments.iter().any(|argument| {
                        expression_may_overwrite(
                            program,
                            machine_symbol,
                            *argument,
                            symbol,
                            read_path,
                        )
                    })
            }
            TransitionTargetNode::Value(value) => {
                expression_may_overwrite(program, machine_symbol, *value, symbol, read_path)
            }
            TransitionTargetNode::Terminal => false,
        },
    )
}

/// `symbol`'s storage at `read_path` is reached by an exclusive borrow or by
/// a call whose callee may write its receiver or a reference argument.
/// Writes through aliases are conservatively covered by their exclusive
/// borrow creation or the call that receives mutable access.
fn expression_may_overwrite(
    program: &dyn ::validation::CheckingInput,
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
                if call_arguments_may_overwrite(
                    program,
                    machine_symbol,
                    call.target_symbol,
                    program.expression_table.expression_handles(call.arguments),
                    symbol,
                    read_path,
                ) {
                    return true;
                }
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
                crate::facts::push_match_children(program, dispatch, &mut pending)
            }
            _ => crate::facts::push_expression_children(program, expression, &mut pending),
        }
    }
    false
}

/// A call whose receiver is rooted at the tracked symbol writes that storage
/// when the callee's `self` parameter is mutable. An unresolvable or
/// self-less target cannot be cleared of writing.
fn receiver_call_writes(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
) -> bool {
    match validation::call_target_parameters(program, target_symbol) {
        Some(parameters) => parameters
            .iter()
            .find(|_parameter| false)
            .is_none_or(|parameter| parameter.is_mutable),
        None => true,
    }
}

/// Passing an existing mutable reference grants the same write access as
/// spelling a fresh exclusive borrow. Unknown callees cannot clear a place
/// argument of writing; ordinary value parameters do not grant such access.
fn call_arguments_may_overwrite(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    arguments: &[ExpressionHandle],
    symbol: SymbolHandle,
    read_path: &[PlaceSegment],
) -> bool {
    // A computed reference can select an alias without spelling one place.
    // Lack of a resolved place is not evidence that a write is disjoint.
    let argument_may_write = |argument: &ExpressionHandle| {
        rooted_place_path(program, machine_symbol, *argument).is_none_or(|(root, write_path)| {
            root == symbol && paths_interfere(&write_path, read_path)
        })
    };
    let Some(parameters) = validation::call_target_parameters(program, target_symbol) else {
        return arguments.iter().any(argument_may_write);
    };
    parameters
        .iter()
        .zip(arguments)
        .any(|(parameter, argument)| {
            let mut reference = parameter.type_reference;
            let writes = loop {
                match program.type_reference_table.type_reference(reference) {
                    typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                        reference = *base_type
                    }
                    typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                        break access.is_exclusive();
                    }
                    _ => break false,
                }
            };
            writes && argument_may_write(argument)
        })
}
