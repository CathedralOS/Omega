//! Join stage 01’s admitted provider candidate rows to their exact plan requirements.

use super::declarations::unavailable;
use diagnostics::Diagnostic;
use build_providers::plans::{ProviderBinding, ServiceSchema};
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct CandidateRow {
    pub requirement: String,
    pub candidates: Vec<(BoundaryMachineId, MachineId)>,
}

#[derive(Clone)]
pub(super) struct ProviderChoice {
    pub package: Option<PackageKeyIdentity>,
    pub provider: String,
    pub target: String,
    pub schema: ServiceSchema,
    pub rows: Vec<CandidateRow>,
    pub method_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ChoiceKey<'a> {
    pub provider_package: Option<PackageKeyIdentity>,
    pub provider_identity: &'a str,
    pub service_package: Option<PackageKeyIdentity>,
    pub service_declaration: &'a str,
    pub service_application: Option<&'a str>,
}

impl ProviderChoice {
    pub fn key(&self) -> ChoiceKey<'_> {
        let schema = &self.schema.declaration;
        ChoiceKey {
            provider_package: self.package,
            provider_identity: &self.provider,
            service_package: schema.trait_package_identity,
            service_declaration: schema.identity_text(schema.trait_name),
            service_application: schema
                .receiver_applications
                .first()
                .filter(|root| {
                    !root.argument_identities.is_empty()
                        || !root.lifetime_argument_identities.is_empty()
                })
                .and_then(|_| schema.root_application_identity()),
        }
    }
}

pub(super) fn from_review(
    checked: &compiler::SelectedProviderResult,
) -> Result<Vec<ProviderChoice>, Vec<Diagnostic>> {
    let plans = checked.plans().plans();
    let checked_rows = match checked.package_review() {
        Some(review) => review
            .provider_candidates
            .as_ref()
            .map_err(Clone::clone)?
            .as_slice(),
        None if plans.is_empty() => &[],
        None => return Err(unavailable("the package's target review plan")),
    };
    let mut checked_rows = checked_rows.iter();
    let mut choices = Vec::with_capacity(plans.len());
    for plan in plans {
        let mut rows = Vec::new();
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter { .. } = row.binding else {
                continue;
            };
            let canonical = plan.identity_text(row.declared_requirement_identity);
            let reviewed = checked_rows
                .next()
                .filter(|reviewed| reviewed.plan == plan.name && reviewed.requirement == canonical)
                .ok_or_else(|| unavailable("one selected row for the exact service method"))?;
            rows.push(CandidateRow {
                requirement: canonical.to_owned(),
                candidates: reviewed.candidates.clone(),
            });
        }
        rows.sort_by(|a, b| a.requirement.cmp(&b.requirement));
        choices.push(ProviderChoice {
            package: plan.provider_type_package_identity,
            provider: plan.provider_type.clone(),
            target: plan.target.clone(),
            schema: plan.schema.clone(),
            rows,
            method_count: plan.rows.len(),
        });
    }
    if checked_rows.next().is_some() {
        return Err(unavailable("one selected row for the exact service method"));
    }
    Ok(choices)
}
