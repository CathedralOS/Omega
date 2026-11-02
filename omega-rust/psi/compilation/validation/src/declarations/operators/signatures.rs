use typed_trees::operator::operator_operand_signature;

pub(super) fn operator_signature_key(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
) -> String {
    format!(
        "{}({})",
        operator_name(program, operator),
        operator_operand_signature(program, operator)
    )
}

/// The canonical operand-type key for an operator (its parameter types,
/// normalized over type parameters) without the operator name. Used by the
/// spelling-overlap ambiguity rule, where the spelling already serves as the
/// first-level discriminator.
///
/// Delegates to the shared `operator_operand_signature` in `typed-trees`
/// so declaration-level ambiguity validation and expression-level spelling
/// dispatch normalize operand types identically.
pub(super) fn operator_operand_key(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
) -> String {
    operator_operand_signature(program, operator)
}

pub(super) fn operator_name(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
) -> String {
    program.operator_path_string(operator.name)
}
