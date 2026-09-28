use ::symbols::SymbolHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn machine_state_count(program: &typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

// The machine, state and data-declaration lookups below all resolve through
// the indexes the program owns (`TypedTrees::declaration_indexes`): each
// slot pairs its map with the revision of the arenas it was built from, so
// a mutation — including the machines monomorphization appends mid-compile —
// rebuilds the index on the next query, and a dropped program can never lend
// a replacement its verdicts.

pub(crate) fn machine_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    program.machine_by_symbol(symbol)
}

pub(crate) fn state_index_by_symbol(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<usize> {
    program.state_position_by_symbol(machine.symbol, symbol)
}

pub(crate) fn data_definition_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    program.data_definition_by_symbol(symbol)
}

pub(crate) fn attached_drop_machine_exists(
    program: &typed_trees::TypedTrees,
    data_symbol: SymbolHandle,
) -> bool {
    program.attached_drop_machine_exists(data_symbol)
}

pub(crate) fn state_by_symbol<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::state::State> {
    state_index_by_symbol(program, machine, symbol)
        .map(|index| &program.machine_states(machine)[index])
}

pub(crate) fn machine_state_by_symbol(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    let machine = machine_by_symbol(program, machine_symbol)?;
    let state = state_by_symbol(program, machine, state_symbol)?;
    Some((machine, state))
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
