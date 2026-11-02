use crate::capture::PackageReviewInput;
use crate::capture::contracts::expressions::projection::project_contract_expression;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::owned_identity;
use crate::capture::semantics::types::review_type_identity_with_binders;
use crate::capture::source::ProjectedReviewRow;
use crate::record::{PackageReviewConstMeaning, PackageReviewConstShape};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use typed_trees::constant::ConstDeclaration;

pub(crate) fn project_public_consts<'a>(
    compilation: &'a PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<'a, PackageReviewConstShape, ConstDeclaration>>, Vec<Diagnostic>>
{
    let mut rows = Vec::new();
    for declaration in compilation
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let Some(identity) = owned_identity(compilation, package, declaration.symbol)? else {
            continue;
        };
        let mut originals = compilation
            .custody
            .const_declarations()
            .iter()
            .filter(|original| original.symbol == declaration.symbol);
        let original = originals.next().filter(|_| originals.next().is_none());
        let Some(original) = original else {
            return Err(vec![Diagnostic::error(format!(
                "public const `{}` has no unique immutable declaration",
                identity.path
            ))]);
        };
        if declaration.pending_target_observation != original.pending_target_observation
            || (declaration.pending_target_observation
                && declaration.authored_initializer != original.authored_initializer)
        {
            return Err(vec![Diagnostic::error(format!(
                "public const `{}` target dependency differs from immutable declaration custody",
                identity.path
            ))]);
        }
        let meaning = if declaration.pending_target_observation {
            if !declaration.authored_initializer.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "public const `{}` has no retained authored target-dependent initializer",
                    identity.path
                ))]);
            }
            // An initializer is implementation, even when its value is public.
            // It can depend on private types/callables without exporting them.
            // No runtime proof owner or target evaluation entry is invented.
            let context = ContractProjectionContext {
                subject_kind: "constant",
                subject_name: &identity.path,
                owner: checked_trees::ContractProofFactOwner::Unknown,
                point: facts::ProgramPoint::Definition {
                    symbol: declaration.symbol,
                },
                parameters: &[],
                domain_symbol: None,
                data_symbol: None,
                lifetime_binders: &[],
                lifetime_substitutions: &[],
                selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            };
            PackageReviewConstMeaning::TargetDependent(project_contract_expression(
                compilation,
                &context,
                &[],
                declaration.authored_initializer,
                None,
                0,
            )?)
        } else {
            let Some(value) = declaration.evaluated_value.as_ref() else {
                return Err(vec![Diagnostic::error(format!(
                    "public const `{}` has no canonical declaration value",
                    identity.path
                ))]);
            };
            // Semantic NaN is not a request for particular IEEE image bits.
            PackageReviewConstMeaning::Evaluated(match value {
                language_core::constant_value::EvaluatedConstant::Encoded(encoding) => {
                    encoding.clone()
                }
                language_core::constant_value::EvaluatedConstant::UndeterminedNaN(format) => {
                    format!("semantic-nan:{}", format.name())
                }
            })
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewConstShape {
                identity,
                declared_type: review_type_identity_with_binders(
                    compilation,
                    declaration.declared_type,
                    &[],
                )?,
                meaning,
            },
            declaration,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
