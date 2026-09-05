//! Literal, container, reference, and other directly structural forms.

use super::super::atomic_loads::project_contract_atomic_load;
use super::super::constructors::project_contract_constructor_expression;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::types::review_signature_type_identity_with_binders_and_substitutions_and_lifetimes;
use crate::record::{
    PackageReviewContractExpression, PackageReviewFloatLiteral, PackageReviewReferenceAccess,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn project_value_form(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    node: &ExpressionNode,
    child: &impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Option<Result<PackageReviewContractExpression, Vec<Diagnostic>>> {
    match node {
        ExpressionNode::Boolean(value) => {
            Some(Ok(PackageReviewContractExpression::Boolean(*value)))
        }
        ExpressionNode::Integer(value) => Some(Ok(PackageReviewContractExpression::Integer(
            value.text().to_owned(),
        ))),
        ExpressionNode::Float(value) => Some(match value.landing() {
            Some(psi_numerics::literals::FloatFormat::F32) => {
                Ok(PackageReviewContractExpression::Float(
                    PackageReviewFloatLiteral::F32(value.f32_bits()),
                ))
            }
            Some(psi_numerics::literals::FloatFormat::F64) => {
                Ok(PackageReviewContractExpression::Float(
                    PackageReviewFloatLiteral::F64(value.landed_f64().to_bits()),
                ))
            }
            None => Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contains a float literal without an exact checked width landing",
                context.subject_kind, context.subject_name
            ))]),
        }),
        ExpressionNode::ArrayLiteral(values) => Some(
            compilation
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| child(*value))
                .collect::<Result<Vec<_>, _>>()
                .map(PackageReviewContractExpression::Array),
        ),
        ExpressionNode::StructLiteral(literal) => Some(project_contract_constructor_expression(
            compilation,
            context,
            literal,
            child,
        )),
        ExpressionNode::Atomic(atomic) => {
            Some(project_contract_atomic_load(context, atomic, child))
        }
        ExpressionNode::Range(range) => Some((|| {
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
        })()),
        ExpressionNode::String(value) => Some(Ok(PackageReviewContractExpression::ByteSequence(
            value.to_vec(),
        ))),
        ExpressionNode::ZeroValue(type_reference) => Some(
            review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                compilation,
                *type_reference,
                binders,
                context.lifetime_binders,
                &[],
                context.lifetime_substitutions,
            )
            .map(PackageReviewContractExpression::ZeroValue),
        ),
        ExpressionNode::Borrow(reference) => Some((|| {
            Ok(PackageReviewContractExpression::Reference {
                access: match reference.access {
                    psi_language_core::ReferenceAccess::Shared => {
                        PackageReviewReferenceAccess::Shared
                    }
                    psi_language_core::ReferenceAccess::Mutable => {
                        PackageReviewReferenceAccess::Mutable
                    }
                    psi_language_core::ReferenceAccess::WriteOnly => {
                        PackageReviewReferenceAccess::WriteOnly
                    }
                },
                target: Box::new(child(reference.target)?),
            })
        })()),
        _ => None,
    }
}
