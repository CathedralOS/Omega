//! Atomic read/write operand reads: carrier targets, local names, and the
//! fetch/compare-exchange operand spellings an atomic call contributes.

use validation::CanonicalPlace;

use super::super::ExpressionHandle;
use super::ReadWalk;

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
pub(super) fn static_application_carries_no_caller_storage(
    program: &dyn ::validation::CheckingInput,
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

impl ReadWalk<'_, '_, '_> {
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
    pub(super) fn collect_atomic_write_reads(
        &mut self,
        expression: ExpressionHandle,
        atomic: &typed_trees::expression::TableAtomicExpression,
        depth: usize,
    ) -> bool {
        let program = self.program;
        use language_core::atomic::AtomicOrderingPlan;
        if let Some(carrier) = validation::atomic_once_carrier(program, expression) {
            return self.collect_place_read(carrier.place, depth + 1)
                && carrier
                    .operands
                    .iter()
                    .all(|operand| self.collect_operand_reads(*operand, depth + 1));
        }
        let Some(resident) = self.atomic_carrier_target(expression) else {
            return false;
        };
        let (operands, result_symbol): (Vec<ExpressionHandle>, Option<SymbolHandle>) =
            match atomic.ordering {
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
                    let Some(result_symbol) = self.atomic_local_name_symbol(atomic.result) else {
                        return false;
                    };
                    (vec![atomic.value], Some(result_symbol))
                }
                // `let r = place.fetch_op(d, ordering)` models `prior OP d`; `d` is
                // the instruction's only other operand read.
                AtomicOrderingPlan::ReadModifyWrite(_) => {
                    let Some(result_symbol) = self.atomic_local_name_symbol(atomic.result) else {
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
                    let Some(result_symbol) = self.atomic_local_name_symbol(atomic.result) else {
                        return false;
                    };
                    let Some(operands) = atomic_compare_exchange_operands(program, atomic) else {
                        return false;
                    };
                    (operands.to_vec(), Some(result_symbol))
                }
            };
        self.collect_place_read(resident, depth + 1)
            && operands
                .iter()
                .all(|operand| self.collect_operand_reads(*operand, depth))
            && result_symbol.is_none_or(|symbol| {
                // The result destination is a slot the carrier owns: join its exact
                // local place so no write reaching it slips past the recorded label.
                let place = CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(symbol),
                    segments: Vec::new(),
                };
                crate::collections::push_unique(&mut self.reads, place);
                true
            })
    }

    /// The resident place of a writing atomic is its carrier assignment's
    /// target. An atomic not carried by the exact assignment at this statement
    /// index has no provable resident place.
    fn atomic_carrier_target(&self, expression: ExpressionHandle) -> Option<ExpressionHandle> {
        let program = self.program;
        let state = self.state;
        let statement_index = self.statement_index;
        let statement = program
            .statement_table
            .statement_at(state, statement_index)?;
        let assignment = statement.as_assignment()?;
        (assignment.value == expression).then_some(assignment.target)
    }

    /// A single-member name's storage identity in this state: the resolved
    /// symbol when the path carries one, else the nearest current local binding
    /// the member text — the same name binding the desugar's generated result
    /// and placeholder names receive, since those never carry resolved symbols.
    /// The root must still be current storage at this statement; a member path,
    /// an unresolved name, or a name bound only later is not a destination.
    fn atomic_local_name_symbol(&self, expression: ExpressionHandle) -> Option<SymbolHandle> {
        let program = self.program;
        let state = self.state;
        let statement_index = self.statement_index;
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let path = program.expression_table.name_path_node(expression)?;
        if path.members.count() != 1 {
            return None;
        }
        let symbol = validation::first_valid_name_path_symbol(path, &program.expression_table)
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
        self.root_is_current(facts::PlaceRoot::Symbol(symbol))
            .then_some(symbol)
    }
}

/// The fetch update model the parser's desugar builds is `prior OP operand`:
/// its left operand is the observed-prior placeholder naming the result
/// destination, and its right operand is the authored operand `d`.
pub(super) fn atomic_fetch_operand(
    program: &dyn ::validation::CheckingInput,
    atomic: &typed_trees::expression::TableAtomicExpression,
) -> Option<ExpressionHandle> {
    let Some(binary) = program.expression_table.binary_node(atomic.value) else {
        return None;
    };
    Some(binary.right)
}

/// The decisive compare-exchange update model the parser's desugar builds is
/// `prior + (prior == expected) * (replacement - prior)`: the resident prior
/// is read through placeholder positions naming the result destination, and
/// the remaining reads are exactly the expected and replacement operands.
pub(super) fn atomic_compare_exchange_operands(
    program: &dyn ::validation::CheckingInput,
    atomic: &typed_trees::expression::TableAtomicExpression,
) -> Option<[ExpressionHandle; 2]> {
    let Some(sum) = program.expression_table.binary_node(atomic.value) else {
        return None;
    };
    let Some(product) = program.expression_table.binary_node(sum.right) else {
        return None;
    };
    let Some(equal) = program.expression_table.binary_node(product.left) else {
        return None;
    };
    let Some(difference) = program.expression_table.binary_node(product.right) else {
        return None;
    };
    Some([equal.right, difference.left])
}
