use std::collections::{BTreeMap, BTreeSet};

use omega_effects::{
    ComponentBuildBoundProgressDemand, ComponentProgressManifest, SelectedProviderClosureDigest,
    SelectedProviderPlanFacts,
    provider_plan::{ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind},
};
use omega_executable_installation::{InstalledCode, InstalledCodeContext};

use crate::{
    ExternalRootDiagnostic, InstalledProviderOccurrenceId, InstalledRootLedger,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId,
    ProviderOccurrenceInstallationReceiptId,
};

/// Provider-reported installation fact for one concrete provider value.
///
/// This is an attestation input, not installation authority. Only the
/// canonical installed-root ledger can join it to the selected provider
/// closure and mint an [`InstalledProviderOccurrence`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOccurrenceInstallationReceipt {
    identity: ProviderOccurrenceInstallationReceiptId,
    installed: InstalledCodeContext,
    occurrence: InstalledProviderOccurrenceId,
    provider_identity: String,
}

impl ProviderOccurrenceInstallationReceipt {
    pub fn from_provider(
        identity: ProviderOccurrenceInstallationReceiptId,
        installed: &InstalledCode,
        occurrence: InstalledProviderOccurrenceId,
        provider_identity: impl Into<String>,
    ) -> Self {
        Self {
            identity,
            installed: installed.receipt_context(),
            occurrence,
            provider_identity: provider_identity.into(),
        }
    }

    pub const fn identity(&self) -> ProviderOccurrenceInstallationReceiptId {
        self.identity
    }

    pub const fn occurrence(&self) -> InstalledProviderOccurrenceId {
        self.occurrence
    }

    pub fn provider_identity(&self) -> &str {
        &self.provider_identity
    }
}

/// Non-authoritative association between one selected provider plan and a
/// provider-reported occurrence. The complete selected closure is sealed
/// atomically, so omitting or padding this list cannot create an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOccurrencePlanBinding {
    provider_plan_identity: u64,
    receipt: ProviderOccurrenceInstallationReceipt,
}

impl ProviderOccurrencePlanBinding {
    pub fn new(
        provider_plan_identity: u64,
        receipt: ProviderOccurrenceInstallationReceipt,
    ) -> Self {
        Self {
            provider_plan_identity,
            receipt,
        }
    }

    pub const fn provider_plan_identity(&self) -> u64 {
        self.provider_plan_identity
    }

    pub const fn receipt(&self) -> &ProviderOccurrenceInstallationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledProviderOccurrenceEvidence {
    installed: InstalledCodeContext,
    occurrence: InstalledProviderOccurrenceId,
    installation_receipt: ProviderOccurrenceInstallationReceiptId,
    provider_identity: String,
}

/// Opaque exact occurrence of one provider value in one installed artifact.
/// Compact identities are reporting keys only; exact installed-code evidence
/// remains private in the carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderOccurrence {
    evidence: InstalledProviderOccurrenceEvidence,
}

impl InstalledProviderOccurrence {
    pub const fn identity(&self) -> InstalledProviderOccurrenceId {
        self.evidence.occurrence
    }

    pub const fn installation_receipt(&self) -> ProviderOccurrenceInstallationReceiptId {
        self.evidence.installation_receipt
    }

    pub fn provider_identity(&self) -> &str {
        &self.evidence.provider_identity
    }
}

/// Complete selected-plan to exact-provider-occurrence closure for one
/// installed-code occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderOccurrenceClosure {
    selected: SelectedProviderPlanFacts,
    by_plan: BTreeMap<u64, InstalledProviderOccurrence>,
    by_occurrence: BTreeMap<InstalledProviderOccurrenceId, InstalledProviderOccurrence>,
    non_authoritative_report_fingerprint: u64,
}

impl InstalledProviderOccurrenceClosure {
    pub const fn selected(&self) -> &SelectedProviderPlanFacts {
        &self.selected
    }

