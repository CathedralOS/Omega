//! Preserve private and unused native choices from the original candidate
//! inventory. Selection and demand do not erase a package's Build promises.
use crate::capture::providers::policy::bindings;
use crate::capture::semantics::services;
use crate::capture::{PackageReviewInput, refusal};
use crate::record::PackagePolicyNativeBinding;
use diagnostics::Diagnostic;
use provider_planning::ProviderRowOrigin;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    input: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyNativeBinding>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    let mut signatures = services::SignatureProjection::new(*input);
    for candidate in &input.custody.providers().provider_candidates {
        if candidate.plan.origin_package_identity != Some(package) {
            continue;
        }
        for ((row, &requirement), &origin) in candidate
            .plan
            .rows
            .iter()
            .zip(&candidate.provenance.row_requirements)
            .zip(&candidate.provenance.row_realizations)
        {
            let ProviderRowOrigin::BuildNative { receipt, .. } = origin else {
                continue;
            };
            let admitted = bindings::native_row(input, requirement, receipt)?;
            let method = candidate
                .plan
                .schema
                .methods
                .iter()
                .find(|method| {
                    candidate
                        .plan
                        .schema
                        .identity_text(method.requirement_identity)
                        == candidate.plan.identities.text(row.requirement_identity)
                })
                .ok_or_else(|| {
                    rejected("native inventory lost its complete checked requirement")
                })?;
            rows.push(PackagePolicyNativeBinding {
                method: services::project_native(
                    &mut signatures,
                    &candidate.plan.schema,
                    candidate.provenance.schema,
                    requirement,
                    method,
                    &candidate.plan.schema.identities,
                    admitted,
                )?,
                application: crate::record::PackagePolicyNativeApplication::from(
                    provider_planning::resolve_native_binding_application(
                        input.checking_input.typed_program(),
                        admitted,
                    )
                    .map_err(|reason| rejected(&reason))?
                    .key,
                ),
                producer: bindings::native_producer(input, admitted)?,
                binding: bindings::project(
                    input,
                    &candidate.plan.identities,
                    &row.binding,
                    requirement,
                    origin,
                    false,
                )?,
            });
        }
    }
    rows.sort_by(|left, right| left.key().cmp(&right.key()));
    rows.dedup();
    if rows.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
        return Err(rejected(
            "native inventory conflicts at one exact producer and requirement",
        ));
    }
    Ok(rows)
}

refusal!(reason, "package policy rejects {reason}");
