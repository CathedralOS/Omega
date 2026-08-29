use crate::evidence::{
    PackageReviewConstShape, PackageReviewSourceLocationRole, ProjectedNestedSourceLocation,
    ProjectedReviewRow,
};
use crate::projection::exact_identity::{
    nominal_identity, review_type_identity_with_binders, reviewed_package_owns,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_public_consts(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewConstShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for declaration in compilation
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let Some(canonical_value_encoding) = declaration.canonical_value_encoding.clone() else {
            return Err(vec![Diagnostic::error(format!(
                "public const `{}` has no canonical declaration value",
                identity.path
            ))]);
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
            declaration: declaration.symbol,
            nested_source_locations: vec![ProjectedNestedSourceLocation {
                source_span: declaration.initializer_source_span,
                role: PackageReviewSourceLocationRole::ConstInitializer,
            }],
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
