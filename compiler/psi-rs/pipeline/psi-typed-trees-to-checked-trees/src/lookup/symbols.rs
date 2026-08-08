use super::*;

pub(crate) fn machine_state_count(program: &psi_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

pub(crate) fn machine_by_symbol(
    program: &psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::machine::Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
}

pub(crate) fn machine_symbol_from_type_reference_handle(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | psi_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        // A `dyn Trait` retains the trait symbol. Exact checked conformance rows
        // drive devirtualization and descriptor lowering; changing this symbol
        // to a coincidentally unique carrier would discard that identity and
        // restore attached-machine discovery.
        psi_typed_trees::types::TypeReferenceNode::DynamicTrait {
            symbol: trait_symbol,
            ..
        } => *trait_symbol,
        psi_typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | psi_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | psi_typed_trees::types::TypeReferenceNode::Slice { .. }
        | psi_typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

pub(crate) fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &psi_typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Mutable(inner) => {
            expression_root_symbol(*inner, expressions, machine_symbol)
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
    path: &psi_typed_trees::expression::TableNamePath,
    expressions: &psi_typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}
