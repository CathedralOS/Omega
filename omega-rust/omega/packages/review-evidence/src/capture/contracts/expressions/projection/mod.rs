//! Recursive contract-expression projection by semantic expression family.
//!
//! This entrance retains recursion depth, substitutions, and checked-fact
//! custody. Every family the reviewed format represents is projected in the
//! one match below; names, members, and casts delegate to their sibling
//! semantic projectors.

mod call_expression;
mod member_expression;

use super::atomic_loads::project_contract_atomic_load;
use super::case_membership::{checked_case_classifier, is_case_membership};
use super::casts::project_contract_cast;
use super::constructors::project_contract_constructor_expression;
use super::names::project_contract_name_expression;
use super::operators::{
    exact_checked_contract_operator_meaning, project_contract_binary_operator,
    project_contract_unary_operator,
};
use crate::capture::PackageReviewInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::types::review_signature_type_identity_with_binders_and_substitutions_and_lifetimes;
use crate::record::{
    PackageReviewContractExpression, PackageReviewContractOperatorMeaning,
    PackageReviewFloatLiteral, PackageReviewMatchArm, PackageReviewReferenceAccess,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_contract_expression(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: typed_trees::expression::ExpressionHandle,
    checked_fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    project_contract_expression_with_substitutions(
        compilation,
        context,
        binders,
        expression,
        &[],
        &[],
        checked_fact,
        depth,
    )
}

pub(crate) fn project_contract_expression_with_substitutions(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: typed_trees::expression::ExpressionHandle,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    projection_substitutions: &[(SymbolHandle, typed_trees::expression::ExpressionHandle)],
    checked_fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use typed_trees::expression::ExpressionNode;

    if depth >= 256 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract expression exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    let child = |expression| {
        project_contract_expression_with_substitutions(
            compilation,
            context,
            binders,
            expression,
            substitutions,
            projection_substitutions,
            checked_fact,
            depth + 1,
        )
    };
    let node = compilation.expression_table.expression(expression);
    match node {
        ExpressionNode::SavedSubject(_) => Err(vec![Diagnostic::error(
            "package contract projection requires a scoped structural-match capture representation; a saved runtime subject is not a free denotational value",
        )]),
        ExpressionNode::UndeterminedNaN(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contains a semantic NaN leaf without a supported canonical package-review contract expression",
            context.subject_kind, context.subject_name
        ))]),
        ExpressionNode::Match(dispatch) => {
            let subject = child(dispatch.subject)?;
            let mut arms = Vec::new();
            for arm in compilation.expression_table.match_arms(dispatch.arms) {
                let pattern = match arm.pattern {
                    typed_trees::expression::MatchPattern::Destructure(_) => {
                        return Err(vec![Diagnostic::error(
                            "package contract projection has no canonical structural-pattern record; refusing to erase payload bindings or guard obligations",
                        )]);
                    }
                    typed_trees::expression::MatchPattern::Value(pattern) => Some(child(pattern)?),
                    typed_trees::expression::MatchPattern::Wildcard => None,
                };
                arms.push(PackageReviewMatchArm {
                    pattern,
                    value: child(arm.value)?,
                });
            }
            Ok(PackageReviewContractExpression::Match {
                subject: Box::new(subject),
                arms,
            })
        }
        ExpressionNode::Boolean(value) => Ok(PackageReviewContractExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => {
            Ok(PackageReviewContractExpression::Integer(value.to_string()))
        }
        ExpressionNode::Float(value) => match value.landing() {
            Some(numerics::literals::FloatFormat::F32) => {
                Ok(PackageReviewContractExpression::Float(
                    PackageReviewFloatLiteral::F32(value.f32_bits()),
                ))
            }
            Some(numerics::literals::FloatFormat::F64) => {
                Ok(PackageReviewContractExpression::Float(
                    PackageReviewFloatLiteral::F64(value.landed_f64().to_bits()),
                ))
            }
            None => Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contains a float literal without an exact checked width landing",
                context.subject_kind, context.subject_name
            ))]),
        },
        ExpressionNode::ArrayLiteral(values) => compilation
            .expression_table
            .expression_handles(*values)
            .iter()
            .map(|value| child(*value))
            .collect::<Result<Vec<_>, _>>()
            .map(PackageReviewContractExpression::Array),
        ExpressionNode::StructLiteral(literal) => {
            project_contract_constructor_expression(compilation, context, literal, &child)
        }
        ExpressionNode::Atomic(atomic) => project_contract_atomic_load(context, atomic, &child),
        ExpressionNode::Range(range) => (|| {
            Ok(PackageReviewContractExpression::Range {
                start: range
                    .start
                    .is_valid()
                    .then(|| child(range.start))
                    .transpose()?
                    .map(Box::new),
                end: range
                    .end
                    .is_valid()
                    .then(|| child(range.end))
                    .transpose()?
                    .map(Box::new),
                end_inclusive: range.end_inclusive,
            })
        })(),
        ExpressionNode::String(value) => Ok(PackageReviewContractExpression::ByteSequence(
            value.to_vec(),
        )),
        ExpressionNode::ZeroValue(type_reference) => {
            review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                compilation,
                *type_reference,
                binders,
                context.lifetime_binders,
                &[],
                context.lifetime_substitutions,
            )
            .map(PackageReviewContractExpression::ZeroValue)
        }
        ExpressionNode::Borrow(reference) => (|| {
            Ok(PackageReviewContractExpression::Reference {
                access: match reference.access {
                    language_core::ReferenceAccess::Shared => PackageReviewReferenceAccess::Shared,
                    language_core::ReferenceAccess::Mutable => {
                        PackageReviewReferenceAccess::Mutable
                    }
                    language_core::ReferenceAccess::WriteOnly => {
                        PackageReviewReferenceAccess::WriteOnly
                    }
                },
                target: Box::new(child(reference.target)?),
            })
        })(),
        ExpressionNode::Indexed(indexed) => (|| {
            Ok(PackageReviewContractExpression::Indexed {
                meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
                collection: Box::new(child(indexed.collection)?),
                index: Box::new(child(indexed.index)?),
            })
        })(),
        ExpressionNode::Binary(binary) => (|| {
            if is_case_membership(compilation, expression, binary) {
                let case = checked_case_classifier(compilation, context, expression, binary)?;
                return Ok(PackageReviewContractExpression::CaseMembership {
                    subject: Box::new(child(binary.left)?),
                    case,
                });
            }
            let operator = project_contract_binary_operator(binary.operator).ok_or_else(|| {
                vec![Diagnostic::error(
                    "contract operator has no ordinary binary representation",
                )]
            })?;
            Ok(PackageReviewContractExpression::Binary {
                meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
                operator,
                left: Box::new(child(binary.left)?),
                right: Box::new(child(binary.right)?),
            })
        })(),
        ExpressionNode::Unary(unary) => (|| {
            if exact_checked_contract_operator_meaning(compilation, context, expression)?
                != PackageReviewContractOperatorMeaning::Builtin
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` unary contract operator is not one closed compiler-owned meaning",
                    context.subject_kind, context.subject_name
                ))]);
            }
            Ok(PackageReviewContractExpression::Unary {
                operator: project_contract_unary_operator(unary.operator),
                operand: Box::new(child(unary.operand)?),
            })
        })(),
        ExpressionNode::Call(call) => call_expression::project_call_expression(
            compilation,
            context,
            binders,
            expression,
            call,
            checked_fact,
            &child,
        ),
        ExpressionNode::Name(path) => project_contract_name_expression(
            compilation,
            context,
            binders,
            expression,
            path,
            substitutions,
            checked_fact,
        ),
        ExpressionNode::Member(member) => member_expression::project_member_expression(
            compilation,
            context,
            expression,
            member,
            substitutions,
            projection_substitutions,
            checked_fact,
            &child,
        ),
        ExpressionNode::Cast(cast) => {
            project_contract_cast(compilation, context, binders, cast, child)
        }
    }
}
