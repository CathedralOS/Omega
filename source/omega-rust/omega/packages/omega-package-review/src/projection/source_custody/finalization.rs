use super::locations::{canonical_source_location, canonical_source_span_location};
use crate::evidence::projection::{
    ProjectedDangerousAuthorityRow, ProjectedDangerousAuthoritySlackRow, ProjectedReviewRow,
    ProjectedSemanticDependencyRow,
};
use crate::evidence::{
    PackageReviewCanonicalRowSource, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthoritySlack, PackageReviewSemanticDependency,
    PackageReviewSourceLocationRole,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn finalize_projected_rows<Row>(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedReviewRow<Row>>,
    role: PackageReviewSourceLocationRole,
) -> Result<(Vec<Row>, Vec<PackageReviewCanonicalRowSource>), Vec<Diagnostic>> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![canonical_source_location(
            compilation,
            projected.declaration,
            role,
        )?];
        for nested in projected.nested_source_locations {
            locations.push(canonical_source_span_location(
                compilation,
                nested.source_span,
                nested.role,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn finalize_semantic_dependency_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedSemanticDependencyRow>,
) -> Result<
    (
        Vec<PackageReviewSemanticDependency>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = Vec::new();
        for declaration in projected.consumer_declarations {
            locations.push(canonical_source_location(
                compilation,
                declaration,
                PackageReviewSourceLocationRole::SemanticDependencyConsumer,
            )?);
        }
        for declaration in projected.dependency_declarations {
            locations.push(canonical_source_location(
                compilation,
                declaration,
                PackageReviewSourceLocationRole::SemanticDependencyDeclaration,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn finalize_dangerous_authority_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedDangerousAuthorityRow>,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthority>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![canonical_source_location(
            compilation,
            projected.declaration,
            PackageReviewSourceLocationRole::AuthorityDeclaration,
        )?];
        for exposure in projected.exposures {
            locations.push(canonical_source_location(
                compilation,
                exposure,
                PackageReviewSourceLocationRole::AuthorityExposure,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn finalize_dangerous_authority_slack_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedDangerousAuthoritySlackRow>,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthoritySlack>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![
            canonical_source_location(
                compilation,
                projected.authority_declaration,
                PackageReviewSourceLocationRole::AuthorityDeclaration,
            )?,
            canonical_source_location(
                compilation,
                projected.callable_declaration,
                PackageReviewSourceLocationRole::AuthorityExposure,
            )?,
        ];
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}
