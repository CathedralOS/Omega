pub(crate) fn lower_operator_definition(
    operator: &omega_symbol_resolved_trees::operator::OperatorDefinition,
) -> omega_typed_trees::operator::OperatorDefinition {
    omega_typed_trees::operator::OperatorDefinition {
        token_count: operator.token_count,
    }
}
