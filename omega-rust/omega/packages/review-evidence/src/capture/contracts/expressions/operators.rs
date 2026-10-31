use crate::capture::PackageReviewInput;
use crate::capture::api::operators::project_operator_coordinate;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::{
    PackageReviewContractBinaryOperator, PackageReviewContractOperatorMeaning,
    PackageReviewContractUnaryOperator,
};
use diagnostics::Diagnostic;

pub(crate) fn exact_checked_contract_operator_meaning(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<PackageReviewContractOperatorMeaning, Vec<Diagnostic>> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionTarget,
    };

    let selection = exact_checked_operator_selection(
        compilation,
        compilation.checking_input,
        expression,
        Some(context.selection_exposure),
    )
    .map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|mut diagnostic| {
                diagnostic.message = format!(
                    "reviewed {} `{}` contract: {}",
                    context.subject_kind, context.subject_name, diagnostic.message
                );
                diagnostic
            })
            .collect::<Vec<_>>()
    })?;
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ) => Ok(PackageReviewContractOperatorMeaning::Builtin),
        AuthoredDeclarationSelectionTarget::Resolved(target) => {
            let symbol = target.selected_symbol();
            let declaration = typed_trees::operator::declaration_by_symbol(compilation, symbol)
                .ok_or_else(|| {
                    context
                        .rejected("contract selected an operator without one retained declaration")
                })?;
            Ok(PackageReviewContractOperatorMeaning::Declared(
                project_operator_coordinate(compilation, declaration)?,
            ))
        }
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
            Err(context.rejected("contract operator selected a non-operator intrinsic"))
        }
        AuthoredDeclarationSelectionTarget::LateBound(_) => {
            Err(context.rejected("contract operator remains late-bound after checked lowering"))
        }
    }
}

/// A projection can clone expression handles, but its selection occurrences
/// must still join the unchanged compiler custody. This query issues no grant.
pub(in crate::capture) fn exact_checked_operator_selection(
    compilation: &PackageReviewInput<'_>,
    program: &dyn validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    expected_exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<language_semantics::declaration_selection::AuthoredDeclarationSelection, Vec<Diagnostic>>
{
    exact_checked_expression_selection(
        compilation,
        program,
        expression,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Operator,
        expected_exposure,
    )
}

pub(in crate::capture) fn exact_checked_expression_selection(
    compilation: &PackageReviewInput<'_>,
    program: &dyn validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
    expected_exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<language_semantics::declaration_selection::AuthoredDeclarationSelection, Vec<Diagnostic>>
{
    let mut selections = Vec::new();
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(expression)
    {
        let selection = program
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "package review expression has a dangling selection occurrence",
                )]
            })?;
        if selection.kind() == kind {
            selections.push(selection);
        }
    }
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review operator has {} exact checked selection rows; expected one",
            selections.len()
        ))]);
    };
    if !compilation
        .custody
        .typed
        .expression_table
        .expression_is_valid(expression)
        || !compilation
            .custody
            .typed
            .expression_table
            .authored_selection_occurrences(expression)
            .any(|occurrence| occurrence == selection.occurrence_id())
    {
        return Err(vec![Diagnostic::error(
            "package review operator occurrence is not attached to its retained original expression",
        )]);
    }
    if compilation
        .custody
        .authored_declaration_selections()
        .get(selection.occurrence_id())
        != Some(*selection)
    {
        return Err(vec![Diagnostic::error(
            "package review operator selection differs from immutable compiler custody",
        )]);
    }
    if let Some(expected_exposure) = expected_exposure
        && selection.exposure() != expected_exposure
    {
        return Err(vec![Diagnostic::error(format!(
            "package review {kind:?} has retained structural exposure {:?}; expected {expected_exposure:?}",
            selection.exposure()
        ))]);
    }
    Ok(**selection)
}

pub(crate) const fn project_contract_binary_operator(
    operator: typed_trees::expression::BinaryOperator,
) -> Option<PackageReviewContractBinaryOperator> {
    use typed_trees::expression::BinaryOperator;
    Some(match operator {
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
        // The review vocabulary does not yet carry a nominal tag operation.
        // Encoding it as value equality would erase payload-vs-tag meaning.
        BinaryOperator::CaseMembership => return None,
    })
}

pub(crate) const fn project_contract_unary_operator(
    operator: typed_trees::expression::UnaryOperator,
) -> PackageReviewContractUnaryOperator {
    match operator {
        typed_trees::expression::UnaryOperator::BitwiseNot => {
            PackageReviewContractUnaryOperator::BitwiseNot
        }
        typed_trees::expression::UnaryOperator::LogicalNot => {
            PackageReviewContractUnaryOperator::LogicalNot
        }
    }
}
