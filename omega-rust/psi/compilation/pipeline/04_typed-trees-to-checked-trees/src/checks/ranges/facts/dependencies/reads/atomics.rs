//! Atomic read/write operand reads: carrier targets, local names, and the
//! fetch/compare-exchange operand spellings an atomic call contributes.

use super::super::{ExpressionHandle, ExpressionNode, Machine, State, TypedTrees};
use super::operands::{collect_operand_reads, collect_place_read};
use super::root_currency::root_is_current;
use crate::checks::ranges::facts::RangeCallContext;
use crate::flow::CanonicalPlace;
use checked_trees::CheckedOperatorFacts;
use symbols::SymbolHandle;

/// Whether a call's static application substitutes only storage-free
/// selections, so the checked operand accesses still enumerate the complete
/// caller footprint. A type argument names a declaration identity and a
/// const argument names a compile-time value; neither is a place, and
/// specialization "creates no runtime dictionary", so a statically applied
/// call reads exactly what the same call without the application reads. A
/// machine-valued argument is different in kind: it hands the callee a
/// callable body this occurrence never authenticated, whose own reads no
/// operand scan here describes. A nested static application or an evidence
/// projection can carry such a binder below the argument this scan sees, so
/// both stay unproven rather than being walked for a machine leaf.
/// Whether a call's static application substitutes only storage-free
/// selections, so the checked operand accesses still enumerate the complete
/// caller footprint. A type argument names a declaration identity and a
/// const argument names a compile-time value; neither is a place, and
/// specialization "creates no runtime dictionary", so a statically applied
/// call reads exactly what the same call without the application reads. A
/// machine-valued argument is different in kind: it hands the callee a
/// callable body this occurrence never authenticated, whose own reads no
/// operand scan here describes. A nested static application or an evidence
/// projection can carry such a binder below the argument this scan sees, so
/// both stay unproven rather than being walked for a machine leaf.
pub(super) fn static_application_carries_no_caller_storage(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    call.machine_arguments.iter().all(|argument| {
        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return false;
        }
        if !argument.symbol.is_valid() {
            // A literal const selection retains no declaration symbol; every
            // other symbol-free argument is an unresolved selection.
            return argument.const_literal.is_some();
        }
        matches!(
            program.symbols.get(argument.symbol).kind,
            symbols::SymbolKind::BuiltinType
                | symbols::SymbolKind::Data
                | symbols::SymbolKind::Const
        )
    })
}

/// The footprint of a writing atomic (`store`, `swap`, fetch, or decisive
/// compare-exchange). All four exist only as an assignment carrier: the
/// parser desugars `place.store(v, ord)` and the `let r = place.op(..)`
/// forms into `target = Atomic { .. }`, so the resident place is that
/// carrier statement's `target`, never a child of `value`. The admitted
/// read set is every storage position the carrier names: the resident
/// place, the operand expressions the instruction consults besides the
/// resident place (`value` for store/swap, the fetch operand for
/// read-modify-write, the expected and replacement operands for a decisive
/// compare-exchange whose `value` is the exact prior-shaped update model),
/// and the `result` destination the carrier materializes into — a slot the
/// carrier owns, so a write reaching it can never slip past the recorded
/// label. The model's prior positions are placeholders for the
/// instruction-observed pre-write value naming the same local `result`
/// receives; they are never scanned as local reads. A missing carrier or a
/// result destination that is not current local storage leaves the
/// footprint incomplete.

