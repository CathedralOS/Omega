//! Collection, substituted, computed, and checked-path member projection.

use super::super::calls::exact_fact_call_projection;
use super::super::members::{
    checked_contract_member_path, contract_member_has_exact_collection_length,
    contract_member_path_root, contract_member_path_source, is_data_subject_field_expression,
    project_computed_contract_member_expression, project_contract_member_expression,
    require_exact_checked_contract_collection_length,
    require_exact_checked_contract_nominal_member,
};
use super::super::names::contract_parameter_field_symbol;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::PackageReviewContractExpression;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};

pub(super) fn project_member_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: ExpressionHandle,
    member: &TableMemberExpression,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    projection_substitutions: &[(SymbolHandle, ExpressionHandle)],
    checked_fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    child: &impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    if contract_member_has_exact_collection_length(compilation, expression) {
        require_exact_checked_contract_collection_length(compilation, context, expression, member)?;
        return Ok(PackageReviewContractExpression::CollectionLength {
            collection: Box::new(child(member.receiver)?),
        });
    }
    if matches!(
        compilation.expression_table.expression(member.receiver),
        ExpressionNode::Name(path)
            if projection_substitutions.iter().any(|(symbol, actual)| {
                *symbol == path.symbol
                    && matches!(
                        compilation.expression_table.expression(*actual),
                        ExpressionNode::Call(_)
                    )
            })
    ) {
        let ExpressionNode::Name(path) = compilation.expression_table.expression(member.receiver)
        else {
            unreachable!()
        };
        let actual = projection_substitutions
            .iter()
            .find(|(symbol, _)| *symbol == path.symbol)
            .map(|(_, actual)| *actual)
            .expect("guarded projection substitution");
        let projection =
            exact_fact_call_projection(compilation, context, expression, actual, member)?;
        require_exact_checked_contract_nominal_member(
            compilation,
            context,
            expression,
            projection.field,
        )?;
        return project_contract_member_expression(
            compilation,
            context,
            child(actual)?,
            projection.field,
            None,
        );
    }
    if let ExpressionNode::Name(path) = compilation.expression_table.expression(member.receiver) {
        if substitutions
            .iter()
            .any(|(symbol, _)| *symbol == path.symbol)
        {
            require_exact_checked_contract_nominal_member(
                compilation,
                context,
                expression,
                member.member_symbol,
            )?;
            return project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                member.member_symbol,
                None,
            );
        }
        if checked_fact.is_none()
            && let Some(parameter) = context
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == path.symbol && member.case_variant.is_none())
        {
            let field = contract_parameter_field_symbol(
                compilation,
                parameter,
                member.member.as_str(),
            )
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition parameter member does not resolve through its declared carrier",
                    context.subject_kind, context.subject_name
                ))]
            })?;
            require_exact_checked_contract_nominal_member(compilation, context, expression, field)?;
            return project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                field,
                None,
            );
        }
    }
    if matches!(
        compilation.expression_table.expression(member.receiver),
        ExpressionNode::Call(_)
    ) {
        let projection =
            exact_fact_call_projection(compilation, context, expression, member.receiver, member)?;
        require_exact_checked_contract_nominal_member(
            compilation,
            context,
            expression,
            projection.field,
        )?;
        return project_contract_member_expression(
            compilation,
            context,
            child(member.receiver)?,
            projection.field,
            None,
        );
    }
    project_checked_or_computed_member(
        compilation,
        context,
        expression,
        member,
        checked_fact,
        child,
    )
}

fn project_checked_or_computed_member(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: ExpressionHandle,
    member: &TableMemberExpression,
    checked_fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    child: &impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let Some(checked_fact) = checked_fact else {
        return project_computed_contract_member_expression(
            compilation,
            context,
            expression,
            member,
            child(member.receiver)?,
        );
    };
    let Some((root_expression, mut source_members)) =
        contract_member_path_source(compilation, expression)
    else {
        return project_computed_contract_member_expression(
            compilation,
            context,
            expression,
            member,
            child(member.receiver)?,
        );
    };
    let data_subject_root = context.data_symbol.is_some_and(|data_symbol| {
        is_data_subject_field_expression(compilation, data_symbol, root_expression)
    });
    let root =
        contract_member_path_root(compilation, context, root_expression).ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract member path has no exact semantic root",
                context.subject_kind, context.subject_name
            ))]
        })?;
    let receiver = if data_subject_root {
        let ExpressionNode::Name(path) = compilation.expression_table.expression(root_expression)
        else {
            unreachable!("guarded data-subject name root")
        };
        let [field_name] = compilation.expression_table.name_path_members(path.members) else {
            unreachable!("guarded single data-subject field")
        };
        source_members.insert(0, field_name.clone());
        PackageReviewContractExpression::DomainSubject
    } else {
        child(root_expression)?
    };
    let member_path = checked_contract_member_path(
        compilation,
        context,
        checked_fact,
        expression,
        root,
        &source_members,
    )?;
    let selected_member = member_path.last().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract member path has no checked member coordinate",
            context.subject_kind, context.subject_name
        ))]
    })?;
    require_exact_checked_contract_nominal_member(
        compilation,
        context,
        expression,
        selected_member.1,
    )?;
    member_path
        .into_iter()
        .try_fold(receiver, |receiver, (case_variant, member_symbol)| {
            project_contract_member_expression(
                compilation,
                context,
                receiver,
                member_symbol,
                case_variant,
            )
        })
}
