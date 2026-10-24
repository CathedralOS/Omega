use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::{
    is_product_scope_instance, nominal_identity, reviewed_package_owns,
};
use crate::capture::semantics::types::review_type_identity_with_binders;
use crate::capture::source::ProjectedReviewRow;
use crate::record::PackageReviewConstShape;
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
        .filter(|declaration| {
            declaration.is_public
                && is_product_scope_instance(&compilation.typed.symbols, declaration.symbol)
        })
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let Some(value) = declaration.evaluated_value.as_ref() else {
            return Err(vec![Diagnostic::error(format!(
                "public const `{}` has no canonical declaration value",
                identity.path
            ))]);
        };
        // Review identity records the semantic value, not a request for image
        // bytes or a generic-index atom. This tag cannot decode as float bits.
        let canonical_value_encoding = match value {
            language_core::constant_value::EvaluatedConstant::Encoded(encoding) => encoding.clone(),
            language_core::constant_value::EvaluatedConstant::UndeterminedNaN(format) => {
                format!("semantic-nan:{}", format.name())
            }
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewConstShape {
                identity,
                declared_type: review_type_identity_with_binders(
                    compilation,
                    declaration.declared_type,
                    &[],
                )?,
                canonical_value_encoding,
            },
            declaration,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