/// The footprint of a writing atomic (`store`, `swap`, fetch, or decisive
/// compare-exchange). All four exist only as an assignment carrier: the
/// parser desugars `place.store(v, ord)` and the `let r = place.op(..)`
/// forms into `target = Atomic { .. }`, so the resident place is that
/// carrier statement's `target`, never a child of `value`. The admitted
/// read set is every storage position the carrier names: the resident
/// place, the operand expressions the instruction consults besides the
/// resident place (`value` for store/swap, the fetch operand for
/// read-modify-write, the expected and replacement operands for a decisive
/// compare-exchange whose `value` is the exact prior-shaped update model),
/// and the `result` destination the carrier materializes into — a slot the
/// carrier owns, so a write reaching it can never slip past the recorded
/// label. The model's prior positions are placeholders for the
/// instruction-observed pre-write value naming the same local `result`
/// receives; they are never scanned as local reads. A missing carrier or a
/// result destination that is not current local storage leaves the
/// footprint incomplete.
pub(super) fn collect_atomic_write_reads<'a, 'b>(
    program: &'a TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    atomic: &typed_trees::expression::TableAtomicExpression,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
    bound_lookup: &validation::ImmutableBoundLookup<'b>,
) -> bool {
    use language_core::atomic::AtomicOrderingPlan;
    let Some(resident) = atomic_carrier_target(program, state, statement_index, expression) else {
        return false;
    };
    let (operands, result_symbol): (Vec<ExpressionHandle>, Option<SymbolHandle>) = match atomic
        .ordering
    {
        // A load is the place-shaped observation `collect_reads` admits; the
        // single-attempt observing compare-exchange names an outcome carrier
        // whose failure path may be an uncommitted attempt, never the
        // resident place's prior value, so it has no scalar footprint.
        AtomicOrderingPlan::Load(_) | AtomicOrderingPlan::CompareExchangeOnce { .. } => {
            return false;
        }
        // `place.store(v, ordering)` reads the resident place and the
        // stored operand `v`.
        AtomicOrderingPlan::Store(_) => (vec![atomic.value], None),
        // `let r = place.swap(v, ordering)` reads the displaced prior into
        // `r` and writes `v`; `value` is the replacement operand.
        AtomicOrderingPlan::Swap(_) => {
            let Some(result_symbol) =
                atomic_local_name_symbol(program, machine, state, statement_index, atomic.result)
            else {
                return false;
            };
            (vec![atomic.value], Some(result_symbol))
        }
        // `let r = place.fetch_op(d, ordering)` models `prior OP d`; `d` is
        // the instruction's only other operand read.
        AtomicOrderingPlan::ReadModifyWrite(_) => {
            let Some(result_symbol) =
                atomic_local_name_symbol(program, machine, state, statement_index, atomic.result)
            else {
                return false;
            };
            let Some(operand) = atomic_fetch_operand(program, atomic) else {
                return false;
            };
            (vec![operand], Some(result_symbol))
        }
        // A decisive compare-exchange reads the resident prior plus the
        // expected and replacement operands of its update model.
        AtomicOrderingPlan::CompareExchange { .. } => {
            let Some(result_symbol) =
                atomic_local_name_symbol(program, machine, state, statement_index, atomic.result)
            else {
                return false;
            };
            let Some(operands) = atomic_compare_exchange_operands(program, atomic) else {
                return false;
            };
            (operands.to_vec(), Some(result_symbol))
        }
    };
    collect_place_read(
        program,
        machine,
        state,
        statement_index,
        resident,
        calls,
        operators,
        reads,
        depth + 1,
        bound_lookup,
    ) && operands.iter().all(|operand| {
        collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            *operand,
            calls,
            operators,
            reads,
            depth,
            bound_lookup,
        )
    }) && result_symbol.is_none_or(|symbol| {
        // The result destination is a slot the carrier owns: join its exact
        // local place so no write reaching it slips past the recorded label.
        let place = CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        };
        if !reads.contains(&place) {
            reads.push(place);
        }
        true
    })
}

/// The resident place of a writing atomic is its carrier assignment's
/// target. An atomic not carried by the exact assignment at this statement
/// index has no provable resident place.

/// The resident place of a writing atomic is its carrier assignment's
/// target. An atomic not carried by the exact assignment at this statement
/// index has no provable resident place.
pub(super) fn atomic_carrier_target(
    program: &TypedTrees,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
        return None;
    };
    (assignment.value == expression).then_some(assignment.target)
}

/// A single-member name's storage identity in this state: the resolved
/// symbol when the path carries one, else the nearest current local binding
/// the member text — the same name binding the desugar's generated result
/// and placeholder names receive, since those never carry resolved symbols.
/// The root must still be current storage at this statement; a member path,
/// an unresolved name, or a name bound only later is not a destination.

/// A single-member name's storage identity in this state: the resolved
/// symbol when the path carries one, else the nearest current local binding
/// the member text — the same name binding the desugar's generated result
/// and placeholder names receive, since those never carry resolved symbols.
/// The root must still be current storage at this statement; a member path,
/// an unresolved name, or a name bound only later is not a destination.
pub(super) fn atomic_local_name_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if path.members.count() != 1 {
        return None;
    }
    let symbol = crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
        .or_else(|| {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .first()?;
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .rev()
                .find_map(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if local.name == *name =>
                    {
                        Some(local.symbol)
                    }
                    _ => None,
                })
        })?;
    root_is_current(
        program,
        machine,
        state,
        statement_index,
        facts::PlaceRoot::Symbol(symbol),
    )
    .then_some(symbol)
}

/// The fetch update model the parser's desugar builds is `prior OP operand`:
/// its left operand is the observed-prior placeholder naming the result
/// destination, and its right operand is the authored operand `d`.

/// The fetch update model the parser's desugar builds is `prior OP operand`:
/// its left operand is the observed-prior placeholder naming the result
/// destination, and its right operand is the authored operand `d`.
pub(super) fn atomic_fetch_operand(
    program: &TypedTrees,
    atomic: &typed_trees::expression::TableAtomicExpression,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(atomic.value) else {
        return None;
    };
    Some(binary.right)
}

/// The decisive compare-exchange update model the parser's desugar builds is
/// `prior + (prior == expected) * (replacement - prior)`: the resident prior
/// is read through placeholder positions naming the result destination, and
/// the remaining reads are exactly the expected and replacement operands.

/// The decisive compare-exchange update model the parser's desugar builds is
/// `prior + (prior == expected) * (replacement - prior)`: the resident prior
/// is read through placeholder positions naming the result destination, and
/// the remaining reads are exactly the expected and replacement operands.
pub(super) fn atomic_compare_exchange_operands(
    program: &TypedTrees,
    atomic: &typed_trees::expression::TableAtomicExpression,
) -> Option<[ExpressionHandle; 2]> {
    let ExpressionNode::Binary(sum) = program.expression_table.expression(atomic.value) else {
        return None;
    };
    let ExpressionNode::Binary(product) = program.expression_table.expression(sum.right) else {
        return None;
    };
    let ExpressionNode::Binary(equal) = program.expression_table.expression(product.left) else {
        return None;
    };
    let ExpressionNode::Binary(difference) = program.expression_table.expression(product.right)
    else {
        return None;
    };
    Some([equal.right, difference.left])
}
