//! The progress demands a component is bound to, and the digest that pins them.

use crate::{
    SelectedProviderClosureDigest, SelectedProviderPlanFacts,
    provider_plan::{ProviderPlanDigest, ServiceProgressSubject},
};
use sha2::{Digest, Sha256};

/// Collision-resistant commitment to one exact component progress manifest.
///
/// The compact normalized identity remains available for diagnostics and
/// compatibility records. Installation admission must retain this commitment
/// (or the complete manifest) so a compact-equal substitute cannot authorize
/// a different selected provider closure or pending demand set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentProgressManifestDigest([u8; 32]);

impl ComponentProgressManifestDigest {
    const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Source-handle-free input derived from checked call facts before composition.
/// It names an obligation only; selected-plan identity is joined by
/// `ComponentProgressManifest::bind`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckedComponentProgressDemand {
    pub provider_service_identity: String,
    pub provider_service_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub requirement_identity: String,
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub profile_identity: String,
    pub subject_projections: Vec<String>,
    pub origin_callable_identity: String,
    pub origin_state_identity: String,
    pub statement_ordinal: usize,
    pub call_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentBuildBoundProgressDemand {
    pub provider_service_identity: String,
    pub provider_service_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub requirement_identity: String,
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub profile_identity: String,
    pub subject_projections: Vec<String>,
    pub establishment_routes: Vec<crate::provider_plan::ServiceProgressEstablishmentRoute>,
    /// Non-authoritative coordinate for the exact selected plan committed by
    /// `provider_plan_digest` and the enclosing selected-closure digest.
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub origin_callable_identity: String,
    pub origin_state_identity: String,
    pub statement_ordinal: usize,
    pub call_ordinal: usize,
}

/// Canonical component-level progress obligations for one exact selected entry
/// and provider closure. Pending rows are deliberately not receipts and carry
/// no mutable "discharged" bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentProgressManifest {
    entry_callable_identity: String,
    /// Non-authoritative compatibility/report coordinate. The selected
    /// closure digest below is the authority-bearing join.
    selected_provider_closure_report_identity: u64,
    selected_provider_closure_digest: SelectedProviderClosureDigest,
    pending: Vec<ComponentBuildBoundProgressDemand>,
    /// Non-authoritative compatibility/report coordinate.
    compatibility_report_identity: u64,
    identity_digest: ComponentProgressManifestDigest,
}

impl ComponentProgressManifest {
    pub fn bind(
        entry_callable_identity: String,
        selected: &SelectedProviderPlanFacts,
        demands: Vec<CheckedComponentProgressDemand>,
    ) -> Result<Self, String> {
        if entry_callable_identity.is_empty() {
            return Err(
                "component progress manifest requires an exact entry callable identity".into(),
            );
        }
        let mut pending = Vec::with_capacity(demands.len());
        for demand in demands {
            if demand.provider_service_identity.is_empty()
                || demand.requirement_identity.is_empty()
                || demand.profile_identity.is_empty()
                || demand.origin_callable_identity.is_empty()
                || demand.origin_state_identity.is_empty()
            {
                return Err("component progress demand contains an empty semantic identity".into());
            }
            let matching_plans = selected
                .plans()
                .iter()
                .filter(|plan| {
                    plan.schema.trait_name == demand.provider_service_identity
                        && plan.schema.trait_package_identity
                            == demand.provider_service_package_identity
                })
                .collect::<Vec<_>>();
            let [plan] = matching_plans.as_slice() else {
                return Err(format!(
                    "build-bound progress demand for service `{}` resolves to {} selected provider plans; expected exactly one",
                    demand.provider_service_identity,
                    matching_plans.len()
                ));
            };
            let matching_methods = plan
                .schema
                .methods
                .iter()
                .filter(|method| {
                    method.requirement_identity == demand.requirement_identity
                        && method.requirement_owner_package_identity
                            == demand.requirement_owner_package_identity
                })
                .collect::<Vec<_>>();
            let [method] = matching_methods.as_slice() else {
                return Err(format!(
                    "build-bound progress demand `{}` resolves to {} methods in selected provider plan `{}`; expected exactly one",
                    demand.requirement_identity,
                    matching_methods.len(),
                    plan.name
                ));
            };
            let matching_premises = method
                .termination_premises
                .iter()
                .filter(|premise| {
                    premise.subject == ServiceProgressSubject::ProviderReceiver
                        && premise.profile == demand.profile_identity
                        && premise.subject_projections == demand.subject_projections
                })
                .collect::<Vec<_>>();
            let [premise] = matching_premises.as_slice() else {
                return Err(format!(
                    "build-bound progress demand `{}` / `{}` has {} exact provider-receiver premise matches in selected provider plan `{}`; expected one",
                    demand.requirement_identity,
                    demand.profile_identity,
                    matching_premises.len(),
                    plan.name
                ));
            };
            let mut establishment_routes = premise.establishment_routes.clone();
            establishment_routes.sort();
            establishment_routes.dedup();
            pending.push(ComponentBuildBoundProgressDemand {
                provider_service_identity: demand.provider_service_identity,
                provider_service_package_identity: demand.provider_service_package_identity,
                requirement_identity: demand.requirement_identity,
                requirement_owner_package_identity: demand.requirement_owner_package_identity,
                profile_identity: demand.profile_identity,
                subject_projections: demand.subject_projections,
                establishment_routes,
                provider_plan_report_identity: plan.report_fingerprint(),
                provider_plan_digest: plan.identity_digest(),
                origin_callable_identity: demand.origin_callable_identity,
                origin_state_identity: demand.origin_state_identity,
                statement_ordinal: demand.statement_ordinal,
                call_ordinal: demand.call_ordinal,
            });
        }
        pending.sort();
        pending.dedup();
        let selected_provider_closure_report_identity = selected.compatibility_report_identity();
        let selected_provider_closure_digest = selected.identity_digest();
        let compatibility_report_identity = compatibility_report_fingerprint(
            &entry_callable_identity,
            selected_provider_closure_report_identity,
            &pending,
        );
        let identity_digest = digest_manifest(
            &entry_callable_identity,
            selected_provider_closure_digest,
            &pending,
        );
        Ok(Self {
            entry_callable_identity,
            selected_provider_closure_report_identity,
            selected_provider_closure_digest,
            pending,
            compatibility_report_identity,
            identity_digest,
        })
    }

