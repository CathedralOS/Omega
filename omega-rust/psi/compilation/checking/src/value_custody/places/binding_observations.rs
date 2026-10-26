//! Preservation of a binding's observed value and its descriptor home.
//!
//! A captured descriptor can survive writes through it, while replacing or
//! exposing its home retires that relationship. These queries never replay an
//! initializer and never supply current referent contents.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// Fixed fields of a private owned binding may carry completed descriptors
/// too. No reference may be crossed before the descriptor slot: a reference
/// to a record lends replaceable fields, rather than a private captured home.
/// Indexed descriptors retain their selector/replacement dependencies.
/// Mutable backing is allowed: this preserves metadata, never element contents.
pub fn stable_slice_descriptor_place(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
    segments: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let root_reference = program
        .state_parameter_by_symbol(state, symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, symbol)
                .map(|(_, local)| local.type_reference)
        })?;
    let mut reference = root_reference;
    for (position, segment) in segments.iter().enumerate() {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        if !symbol.is_valid() || crate::reference_shell(program, reference).is_some() {
            return None;
        }
        reference = super::project_type_reference_from_segments(
            program,
            root_reference,
            &segments[..=position],
        )?;
    }
    crate::slice_view_shell(program, reference, &[])?;
    (!binding_is_exclusively_exposed(program, state, symbol)
        && !binding_is_written(program, state, symbol, BindingObservation::Descriptor))
    .then_some(reference)
}

/// An immutable local's initializer describes this occurrence only while no
/// exclusive borrow roots at its binding: `expose(&mut view)` hands the
/// descriptor to a callee that can install a different slice, so the saved
/// initializer no longer names the referent the call sees. A borrow that
/// selects into the binding (`&mut view[a..b]`, `&mut view[i]`) lends backing
/// contents through a descriptor of its own and cannot replace the binding's
/// descriptor, so it leaves the saved length valid; under
/// `bare-range-reference-access` that spelling is the only mutable window.
/// The scan is deliberately state-wide -- the caller's obligation order is
/// not the statement order -- and covers every carrier that can hold an
/// expression. Ordinary borrows and representation recasts use the same
/// formation source. Placed factory views interpret separate backing, not the
/// source owner's descriptor bytes.
pub(crate) fn binding_is_exclusively_exposed(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    binding_has_exclusive_exposure(program, state, symbol, BindingObservation::Descriptor)
}

fn binding_has_exclusive_exposure(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
    observation: BindingObservation,
) -> bool {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
    for root in statement_expression_roots(program, state) {
        crate::expression_traversal::collect_expression_nodes(program, root, &mut nodes, &mut seen);
    }
    nodes.iter().any(|node| {
        crate::value_custody::recasts::reference_formation_source(program, *node).is_some_and(
            |formation| {
                !formation.interpretation.is_placed()
                    && formation.access.is_exclusive()
                    && borrow_root_symbol(program, formation.source) == Some(symbol)
                    && (observation == BindingObservation::WholeValue
                        || !selects_builtin_collection_contents(program, state, formation.source))
            },
        )
    })
}

/// Whether a place selects elements of its root binding rather than
/// the binding (or one of its fields) itself. An indexed step anywhere on the
/// path lends contents under a new descriptor; the root's own descriptor stays
/// where the declaration left it.
fn selects_collection_contents(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> bool {
    let mut visited = 0;
    while program.expression_table.expression_is_valid(expression) && visited < 128 {
        visited += 1;
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(_) => return true,
            _ => return false,
        }
    }
    false
}

fn selects_builtin_collection_contents(
    program: &dyn crate::CheckingInput,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    // Authored indexing can lend a descriptor slot or execute other writes.
    // Syntax alone cannot classify an exclusive borrow as payload-only.
    selects_collection_contents(program, expression)
        && program
            .machine_holding_state(state.symbol)
            .is_some_and(|machine| {
                crate::place_has_builtin_coordinates(program, machine, Some(state), expression)
            })
}

/// Metadata and value observations have different storage dependencies. A
/// builtin element write can replace a nested descriptor, but never the outer
/// slice's pointer or extent. Whole-value observations still depend on contents.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingObservation {
    WholeValue,
    Descriptor,
}

/// An entry observation names the value the call edge still sees only while
/// nothing in `state` writes the carrier. `binding_is_exclusively_exposed`
/// covers exclusive reference formations; the remaining write channels are
/// an assignment reaching the observed storage, a receiver call
/// whose callee claims a mutable `self`, and a sealed atomic operation on
/// the place. Reads and moves leave the observed value alone -- a moved
/// name cannot be an actual afterwards. The scan is state-wide for the same
/// reason as the borrow scan: obligation order is not statement order.
pub(crate) fn binding_is_written(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
    observation: BindingObservation,
) -> bool {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        if let StatementNode::Assignment(assignment) = statement
            && borrow_root_symbol(program, assignment.target) == Some(symbol)
        {
            // Indexed syntax alone does not prove an element write:
            // authored indexing can execute arbitrary code. Only builtin
            // coordinates separate descriptor storage from its elements.
            let selects_contents = observation == BindingObservation::Descriptor
                && selects_builtin_collection_contents(program, state, assignment.target);
            if !selects_contents {
                return true;
            }
        }
    }
    binding_has_write_effects(program, state, symbol, observation)
}