    pub const fn selected_provider_closure_identity(&self) -> u64 {
        self.selected.normalized_identity()
    }

    /// Explicitly non-authoritative compatibility/report coordinate.
    pub const fn selected_provider_closure_report_identity(&self) -> u64 {
        self.selected.compatibility_report_identity()
    }

    pub fn selected_provider_closure_digest(&self) -> SelectedProviderClosureDigest {
        self.selected.identity_digest()
    }

    pub fn occurrence_for_plan(
        &self,
        provider_plan_identity: u64,
    ) -> Option<&InstalledProviderOccurrence> {
        self.by_plan.get(&provider_plan_identity)
    }

    pub fn occurrence(
        &self,
        identity: InstalledProviderOccurrenceId,
    ) -> Option<&InstalledProviderOccurrence> {
        self.by_occurrence.get(&identity)
    }

    pub fn occurrences(&self) -> impl ExactSizeIterator<Item = &InstalledProviderOccurrence> {
        self.by_occurrence.values()
    }

    /// Explicitly non-authoritative report/cache coordinate. The complete
    /// selected facts and occurrence evidence above are the replay authority.
    pub const fn non_authoritative_report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

/// Provider attestation that one authorized boundary grant established a
/// progress-profile qualification for one exact installed subject occurrence.
/// The issuer and subject are independent occurrences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressProfileEstablishmentAttestation {
    receipt: ProgressProfileEstablishmentReceiptId,
    installed: InstalledCodeContext,
    subject: InstalledProviderOccurrenceId,
    issuer: InstalledProviderOccurrenceId,
    issuer_provider_plan_identity: u64,
    grant_invocation: ProgressProfileGrantInvocationId,
    profile_identity: String,
    subject_projections: Vec<String>,
    route: ServiceProgressEstablishmentRoute,
}

impl ProgressProfileEstablishmentAttestation {
    #[allow(clippy::too_many_arguments)]
    pub fn from_provider(
        receipt: ProgressProfileEstablishmentReceiptId,
        installed: &InstalledCode,
        subject: InstalledProviderOccurrenceId,
        issuer: InstalledProviderOccurrenceId,
        issuer_provider_plan_identity: u64,
        grant_invocation: ProgressProfileGrantInvocationId,
        profile_identity: impl Into<String>,
        subject_projections: Vec<String>,
        route: ServiceProgressEstablishmentRoute,
    ) -> Self {
        Self {
            receipt,
            installed: installed.receipt_context(),
            subject,
            issuer,
            issuer_provider_plan_identity,
            grant_invocation,
            profile_identity: profile_identity.into(),
            subject_projections,
            route,
        }
    }
}

/// Ledger-admitted, non-forgeable establishment evidence. It is cloneable
/// because a proposition may discharge several exact call-site demands; its
/// grant invocation and subject remain fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "admitted progress establishment evidence must be retained for component closure"]
pub struct AdmittedProgressProfileEstablishment {
    receipt: ProgressProfileEstablishmentReceiptId,
    installed: InstalledCodeContext,
    selected_provider_closure_report_identity: u64,
    selected_provider_closure_digest: SelectedProviderClosureDigest,
    subject: InstalledProviderOccurrenceEvidence,
    issuer: InstalledProviderOccurrenceEvidence,
    issuer_provider_plan_identity: u64,
    grant_invocation: ProgressProfileGrantInvocationId,
    profile_identity: String,
    subject_projections: Vec<String>,
    route: ServiceProgressEstablishmentRoute,
}

impl AdmittedProgressProfileEstablishment {
    pub const fn receipt(&self) -> ProgressProfileEstablishmentReceiptId {
        self.receipt
    }

    pub const fn subject(&self) -> InstalledProviderOccurrenceId {
        self.subject.occurrence
    }

    pub const fn issuer(&self) -> InstalledProviderOccurrenceId {
        self.issuer.occurrence
    }

