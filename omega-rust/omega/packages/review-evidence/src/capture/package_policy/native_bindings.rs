//! Capture private and unused native choices as explicit acceptance/audit rows.
use crate::capture::EvidenceOption;
use crate::capture::{PackageReviewInput, semantics::services};
use crate::record::PackagePolicyNativeBinding;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
pub(super) fn project(
    input: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyNativeBinding>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    let mut signatures = services::SignatureProjection::new(*input);
    for candidate in &input.custody.providers().native_build_plans {
        if candidate.plan.origin_package_identity != Some(package) {
            continue;
        }
        for (row, realization) in candidate
            .plan
            .rows
            .iter()
            .zip(&candidate.provenance.row_realizations)
        {
            let native = realization
                .native()
                .ok_or_else(|| vec![Diagnostic::error("native inventory lost its Build origin")])?;
            let method = candidate
                .plan
                .schema
                .declaration
                .methods
                .iter()
                .find(|method| {
                    candidate
                        .plan
                        .schema
                        .identity_text(method.requirement_identity)
                        == candidate.plan.identities.text(row.requirement_identity)
                })
                .or_evidence("native inventory lost its complete checked requirement")?;
            let method = services::project(
                &mut signatures,
                &candidate.plan.schema,
                candidate.provenance.schema,
                None,
                native.requirement,
                method,
                &candidate.plan.schema.declaration.identities,
            )?;
            for production in
                std::iter::once(&native.origin.primary).chain(&native.origin.equivalents)
            {
                let occurrence = provider_planning::native_bindings::NativeBuildRowOrigin {
                    requirement: native.requirement,
                    origin: package_compilation::PackageNativeBindingOrigin {
                        primary: production.clone(),
                        equivalents: Vec::new(),
                    },
                };
                let binding = crate::capture::providers::policy::bindings::project_native(
                    input,
                    &occurrence,
                    &row.binding,
                )?;
                rows.push(PackagePolicyNativeBinding {
                    method: method.clone(),
                    binding,
                });
            }
        }
    }
    rows.sort_by(|left, right| left.key().cmp(&right.key()));
    rows.dedup();
    if rows.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
        return Err(vec![Diagnostic::error(
            "native inventory conflicts at one exact producer and requirement",
        )]);
    }
    Ok(rows)
}