/// Writes performed by calls or sealed atomic operations, independently of an
/// assignment's explicit destination. A source-value transfer can account for
/// its own direct stores without mistaking these effects for those stores.
/// Explicit exclusive borrows remain a separate exposure observation.
pub(crate) fn binding_has_write_effects(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
    observation: BindingObservation,
) -> bool {
    if program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::Call(call)
                if call.receiver_root_symbol == symbol
                    && call_receiver_is_mutable(program, call.target_symbol))
        })
    {
        return true;
    }
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
    for root in statement_expression_roots(program, state) {
        crate::expression_traversal::collect_expression_nodes(program, root, &mut nodes, &mut seen);
    }
    nodes
        .iter()
        .any(|node| match program.expression_table.expression(*node) {
            ExpressionNode::Call(call) => {
                call.receiver.is_valid()
                    && borrow_root_symbol(program, call.receiver) == Some(symbol)
                    && call_receiver_is_mutable(program, call.target_symbol)
            }
            ExpressionNode::Atomic(atomic) => {
                if let Some(carrier) =
                    crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
                {
                    // A resident update cannot replace the descriptor home used
                    // to reach it. Whole-value observations still see the write.
                    return observation != BindingObservation::Descriptor
                        && borrow_root_symbol(program, carrier.place) == Some(symbol);
                }
                borrow_root_symbol(program, atomic.value) == Some(symbol)
                    || borrow_root_symbol(program, atomic.result) == Some(symbol)
            }
            _ => false,
        })
}

/// Unknown callees conservatively invalidate a receiver observation. Resolved
/// static calls carry every writable place through their explicit arguments.
fn call_receiver_is_mutable(
    program: &dyn crate::CheckingInput,
    target_symbol: SymbolHandle,
) -> bool {
    crate::machine_calls::calls::machine_state_by_symbol(program, target_symbol).is_none()
}

/// Every expression root a statement of `state` can carry: the channels a
/// nested borrow or receiver call could reach an observed carrier through.
fn statement_expression_roots(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Vec<ExpressionHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .flat_map(|statement| program.tables.expression_roots(statement))
        .map(|(_, root)| root)
        .collect()
}

/// The root symbol a borrowable place path descends from: `view`, `view.field`,
/// or `view[i]` all root at `view`'s local symbol.
fn borrow_root_symbol(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let mut visited = 0;
    while program.expression_table.expression_is_valid(expression) && visited < 128 {
        visited += 1;
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Name(path) => return Some(path.head_symbol),
            _ => return None,
        }
    }
    None
}

/// The complete stored value has no observed write or exclusive exposure in
/// this state. Unlike descriptor preservation, an exclusive borrow into a
/// builtin element can change this value and therefore retires the identity.
/// This conservative bar is independent of proof/call obligation order.
pub(crate) fn binding_is_unchanged_value(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    !binding_is_written(program, state, symbol, BindingObservation::WholeValue)
        && !binding_has_exclusive_exposure(program, state, symbol, BindingObservation::WholeValue)
}

/// A prepared builtin place can denote the same value without another stored
/// capture only while every named coordinate is immutable and unchanged. This
/// supplies value identity, never a loan, ownership transfer or permission to
/// duplicate a resource. The consumer separately decides whether its carrier
/// admits a logical observation. Calls and computed selectors must first be
/// captured by their ordinary evaluation owner; this query never repeats them.
pub fn place_has_immutable_denotation(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &State,
    before: usize,
    expression: ExpressionHandle,
) -> bool {
    if !crate::place_has_builtin_coordinates(program, machine, Some(state), expression) {
        return false;
    }
    let Some(place) =
        crate::canonical_place_from_expression_in_state(program, state.symbol, before, expression)
    else {
        return false;
    };
    if place.root.as_symbol().is_none()
        || !(0..=place.segments.len()).all(|length| {
            crate::canonical_place_type_reference(
                program,
                state.symbol,
                before,
                &crate::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..length].to_vec(),
                },
            )
            .is_some_and(|reference| crate::reference_shell(program, reference).is_none())
        })
    {
        return false;
    }
    let mut nodes = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, expression, &mut nodes);
    nodes
        .into_iter()
        .all(|node| match program.expression_table.expression(node) {
            ExpressionNode::Name(path) => {
                let symbol = path.head_symbol;
                let reference = if let Some(parameter) =
                    program.state_parameter_by_symbol(state, symbol)
                {
                    (!parameter.is_mutable).then_some(parameter.type_reference)
                } else {
                    program
                        .state_local_declaration(state, symbol)
                        .and_then(|(position, local)| {
                            (position < before && !local.is_mutable).then_some(local.type_reference)
                        })
                };
                reference.is_some_and(|reference| {
                    crate::reference_shell(program, reference).is_none()
                        && binding_is_unchanged_value(program, state, symbol)
                })
            }
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Integer(_) => {
                true
            }
            _ => false,
        })
}

/// Rejoin an earlier immutable binding to the exact value it produced. Every
/// definition precedes the occurrence it serves, so alias cycles and forward
/// references decline. A fixed home alone does not freeze fields; whole-value
/// writes and exclusive exposures also prevent this rejoin.
///
/// The returned position belongs to the original producer, not the later use.
/// This supplies no current referent contents, projection or loan authority.
/// Calls and constructors remain produced expressions for the consumer to
/// judge; the query never imports their guarantees or executes them again.
pub(crate) fn immutable_value_origin(
    program: &dyn crate::CheckingInput,
    state: &State,
    mut before: usize,
    mut expression: ExpressionHandle,
) -> Option<(usize, ExpressionHandle)> {
    loop {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let Some(path) = program.expression_table.name_path_node(expression) else {
            return Some((before, expression));
        };
        if !path.symbol.is_valid() {
            return None;
        }
        if path.head_symbol != path.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return Some((before, expression));
        }
        let Some((position, local)) = program.state_local_declaration(state, path.symbol) else {
            return Some((before, expression));
        };
        if position >= before
            || local.is_mutable
            || !local.initial_value.is_valid()
            || !binding_is_unchanged_value(program, state, local.symbol)
        {
            return None;
        }
        before = position;
        expression = local.initial_value;
    }
}
