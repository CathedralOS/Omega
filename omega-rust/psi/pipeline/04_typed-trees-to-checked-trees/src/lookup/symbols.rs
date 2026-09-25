use ::symbols::SymbolHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use std::cell::RefCell;

pub(crate) fn machine_state_count(program: &typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

// The machine table is queried once per expression through this module: a
// whole-table `.find` per query rescans every machine. Cache each query's
// exact scan verdict (hit AND miss) per program; monomorphization appends
// machines mid-compile, so freshness is the owner pointer AND the current
// machine count. Hits are still validated against the machine's own symbol,
// so a stale or synthesized handle can only ever trigger a rescan.
thread_local! {
    static MACHINE_INDEX: RefCell<
        Option<(
            *const typed_trees::TypedTrees,
            usize,
            Option<SymbolHandle>,
            Option<SymbolHandle>,
            std::collections::HashMap<SymbolHandle, Option<usize>>,
        )>,
    > = const { RefCell::new(None) };
}

fn machine_index_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    MACHINE_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        let machines = program.machines();
        let first = machines.first().map(|machine| machine.symbol);
        let last = machines.last().map(|machine| machine.symbol);
        let stale = match &*slot {
            Some((owner, len, first_anchor, last_anchor, _)) => {
                !std::ptr::eq(*owner, program as *const _)
                    || *len != machines.len()
                    || *first_anchor != first
                    || *last_anchor != last
            }
            None => true,
        };
        if stale {
            *slot = Some((
                program as *const typed_trees::TypedTrees,
                machines.len(),
                first,
                last,
                std::collections::HashMap::new(),
            ));
        }
        let Some((_, _, _, _, verdicts)) = &mut *slot else {
            return None;
        };
        match verdicts.get(&symbol) {
            Some(Some(index)) => {
                // A cached hit still verifies: a stale map under a reused
                // address can only ever send the query back to the scan.
                if machines
                    .get(*index)
                    .is_some_and(|machine| machine.symbol == symbol)
                {
                    Some(*index)
                } else {
                    verdicts.remove(&symbol);
                    let found = machines.iter().position(|machine| machine.symbol == symbol);
                    verdicts.insert(symbol, found);
                    found
                }
            }
            Some(None) => None,
            None => {
                let found = machines.iter().position(|machine| machine.symbol == symbol);
                verdicts.insert(symbol, found);
                found
            }
        }
    })
}

pub(crate) fn machine_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    machine_index_by_symbol(program, symbol).map(|index| &program.machines()[index])
}

pub(crate) fn machine_symbol_from_type_reference_handle(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        // A `dyn Trait` retains the trait symbol. Exact checked conformance rows
        // drive devirtualization and descriptor lowering; changing this symbol
        // to a coincidentally unique carrier would discard that identity and
        // restore attached-machine discovery.
        typed_trees::types::TypeReferenceNode::DynamicTrait {
            symbol: trait_symbol,
            ..
        } => *trait_symbol,
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

pub(crate) fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Borrow(inner) => {
            expression_root_symbol(inner.target, expressions, machine_symbol)
        }
        ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
            }
            _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
        },
        ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
        _ => None,
    }
}

pub(crate) fn first_valid_name_path_symbol(
    path: &typed_trees::expression::TableNamePath,
    expressions: &typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}
