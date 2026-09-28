//! Provider-admitted external stack leases for one exact installed
//! occurrence.
//!
//! The bound epoch composition retained by `external-roots` is demand
//! evidence: on its own it is neither a provisioned lease nor permission to
//! execute. The publication contract carries the mirror image — runtime
//! provision must cover the admitted candidate before its roots go live, and
//! stack supply is provider-owned. This module owns that supply lane: at most
//! one provider-admitted lease per concrete [`StackDomain`], sealed to the
//! exact installed-code occurrence and rejoined against each incoming root's
//! composed domain demand at install time.
//!
//! The join stays independent of the WCSU evidence, the artifact-entry
//! binding, and ledger custody: a lease or set that names another installed
//! occurrence, bound epoch evidence retained for a different placement under
//! colliding compact identities, a demanded domain with no admitted lease, an
//! unresolved provider-selected domain, and a lease whose capacity or
//! alignment is below the composed domain demand all reject before the
//! ledger sees the install.

use std::collections::BTreeMap;

use executable_installation::{ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId};
use external_roots::{
    ExternalRootDiagnostic, RootAdmission, RootInstallError, RootProviderId, RootSlotAuthority,
    StackDomain, StackValidationReceiptId, ValidatedExternalRoot,
};

/// One provider-admitted stack lease for one exact domain of one installed
/// occurrence.
///
/// The domain is the post-resolution provisioning domain: `ProviderSelected`
/// is a disposition that must resolve to the interrupted domain or one exact
/// provisioned domain, never a lease of its own. `capacity_bytes` and
/// `alignment` are the supply the runtime commits; an installed root rejoins
/// them against its composed domain demand when it installs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedExternalStackDomainLease {
    domain: StackDomain,
    capacity_bytes: u64,
    alignment: u64,
    provisioner: RootProviderId,
    validation_receipt: StackValidationReceiptId,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
}

impl AdmittedExternalStackDomainLease {
    pub const fn domain(&self) -> StackDomain {
        self.domain
    }

    pub const fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn provisioner(&self) -> RootProviderId {
        self.provisioner
    }

    pub const fn validation_receipt(&self) -> StackValidationReceiptId {
        self.validation_receipt
    }

    fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed_code == installed.identity()
            && self.installed_code_context == installed.receipt_context()
            && self.artifact == installed.artifact()
    }
}

#[cfg(test)]
impl AdmittedExternalStackDomainLease {
    pub fn domain_mut_for_test(&mut self) -> &mut StackDomain {
        &mut self.domain
    }
    pub fn capacity_bytes_mut_for_test(&mut self) -> &mut u64 {
        &mut self.capacity_bytes
    }
    pub fn alignment_mut_for_test(&mut self) -> &mut u64 {
        &mut self.alignment
    }
    pub fn provisioner_mut_for_test(&mut self) -> &mut RootProviderId {
        &mut self.provisioner
    }
    pub fn validation_receipt_mut_for_test(&mut self) -> &mut StackValidationReceiptId {
        &mut self.validation_receipt
    }
    pub fn installed_code_mut_for_test(&mut self) -> &mut InstalledCodeId {
        &mut self.installed_code
    }
    pub fn installed_code_context_mut_for_test(&mut self) -> &mut InstalledCodeContext {
        &mut self.installed_code_context
    }
    pub fn artifact_mut_for_test(&mut self) -> &mut ArtifactId {
        &mut self.artifact
    }
}

/// Admit one provider-owned stack lease for `domain` of `installed`.
///
/// An unresolved provider-selected disposition can never be leased: it is a
/// pending choice, and provisioning the choice itself would admit the root
/// without deciding which concrete domain actually runs. Zero capacity and
/// non-power-of-two alignment cannot describe real stack storage either.
pub fn admit_external_stack_domain_lease(
    installed: &InstalledCode,
    domain: StackDomain,
    capacity_bytes: u64,
    alignment: u64,
    provisioner: RootProviderId,
    validation_receipt: StackValidationReceiptId,
) -> Result<AdmittedExternalStackDomainLease, ExternalRootDiagnostic> {
    if domain == StackDomain::ProviderSelected {
        return Err(ExternalRootDiagnostic(
            "an unresolved provider-selected stack disposition cannot be provisioned; it must resolve to the interrupted domain or one exact provisioned domain"
                .into(),
        ));
    }
    if capacity_bytes == 0 {
        return Err(ExternalRootDiagnostic(
            "an external stack domain lease requires nonzero capacity".into(),
        ));
    }
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(ExternalRootDiagnostic(format!(
            "an external stack domain lease requires a nonzero power-of-two alignment, not {alignment}"
        )));
    }
    Ok(AdmittedExternalStackDomainLease {
        domain,
        capacity_bytes,
        alignment,
        provisioner,
        validation_receipt,
        installed_code: installed.identity(),
        installed_code_context: installed.receipt_context(),
        artifact: installed.artifact(),
    })
}

