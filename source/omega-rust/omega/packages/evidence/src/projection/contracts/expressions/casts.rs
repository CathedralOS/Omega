use crate::evidence::{
    PackageReviewArithmeticDomain, PackageReviewCastForm, PackageReviewContractExpression,
};
use crate::projection::contracts::checked::facts::ContractProjectionContext;
use crate::projection::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::projection::semantics::types::review_type_identity_with_binders;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, TableCastExpression};

pub(crate) fn project_contract_cast(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    cast: &TableCastExpression,
    project_child: impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let semantic_domain = if cast.semantic_domain_symbol.is_valid() {
        let domain = compilation
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == cast.semantic_domain_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` cast refers to an unresolved semantic domain",
                    context.subject_kind, context.subject_name
                ))]
            })?;
        let identity = nominal_identity(compilation, domain.symbol)?;
        let reviewed_package = compilation.package_identity().ok_or_else(|| {
            vec![Diagnostic::error(
                "package review requires package-aware checked compilation",
            )]
        })?;
        if reviewed_package_owns(&identity, reviewed_package)? && !domain.is_public {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` exposes non-public semantic domain `{}` in a cast",
                context.subject_kind, context.subject_name, domain.name
            ))]);
        }
        Some(identity)
    } else {
        None
    };
    Ok(PackageReviewContractExpression::Cast {
        value: Box::new(project_child(cast.value)?),
        target: review_type_identity_with_binders(compilation, cast.target_type, binders)?,
        arithmetic_domain: match cast.domain {
            psi_numerics::arithmetic::ArithmeticDomain::Exact => {
                PackageReviewArithmeticDomain::Exact
            }
            psi_numerics::arithmetic::ArithmeticDomain::Wrapping => {
                PackageReviewArithmeticDomain::Wrapping
            }
            psi_numerics::arithmetic::ArithmeticDomain::Saturating => {
                PackageReviewArithmeticDomain::Saturating
            }
            psi_numerics::arithmetic::ArithmeticDomain::Trapping => {
                PackageReviewArithmeticDomain::Trapping
            }
        },
        semantic_domain,
        semantic_domain_arguments: compilation
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
            .iter()
            .map(|argument| review_type_identity_with_binders(compilation, *argument, binders))
            .collect::<Result<Vec<_>, _>>()?,
        form: match cast.form {
            psi_language_core::cast_form::CastForm::Value => PackageReviewCastForm::Value,
            psi_language_core::cast_form::CastForm::RecastShared => {
                PackageReviewCastForm::RecastShared
            }
            psi_language_core::cast_form::CastForm::RecastMutable => {
                PackageReviewCastForm::RecastMutable
            }
        },
    })
}
