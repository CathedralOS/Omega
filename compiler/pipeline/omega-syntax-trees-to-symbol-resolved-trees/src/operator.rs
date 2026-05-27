use omega_syntax_trees as syntax;

pub(crate) fn lower_operator_definition(
    operator: &syntax::item::OperatorDefinition,
) -> omega_symbol_resolved_trees::operator::OperatorDefinition {
    omega_symbol_resolved_trees::operator::OperatorDefinition {
        token_count: operator.token_count,
    }
}
