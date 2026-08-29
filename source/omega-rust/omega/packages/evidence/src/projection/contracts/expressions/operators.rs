use crate::evidence::{
    PackageReviewContractBinaryOperator, PackageReviewContractOperatorMeaning,
    PackageReviewContractUnaryOperator,
};
use crate::projection::api::operators::project_operator_coordinate;
use crate::projection::contracts::checked::facts::ContractProjectionContext;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn exact_checked_contract_operator_meaning(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Result<PackageReviewContractOperatorMeaning, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionIntrinsic,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator has {} exact checked selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ) => Ok(PackageReviewContractOperatorMeaning::Builtin),
        AuthoredDeclarationSelectionTarget::Resolved(target) => {
            let symbol = target.selected_symbol();
            let declaration = psi_typed_trees::operator::declaration_by_symbol(compilation, symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contract selected an operator without one retained declaration",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            Ok(PackageReviewContractOperatorMeaning::Declared(
                project_operator_coordinate(compilation, declaration)?,
            ))
        }
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator selected a non-operator intrinsic",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator remains late-bound after checked lowering",
            context.subject_kind, context.subject_name
        ))]),
    }
}

pub(crate) const fn project_contract_binary_operator(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> PackageReviewContractBinaryOperator {
    use psi_typed_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Add => PackageReviewContractBinaryOperator::Add,
        BinaryOperator::And => PackageReviewContractBinaryOperator::And,
        BinaryOperator::BitwiseAnd => PackageReviewContractBinaryOperator::BitwiseAnd,
        BinaryOperator::BitwiseOr => PackageReviewContractBinaryOperator::BitwiseOr,
        BinaryOperator::BitwiseXor => PackageReviewContractBinaryOperator::BitwiseXor,
        BinaryOperator::Divide => PackageReviewContractBinaryOperator::Divide,
        BinaryOperator::Equal => PackageReviewContractBinaryOperator::Equal,
        BinaryOperator::Greater => PackageReviewContractBinaryOperator::Greater,
        BinaryOperator::GreaterOrEqual => PackageReviewContractBinaryOperator::GreaterOrEqual,
        BinaryOperator::Less => PackageReviewContractBinaryOperator::Less,
        BinaryOperator::LessOrEqual => PackageReviewContractBinaryOperator::LessOrEqual,
        BinaryOperator::Modulo => PackageReviewContractBinaryOperator::Modulo,
        BinaryOperator::Multiply => PackageReviewContractBinaryOperator::Multiply,
        BinaryOperator::NotEqual => PackageReviewContractBinaryOperator::NotEqual,
        BinaryOperator::Or => PackageReviewContractBinaryOperator::Or,
        BinaryOperator::ShiftLeft => PackageReviewContractBinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight => PackageReviewContractBinaryOperator::ShiftRight,
        BinaryOperator::Subtract => PackageReviewContractBinaryOperator::Subtract,
    }
}

pub(crate) const fn project_contract_unary_operator(
    operator: psi_typed_trees::expression::UnaryOperator,
) -> PackageReviewContractUnaryOperator {
    match operator {
        psi_typed_trees::expression::UnaryOperator::BitwiseNot => {
            PackageReviewContractUnaryOperator::BitwiseNot
        }
        psi_typed_trees::expression::UnaryOperator::LogicalNot => {
            PackageReviewContractUnaryOperator::LogicalNot
        }
    }
}
