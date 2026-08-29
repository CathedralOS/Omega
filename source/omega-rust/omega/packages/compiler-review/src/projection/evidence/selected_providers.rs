use super::source_locations::{canonical_source_location, canonical_source_span_location};
use crate::evidence::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn validate_selected_provider_declaration_owner(
    declaration: &PackageReviewNominalIdentity,
    expected_package: Option<PackageKeyIdentity>,
    plan_name: &str,
    role: &str,
) -> Result<(), Vec<Diagnostic>> {
    let matches = match (expected_package, declaration.owner) {
        (Some(expected), PackageReviewNominalOwner::Package(actual)) => expected == actual,
        (None, PackageReviewNominalOwner::ToolchainSource(_)) => true,
        (Some(_), PackageReviewNominalOwner::ToolchainSource(_))
        | (None, PackageReviewNominalOwner::Package(_))
        | (_, PackageReviewNominalOwner::Unresolved) => false,
    };
    if matches {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(format!(
            "selected provider plan `{plan_name}` {role} `{}` disagrees with its exact package/toolchain ownership",
            declaration.path,
        ))])
    }
}

pub(crate) fn selected_provider_row_source(
    compilation: &CheckedCompilation,
    selected_providers: &[CheckedPackageProviderReview],
) -> Result<PackageReviewCanonicalRowSource, Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_providers.len() || selected_plans.len() != provenance.len()
    {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }
    if selected_plans.is_empty() {
        return Ok(PackageReviewCanonicalRowSource::compiler_derived(
            PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
        ));
    }

    let mut locations = Vec::new();
    let mut compiler_derivations = Vec::new();
    for (index, plan) in selected_plans.iter().enumerate() {
        let retained = &provenance[index];
        if retained.plan != *plan {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` is not aligned with its retained provenance",
                plan.name,
            ))]);
        }

        match &retained.selected_by {
            omega_provider_planning::plans::ProviderSelectionProvenance::BuildOverride(declarations)
            | omega_provider_planning::plans::ProviderSelectionProvenance::TargetDefault(declarations) => {
                for declaration in declarations {
                    locations.push(canonical_source_span_location(
                        compilation,
                        declaration.source_span,
                        PackageReviewSourceLocationRole::ProviderSelection,
                    )?);
                }
            }
            omega_provider_planning::plans::ProviderSelectionProvenance::UniqueCoveringCandidate => {
                compiler_derivations
                    .push(PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection);
            }
        }

        locations.push(canonical_source_location(
            compilation,
            retained.provider.schema.symbol(),
            PackageReviewSourceLocationRole::ProviderSchemaDeclaration,
        )?);

        if let Some(provider_type) = retained.provider.provider_type {
            locations.push(canonical_source_location(
                compilation,
                provider_type,
                PackageReviewSourceLocationRole::ProviderTypeDeclaration,
            )?);
        } else {
            compiler_derivations.push(PackageReviewSyntheticSourceKind::FreeExternalProviderType);
        }

        for requirement in &retained.provider.row_requirements {
            locations.push(canonical_source_location(
                compilation,
                *requirement,
                PackageReviewSourceLocationRole::ProviderRequirementDeclaration,
            )?);
        }

        for realization in &retained.provider.row_realizations {
            locations.push(canonical_source_location(
                compilation,
                *realization,
                PackageReviewSourceLocationRole::ProviderRealization,
            )?);
        }
    }
    locations.sort();
    locations.dedup();
    compiler_derivations.sort();
    compiler_derivations.dedup();
    Ok(PackageReviewCanonicalRowSource::mixed(
        locations,
        compiler_derivations,
    ))
}