/// The retained runtime provision for one installed occurrence: at most one
/// admitted lease per stack domain, every lease bound to the same exact
/// installed-code context.
///
/// The set is runtime custody, not candidate evidence — the candidate's
/// bound epoch composition says how much each domain needs, and this set
/// says what the runtime actually provisioned. The two rejoin at install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedExternalStackSet {
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    leases: BTreeMap<StackDomain, AdmittedExternalStackDomainLease>,
}

impl ProvisionedExternalStackSet {
    /// The independent installed-occurrence rejoin: this set covers roots of
    /// exactly the code occurrence it was sealed against, including the exact
    /// receipt context that distinguishes two placements of the same code.
    pub fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed_code == installed.identity()
            && self.installed_code_context == installed.receipt_context()
            && self.artifact == installed.artifact()
    }

    pub fn lease(&self, domain: StackDomain) -> Option<&AdmittedExternalStackDomainLease> {
        self.leases.get(&domain)
    }

    pub fn leases(&self) -> impl Iterator<Item = (StackDomain, &AdmittedExternalStackDomainLease)> {
        self.leases.iter().map(|(domain, lease)| (*domain, lease))
    }

    /// Rejoin this retained provision against one incoming root's bound epoch
    /// composition. Every bound stack input must name this exact installed
    /// occurrence — the complete retained code context, not only the compact
    /// installed-code and artifact identities, which cannot distinguish two
    /// placements issued the same provider-minted identity (a cross-context
    /// disposition rejects). The composition must retain the root's own
    /// demand, and each composed domain demand must be covered by an admitted
    /// lease with sufficient capacity and alignment. A demanded domain no
    /// lease provisions — including a provider-selected domain that never
    /// resolved — rejects here rather than at entry.
    pub fn covers_root_demand(
        &self,
        root: &ValidatedExternalRoot,
    ) -> Result<(), ExternalRootDiagnostic> {
        let candidate = root.candidate();
        for (_, input) in candidate.stack.realization.inputs() {
            let evidence = input.realization_evidence();
            if evidence.installed_code() != self.installed_code
                || evidence.installed_code_context() != &self.installed_code_context
                || evidence.artifact() != self.artifact
            {
                return Err(ExternalRootDiagnostic(format!(
                    "bound epoch stack evidence for root 0x{:016x} names a different installed-code occurrence than the retained stack provision",
                    candidate.identity.normalized_identity()
                )));
            }
        }
        let Some(demand) = candidate.stack.realization.demand(candidate.identity) else {
            return Err(ExternalRootDiagnostic(format!(
                "bound epoch stack composition retains no demand for root 0x{:016x}",
                candidate.identity.normalized_identity()
            )));
        };
        for (domain, need) in demand.domains() {
            let Some(lease) = self.leases.get(&domain) else {
                return Err(ExternalRootDiagnostic(format!(
                    "no admitted stack lease provisions domain {domain:?} demanded by root 0x{:016x}",
                    candidate.identity.normalized_identity()
                )));
            };
            if lease.capacity_bytes < need.bytes {
                return Err(ExternalRootDiagnostic(format!(
                    "admitted stack lease for domain {domain:?} provides {} bytes below the composed {}-byte demand of root 0x{:016x}",
                    lease.capacity_bytes,
                    need.bytes,
                    candidate.identity.normalized_identity()
                )));
            }
            if lease.alignment < need.alignment {
                return Err(ExternalRootDiagnostic(format!(
                    "admitted stack lease for domain {domain:?} provides alignment {} below the composed alignment {} of root 0x{:016x}",
                    lease.alignment,
                    need.alignment,
                    candidate.identity.normalized_identity()
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
impl ProvisionedExternalStackSet {
    pub fn installed_code_mut_for_test(&mut self) -> &mut InstalledCodeId {
        &mut self.installed_code
    }
    pub fn installed_code_context_mut_for_test(&mut self) -> &mut InstalledCodeContext {
        &mut self.installed_code_context
    }
    pub fn artifact_mut_for_test(&mut self) -> &mut ArtifactId {
        &mut self.artifact
    }
    pub fn leases_mut_for_test(
        &mut self,
    ) -> &mut BTreeMap<StackDomain, AdmittedExternalStackDomainLease> {
        &mut self.leases
    }
}

/// Seal provider-admitted leases into the retained provision set for one
/// exact installed occurrence.
///
/// A lease bound to a different installed occurrence is a cross-context
/// disposition and rejects; a second lease for the same domain rejects.
/// Either way every supplied lease is returned so the caller can correct and
/// retry without losing custody.
pub fn seal_external_stack_provision(
    installed: &InstalledCode,
    leases: impl IntoIterator<Item = AdmittedExternalStackDomainLease>,
) -> Result<ProvisionedExternalStackSet, ExternalStackProvisionSealError> {
    let leases: Vec<AdmittedExternalStackDomainLease> = leases.into_iter().collect();
    if leases.is_empty() {
        return Err(ExternalStackProvisionSealError {
            leases,
            diagnostic: ExternalRootDiagnostic(
                "external stack provision requires at least one admitted domain lease".into(),
            ),
        });
    }
    for lease in &leases {
        if !lease.binds_installed_code(installed) {
            return Err(ExternalStackProvisionSealError {
                leases,
                diagnostic: ExternalRootDiagnostic(
                    "external stack lease binds a different installed-code occurrence than the provision set being sealed"
                        .into(),
                ),
            });
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut duplicate = None;
    for lease in &leases {
        if !seen.insert(lease.domain) {
            duplicate = Some(lease.domain);
        }
    }
    if let Some(domain) = duplicate {
        return Err(ExternalStackProvisionSealError {
            leases,
            diagnostic: ExternalRootDiagnostic(format!(
                "two external stack leases provision domain {domain:?}"
            )),
        });
    }
    let by_domain: BTreeMap<StackDomain, AdmittedExternalStackDomainLease> = leases
        .iter()
        .map(|lease| (lease.domain, lease.clone()))
        .collect();
    Ok(ProvisionedExternalStackSet {
        installed_code: installed.identity(),
        installed_code_context: installed.receipt_context(),
        artifact: installed.artifact(),
        leases: by_domain,
    })
}

/// Shared provision gate for every runtime-custody install path: the
/// retained set must exist, must bind the occurrence being installed into,
/// and must cover the incoming root's composed domain demand.
pub(crate) fn check_external_stack_provision(
    provision: Option<&ProvisionedExternalStackSet>,
    installed: &InstalledCode,
    root: &ValidatedExternalRoot,
) -> Result<(), ExternalRootDiagnostic> {
    let Some(provision) = provision else {
        return Err(ExternalRootDiagnostic(
            "the retained installed occurrence has no admitted external stack provision; provider-owned supply must cover composed domain demand before an external root installs"
                .into(),
        ));
    };
    if !provision.binds_installed_code(installed) {
        return Err(ExternalRootDiagnostic(
            "retained external stack provision names a different installed-code occurrence".into(),
        ));
    }
    provision.covers_root_demand(root)
}

/// Rejection of an external-root install under runtime custody.
///
/// Provision coverage and ledger admission stay distinguishable; both return
/// the validated root, slot authority, and admission so the caller can
/// correct and retry without losing custody.
#[derive(Debug)]
pub enum ProvisionedRootInstallError {
    /// The retained provision set is absent, names a different installed
    /// occurrence, or does not cover the candidate's composed domain demand.
    Provision {
        root: ValidatedExternalRoot,
        slot: RootSlotAuthority,
        admission: RootAdmission,
        diagnostic: ExternalRootDiagnostic,
    },
    /// The installed-root ledger rejected after provision coverage held.
    Ledger(Box<RootInstallError>),
}

impl ProvisionedRootInstallError {
    pub fn diagnostic(&self) -> &ExternalRootDiagnostic {
        match self {
            Self::Provision { diagnostic, .. } => diagnostic,
            Self::Ledger(error) => error.diagnostic(),
        }
    }

    pub fn into_parts(self) -> (ValidatedExternalRoot, RootSlotAuthority, RootAdmission) {
        match self {
            Self::Provision {
                root,
                slot,
                admission,
                ..
            } => (root, slot, admission),
            Self::Ledger(error) => error.into_parts(),
        }
    }
}

impl std::fmt::Display for ProvisionedRootInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic().fmt(formatter)
    }
}

impl std::error::Error for ProvisionedRootInstallError {}

/// Rejected lease seal; every supplied lease returns for correction.
#[derive(Debug)]
pub struct ExternalStackProvisionSealError {
    leases: Vec<AdmittedExternalStackDomainLease>,
    diagnostic: ExternalRootDiagnostic,
}

impl ExternalStackProvisionSealError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_leases(self) -> Vec<AdmittedExternalStackDomainLease> {
        self.leases
    }
}

impl std::fmt::Display for ExternalStackProvisionSealError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ExternalStackProvisionSealError {}

/// Rejected retained provision; the supplied set returns for retry.
#[derive(Debug)]
pub struct ExternalStackProvisionAdmissionError {
    provision: ProvisionedExternalStackSet,
    diagnostic: ExternalRootDiagnostic,
}

impl ExternalStackProvisionAdmissionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_provision(self) -> ProvisionedExternalStackSet {
        self.provision
    }

    pub(crate) fn new(
        provision: ProvisionedExternalStackSet,
        diagnostic: ExternalRootDiagnostic,
    ) -> Self {
        Self {
            provision,
            diagnostic,
        }
    }
}

impl std::fmt::Display for ExternalStackProvisionAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ExternalStackProvisionAdmissionError {}