    pub const fn issuer_provider_plan_identity(&self) -> u64 {
        self.issuer_provider_plan_identity
    }

    pub const fn grant_invocation(&self) -> ProgressProfileGrantInvocationId {
        self.grant_invocation
    }

    pub fn profile_identity(&self) -> &str {
        &self.profile_identity
    }

    pub fn subject_projections(&self) -> &[String] {
        &self.subject_projections
    }

    pub const fn route(&self) -> &ServiceProgressEstablishmentRoute {
        &self.route
    }

    pub const fn selected_provider_closure_digest(&self) -> SelectedProviderClosureDigest {
        self.selected_provider_closure_digest
    }
}

#[derive(Debug)]
pub struct ProgressProfileEstablishmentAdmissionError {
    attestation: ProgressProfileEstablishmentAttestation,
    diagnostic: ExternalRootDiagnostic,
}

impl ProgressProfileEstablishmentAdmissionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_attestation(self) -> ProgressProfileEstablishmentAttestation {
        self.attestation
    }
}

impl std::fmt::Display for ProgressProfileEstablishmentAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ProgressProfileEstablishmentAdmissionError {}

/// Canonical source-free identity of one pending demand row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentProgressDemandIdentity(ComponentBuildBoundProgressDemand);

impl ComponentProgressDemandIdentity {
    pub fn from_demand(demand: &ComponentBuildBoundProgressDemand) -> Self {
        Self(demand.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentProgressReceiptBinding {
    demand: ComponentProgressDemandIdentity,
    receipt: AdmittedProgressProfileEstablishment,
}

impl ComponentProgressReceiptBinding {
    pub fn new(
        demand: ComponentProgressDemandIdentity,
        receipt: AdmittedProgressProfileEstablishment,
    ) -> Self {
        Self { demand, receipt }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledComponentProgressBinding {
    demand: ComponentProgressDemandIdentity,
    subject: InstalledProviderOccurrenceEvidence,
    receipt: AdmittedProgressProfileEstablishment,
}

/// Opaque acceptance that every pending row of one exact component manifest
/// was resolved against the installed selected-provider closure and admitted
/// receipt evidence. The original manifest remains retained; no forgeable
/// `discharged` bit replaces its obligations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "installed component progress closure must be retained through publication"]
pub struct InstalledComponentProgressClosure {
    installed: InstalledCodeContext,
    selected_provider_plans: Vec<u64>,
    manifest: ComponentProgressManifest,
    bindings: Vec<InstalledComponentProgressBinding>,
    non_authoritative_report_fingerprint: u64,
}

impl InstalledComponentProgressClosure {
    /// Test whether this acceptance belongs to one exact installed-code
    /// occurrence. The complete opaque installation context participates in
    /// the comparison; compact installed/artifact identities are insufficient.
    pub fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed == installed.receipt_context()
    }

    /// Canonical complete selected-plan set sealed by the installation
    /// registry. Runnable publication joins this set to the terminal
    /// installation record rather than reconstructing it from demand rows.
    pub fn selected_provider_plans(&self) -> &[u64] {
        &self.selected_provider_plans
    }

    pub const fn manifest(&self) -> &ComponentProgressManifest {
        &self.manifest
    }

    /// Explicitly non-authoritative report/cache coordinate. Publication must
    /// retain this complete opaque acceptance and its manifest commitment.
    pub const fn non_authoritative_report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    pub fn receipts(&self) -> impl ExactSizeIterator<Item = &AdmittedProgressProfileEstablishment> {
        self.bindings.iter().map(|binding| &binding.receipt)
    }
}

impl omega_installation_evidence::ComponentProgressAcceptanceEvidence
    for InstalledComponentProgressClosure
{
    fn component_progress_manifest_identity(&self) -> u64 {
        self.manifest.normalized_identity()
    }

    fn component_progress_acceptance_identity(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

#[derive(Debug)]
pub struct ComponentProgressSealError {
    manifest: ComponentProgressManifest,
    bindings: Vec<ComponentProgressReceiptBinding>,
    diagnostic: ExternalRootDiagnostic,
}

impl ComponentProgressSealError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ComponentProgressManifest,
        Vec<ComponentProgressReceiptBinding>,
    ) {
        (self.manifest, self.bindings)
    }
}

impl std::fmt::Display for ComponentProgressSealError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ComponentProgressSealError {}

impl InstalledRootLedger {
    /// Seal the complete selected provider-plan closure to exact installed
    /// occurrences. The operation is transactional and one-shot.
    pub fn seal_provider_occurrence_closure(
        &mut self,
        selected: &SelectedProviderPlanFacts,
        bindings: impl IntoIterator<Item = ProviderOccurrencePlanBinding>,
    ) -> Result<&InstalledProviderOccurrenceClosure, ExternalRootDiagnostic> {
        if self.provider_occurrence_closure.is_some() {
            return Err(ExternalRootDiagnostic(
                "installed provider-occurrence closure was already sealed".into(),
            ));
        }
        let bindings = bindings.into_iter().collect::<Vec<_>>();
        let expected = selected
            .plans()
            .iter()
            .map(|plan| plan.identity_fingerprint())
            .collect::<BTreeSet<_>>();
        let supplied = bindings
            .iter()
            .map(|binding| binding.provider_plan_identity)
            .collect::<BTreeSet<_>>();
        if bindings.len() != supplied.len() {
            return Err(ExternalRootDiagnostic(
                "provider-occurrence closure contains a duplicate selected plan".into(),
            ));
        }
        if supplied != expected {
            return Err(ExternalRootDiagnostic(
                "provider-occurrence closure does not exactly cover the selected provider plans"
                    .into(),
            ));
        }

        let mut by_plan = BTreeMap::new();
        let mut by_occurrence = BTreeMap::new();
        let mut receipt_owners = BTreeMap::new();
        for binding in bindings {
            let plan = selected
                .plan_by_identity(binding.provider_plan_identity)
                .expect("exact selected-plan set was checked above");
            let receipt = binding.receipt;
            if receipt.installed != self.installed_context {
                return Err(ExternalRootDiagnostic(
                    "provider occurrence receipt names a different installed-code occurrence"
                        .into(),
                ));
            }
            if receipt.provider_identity.is_empty() {
                return Err(ExternalRootDiagnostic(
                    "provider occurrence receipt has an empty nominal provider identity".into(),
                ));
            }
            if !plan.provider_type.is_empty() && receipt.provider_identity != plan.provider_type {
                return Err(ExternalRootDiagnostic(format!(
                    "provider occurrence `{}` cannot realize selected plan `{}` for provider type `{}`",
                    receipt.provider_identity, plan.name, plan.provider_type
                )));
            }
            if let Some(owner) = receipt_owners.insert(receipt.identity, receipt.occurrence)
                && owner != receipt.occurrence
            {
                return Err(ExternalRootDiagnostic(
                    "one provider installation receipt names several occurrences".into(),
                ));
            }
            let evidence = InstalledProviderOccurrenceEvidence {
                installed: receipt.installed,
                occurrence: receipt.occurrence,
                installation_receipt: receipt.identity,
                provider_identity: receipt.provider_identity,
            };
            let occurrence = InstalledProviderOccurrence { evidence };
            if let Some(existing) = by_occurrence.get(&occurrence.identity())
                && existing != &occurrence
            {
                return Err(ExternalRootDiagnostic(
                    "one installed provider occurrence has divergent admission evidence".into(),
                ));
            }
            by_occurrence.insert(occurrence.identity(), occurrence.clone());
            by_plan.insert(binding.provider_plan_identity, occurrence);
        }
        let non_authoritative_report_fingerprint =
            non_authoritative_provider_occurrence_report_fingerprint(selected, &by_plan);
        self.provider_occurrence_closure = Some(InstalledProviderOccurrenceClosure {
            selected: selected.clone(),
            by_plan,
            by_occurrence,
            non_authoritative_report_fingerprint,
        });
        Ok(self
            .provider_occurrence_closure
            .as_ref()
            .expect("provider occurrence closure was just installed"))
    }

    pub const fn provider_occurrence_closure(&self) -> Option<&InstalledProviderOccurrenceClosure> {
        self.provider_occurrence_closure.as_ref()
    }

    /// Admit one provider-issued progress-profile establishment receipt after
    /// replaying its subject, issuer, exact route, and grant invocation against
    /// this installation's provider-occurrence closure.
    pub fn admit_progress_profile_establishment(
        &mut self,
        attestation: ProgressProfileEstablishmentAttestation,
    ) -> Result<AdmittedProgressProfileEstablishment, ProgressProfileEstablishmentAdmissionError>
    {
        let fail = |attestation, message: String| ProgressProfileEstablishmentAdmissionError {
            attestation,
            diagnostic: ExternalRootDiagnostic(message),
        };
        let Some(closure) = self.provider_occurrence_closure.as_ref() else {
            return Err(fail(
                attestation,
                "progress-profile establishment requires a sealed provider-occurrence closure"
                    .into(),
            ));
        };
        if attestation.installed != self.installed_context {
            return Err(fail(
                attestation,
                "progress-profile attestation names a different installed-code occurrence".into(),
            ));
        }
        if attestation.profile_identity.is_empty() {
            return Err(fail(
                attestation,
                "progress-profile attestation has an empty profile identity".into(),
            ));
        }
        if attestation.route.kind != ServiceProgressEstablishmentRouteKind::BoundaryRequirement {
            return Err(fail(
                attestation,
                "progress-profile establishment requires an admitted boundary requirement route"
                    .into(),
            ));
        }
        let Some(subject) = closure.occurrence(attestation.subject).cloned() else {
            return Err(fail(
                attestation,
                "progress-profile attestation subject is not installed in this closure".into(),
            ));
        };
        let Some(issuer) = closure.occurrence(attestation.issuer).cloned() else {
            return Err(fail(
                attestation,
                "progress-profile attestation issuer is not installed in this closure".into(),
            ));
        };
        let Some(plan_occurrence) =
            closure.occurrence_for_plan(attestation.issuer_provider_plan_identity)
        else {
            return Err(fail(
                attestation,
                "progress-profile attestation names an unselected issuer provider plan".into(),
            ));
        };
        if plan_occurrence != &issuer {
            return Err(fail(
                attestation,
                "progress-profile attestation issuer occurrence does not realize its named plan"
                    .into(),
            ));
        }
        let issuer_plan = closure
            .selected
            .plan_by_identity(attestation.issuer_provider_plan_identity)
            .expect("installed plan binding came from selected facts");
        let exact_routes = issuer_plan
            .rows
            .iter()
            .filter(|row| row.requirement_identity == attestation.route.requirement_identity)
            .count();
        if exact_routes != 1 {
            return Err(fail(
                attestation,
                "progress-profile attestation route is not one exact requirement realized by the issuer plan"
                    .into(),
            ));
        }

        let admitted = AdmittedProgressProfileEstablishment {
            receipt: attestation.receipt,
            installed: attestation.installed,
            selected_provider_closure_report_identity: closure
                .selected
                .compatibility_report_identity(),
            selected_provider_closure_digest: closure.selected.identity_digest(),
            subject: subject.evidence,
            issuer: issuer.evidence,
            issuer_provider_plan_identity: attestation.issuer_provider_plan_identity,
            grant_invocation: attestation.grant_invocation,
            profile_identity: attestation.profile_identity,
            subject_projections: attestation.subject_projections,
            route: attestation.route,
        };
        if let Some(existing) = self.admitted_progress_receipts.get(&admitted.receipt) {
            return if existing == &admitted {
                Ok(existing.clone())
            } else {
                Err(fail(
                    ProgressProfileEstablishmentAttestation {
                        receipt: admitted.receipt,
                        installed: admitted.installed,
                        subject: admitted.subject.occurrence,
                        issuer: admitted.issuer.occurrence,
                        issuer_provider_plan_identity: admitted.issuer_provider_plan_identity,
                        grant_invocation: admitted.grant_invocation,
                        profile_identity: admitted.profile_identity,
                        subject_projections: admitted.subject_projections,
                        route: admitted.route,
                    },
                    "progress-profile receipt identity was replayed with divergent evidence".into(),
                ))
            };
        }
        let invocation = (admitted.issuer.occurrence, admitted.grant_invocation);
        if self.admitted_progress_invocations.contains_key(&invocation) {
            return Err(fail(
                ProgressProfileEstablishmentAttestation {
                    receipt: admitted.receipt,
                    installed: admitted.installed,
                    subject: admitted.subject.occurrence,
                    issuer: admitted.issuer.occurrence,
                    issuer_provider_plan_identity: admitted.issuer_provider_plan_identity,
                    grant_invocation: admitted.grant_invocation,
                    profile_identity: admitted.profile_identity,
                    subject_projections: admitted.subject_projections,
                    route: admitted.route,
                },
                "one progress-profile grant invocation cannot mint several receipts".into(),
            ));
        }
        self.admitted_progress_invocations
            .insert(invocation, admitted.receipt);
        self.admitted_progress_receipts
            .insert(admitted.receipt, admitted.clone());
        Ok(admitted)
    }

    /// Atomically discharge every exact pending row of one component manifest.
    /// Extra, missing, duplicate, substituted, or foreign-ledger bindings
    /// reject before the one-shot acceptance is committed.
    pub fn seal_component_progress(
        &mut self,
        manifest: ComponentProgressManifest,
        bindings: impl IntoIterator<Item = ComponentProgressReceiptBinding>,
    ) -> Result<InstalledComponentProgressClosure, ComponentProgressSealError> {
        let bindings = bindings.into_iter().collect::<Vec<_>>();
        let fail = |manifest, bindings, message: String| ComponentProgressSealError {
            manifest,
            bindings,
            diagnostic: ExternalRootDiagnostic(message),
        };
        let Some(closure) = self.provider_occurrence_closure.as_ref() else {
            return Err(fail(
                manifest,
                bindings,
                "component progress sealing requires a provider-occurrence closure".into(),
            ));
        };
        if !manifest.matches_selected_provider_closure(&closure.selected) {
            return Err(fail(
                manifest,
                bindings,
                "component progress manifest names a different selected provider closure".into(),
            ));
        }
        if self
            .accepted_component_progress
            .iter()
            .any(|accepted| accepted == &manifest)
        {
            return Err(fail(
                manifest,
                bindings,
                "component progress manifest was already sealed for this installation".into(),
            ));
        }
        let expected = manifest
            .pending()
            .iter()
            .map(ComponentProgressDemandIdentity::from_demand)
            .collect::<BTreeSet<_>>();
        let supplied = bindings
            .iter()
            .map(|binding| binding.demand.clone())
            .collect::<BTreeSet<_>>();
        if bindings.len() != supplied.len() || supplied != expected {
            return Err(fail(
                manifest,
                bindings,
                "component progress bindings do not exactly cover the pending demand rows".into(),
            ));
        }

        let supplied = bindings
            .iter()
            .map(|binding| (binding.demand.clone(), &binding.receipt))
            .collect::<BTreeMap<_, _>>();
        let mut accepted = Vec::with_capacity(manifest.pending().len());
        for demand in manifest.pending() {
            let demand_identity = ComponentProgressDemandIdentity::from_demand(demand);
            let receipt = supplied
                .get(&demand_identity)
                .expect("exact binding set was checked above");
            let Some(known_receipt) = self.admitted_progress_receipts.get(&receipt.receipt) else {
                return Err(fail(
                    manifest,
                    bindings,
                    "component progress binding uses a receipt not admitted by this installation"
                        .into(),
                ));
            };
            if known_receipt != *receipt {
                return Err(fail(
                    manifest,
                    bindings,
                    "component progress binding substitutes divergent receipt evidence".into(),
                ));
            }
            let Some(subject) = closure.occurrence_for_plan(demand.provider_plan_identity) else {
                return Err(fail(
                    manifest,
                    bindings,
                    "component progress demand names an uninstalled provider plan".into(),
                ));
            };
            if receipt.subject != subject.evidence
                || receipt.selected_provider_closure_report_identity
                    != manifest.selected_provider_closure_identity()
                || receipt.selected_provider_closure_digest
                    != manifest.selected_provider_closure_digest()
                || receipt.profile_identity != demand.profile_identity
                || receipt.subject_projections != demand.subject_projections
                || !demand.establishment_routes.contains(&receipt.route)
            {
                return Err(fail(
                    manifest,
                    bindings,
                    "component progress receipt does not exactly discharge its demand subject, profile, projections, and authorized route"
                        .into(),
                ));
            }
            accepted.push(InstalledComponentProgressBinding {
                demand: demand_identity,
                subject: subject.evidence.clone(),
                receipt: (*receipt).clone(),
            });
        }
        accepted.sort_by(|left, right| left.demand.cmp(&right.demand));
        let mut selected_provider_plans = closure
            .selected
            .plans()
            .iter()
            .map(omega_effects::provider_plan::ProviderPlan::identity_fingerprint)
            .collect::<Vec<_>>();
        selected_provider_plans.sort_unstable();
        selected_provider_plans.dedup();
        let non_authoritative_report_fingerprint =
            non_authoritative_component_progress_report_fingerprint(&manifest, &accepted);
        self.accepted_component_progress.push(manifest.clone());
        Ok(InstalledComponentProgressClosure {
            installed: self.installed_context.clone(),
            selected_provider_plans,
            manifest,
            bindings: accepted,
            non_authoritative_report_fingerprint,
        })
    }
}

fn non_authoritative_provider_occurrence_report_fingerprint(
    selected: &SelectedProviderPlanFacts,
    by_plan: &BTreeMap<u64, InstalledProviderOccurrence>,
) -> u64 {
    let mut hash = Fnv1a::new(b"omega.installed-provider-occurrences.v1");
    hash.u64(selected.normalized_identity());
    for (plan, occurrence) in by_plan {
        hash.u64(*plan);
        hash.u64(occurrence.identity().normalized_identity());
        hash.u64(occurrence.installation_receipt().normalized_identity());
        hash.bytes(occurrence.provider_identity().as_bytes());
    }
    hash.finish()
}

fn non_authoritative_component_progress_report_fingerprint(
    manifest: &ComponentProgressManifest,
    bindings: &[InstalledComponentProgressBinding],
) -> u64 {
    let mut hash = Fnv1a::new(b"omega.installed-component-progress.v1");
    hash.u64(manifest.normalized_identity());
    for binding in bindings {
        hash.u64(binding.subject.occurrence.normalized_identity());
        hash.u64(binding.receipt.receipt.normalized_identity());
        hash.u64(binding.receipt.issuer.occurrence.normalized_identity());
        hash.u64(binding.receipt.grant_invocation.normalized_identity());
    }
    hash.finish()
}

struct Fnv1a(u64);

impl Fnv1a {
    fn new(domain: &[u8]) -> Self {
        let mut value = Self(0xcbf29ce484222325);
        value.bytes(domain);
        value
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.u64(bytes.len() as u64);
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> u64 {
        if self.0 == 0 { 1 } else { self.0 }
    }
}