    pub fn entry_callable_identity(&self) -> &str {
        &self.entry_callable_identity
    }

    /// Non-authoritative compact selected-closure coordinate. Admission also
    /// compares the closure digest.
    pub const fn selected_provider_closure_report_identity(&self) -> u64 {
        self.selected_provider_closure_report_identity
    }

    /// Strong authority join to the exact selected-provider closure used
    /// while binding this manifest.
    pub const fn selected_provider_closure_digest(&self) -> SelectedProviderClosureDigest {
        self.selected_provider_closure_digest
    }

    /// Replays both the report coordinate and collision-resistant commitment.
    pub fn matches_selected_provider_closure(&self, selected: &SelectedProviderPlanFacts) -> bool {
        self.selected_provider_closure_report_identity == selected.compatibility_report_identity()
            && self.selected_provider_closure_digest == selected.identity_digest()
    }

    pub fn pending(&self) -> &[ComponentBuildBoundProgressDemand] {
        &self.pending
    }

    /// Explicit name for the non-authoritative compact manifest coordinate.
    pub const fn compatibility_report_identity(&self) -> u64 {
        self.compatibility_report_identity
    }

    pub const fn identity_digest(&self) -> ComponentProgressManifestDigest {
        self.identity_digest
    }
}

fn digest_manifest(
    entry: &str,
    selected: SelectedProviderClosureDigest,
    pending: &[ComponentBuildBoundProgressDemand],
) -> ComponentProgressManifestDigest {
    let mut encoder = ManifestDigestEncoder::new();
    encoder.string(entry);
    encoder.bytes(selected.as_bytes());
    encoder.len(pending.len());
    for demand in pending {
        encoder.string(&demand.provider_service_identity);
        encoder.package_identity(demand.provider_service_package_identity);
        encoder.string(&demand.requirement_identity);
        encoder.package_identity(demand.requirement_owner_package_identity);
        encoder.string(&demand.profile_identity);
        encoder.strings(&demand.subject_projections);
        encoder.len(demand.establishment_routes.len());
        for route in &demand.establishment_routes {
            encoder.string(route.kind.as_str());
            encoder.string(&route.requirement_identity);
        }
        // This remains a compact plan coordinate, but the selected closure
        // commitment above binds the exact uniquely indexed plans.
        encoder.u64(demand.provider_plan_report_identity);
        encoder.bytes(demand.provider_plan_digest.as_bytes());
        encoder.string(&demand.origin_callable_identity);
        encoder.string(&demand.origin_state_identity);
        encoder.u64(demand.statement_ordinal as u64);
        encoder.u64(demand.call_ordinal as u64);
    }
    encoder.finish()
}

struct ManifestDigestEncoder(Sha256);

impl ManifestDigestEncoder {
    fn new() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.component-progress-manifest.sha256.v1\0");
        Self(digest)
    }

    fn finish(self) -> ComponentProgressManifestDigest {
        ComponentProgressManifestDigest::from_digest(self.0.finalize().into())
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }

    fn len(&mut self, len: usize) {
        self.u64(len as u64);
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    fn package_identity(&mut self, identity: Option<semantic_vocabulary::PackageKeyIdentity>) {
        match identity {
            Some(identity) => {
                self.0.update([1]);
                self.bytes(&identity.digest());
            }
            None => self.0.update([0]),
        }
    }
}

fn compatibility_report_fingerprint(
    entry: &str,
    selected: u64,
    pending: &[ComponentBuildBoundProgressDemand],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut write = |bytes: &[u8]| {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    write(entry.as_bytes());
    write(&selected.to_le_bytes());
    write(&(pending.len() as u64).to_le_bytes());
    for demand in pending {
        write(demand.provider_service_identity.as_bytes());
        match demand.provider_service_package_identity {
            Some(identity) => {
                write(&[1]);
                write(&identity.digest());
            }
            None => write(&[0]),
        }
        write(demand.requirement_identity.as_bytes());
        match demand.requirement_owner_package_identity {
            Some(identity) => {
                write(&[1]);
                write(&identity.digest());
            }
            None => write(&[0]),
        }
        write(demand.profile_identity.as_bytes());
        write(&(demand.subject_projections.len() as u64).to_le_bytes());
        for projection in &demand.subject_projections {
            write(projection.as_bytes());
        }
        write(&(demand.establishment_routes.len() as u64).to_le_bytes());
        for route in &demand.establishment_routes {
            write(route.kind.as_str().as_bytes());
            write(route.requirement_identity.as_bytes());
        }
        write(&demand.provider_plan_report_identity.to_le_bytes());
        write(demand.origin_callable_identity.as_bytes());
        write(demand.origin_state_identity.as_bytes());
        write(&(demand.statement_ordinal as u64).to_le_bytes());
        write(&(demand.call_ordinal as u64).to_le_bytes());
    }
    hash
}
