//! The two origins that may introduce a fresh authority account: an exact
//! established program-local occurrence, or an admitted provider issuance.

use crate::extents::extent::diagnostic::{ExtentDiagnostic, nonzero_identity};
use crate::extents::identities::{
    ExtentAliasClassId, ExtentBackingId, ExtentCapacityId, ExtentCustodyRootId,
    ExtentEstablishmentRouteId, ExtentIssuanceId, ExtentLiveIssuancePremiseId,
    ExtentProviderCorrespondenceId, ExtentProviderId, ExtentProviderInvocationId,
    ExtentProviderPlanId, ExtentQualificationId, ExtentTrustProvenanceId,
};

/// Exact admitted external-supply premise behind one provider-issued root.
/// Geometry remains separate and checked; this record identifies why a fresh
/// root over that geometry may enter the conservation ledger at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtentProviderIssuance {
    issuance: ExtentIssuanceId,
    backing: ExtentBackingId,
    provider: ExtentProviderId,
    live_issuance_premise: ExtentLiveIssuancePremiseId,
    custody_root: ExtentCustodyRootId,
    alias_class: ExtentAliasClassId,
    correspondence: ExtentProviderCorrespondenceId,
    trust_provenance: ExtentTrustProvenanceId,
    invocation: ExtentProviderInvocation,
}

/// Exact selected-provider occurrence authorized to introduce one root.
///
/// The plan and invocation identify the concrete occurrence. Route, capacity,
/// and qualification independently identify why that occurrence may establish
/// this content-bearing authority account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtentProviderInvocation {
    provider_plan: ExtentProviderPlanId,
    invocation: ExtentProviderInvocationId,
    establishment_route: ExtentEstablishmentRouteId,
    capacity: ExtentCapacityId,
    qualification: ExtentQualificationId,
}

impl ExtentProviderInvocation {
    pub const fn from_admitted_provider(
        provider_plan: ExtentProviderPlanId,
        invocation: ExtentProviderInvocationId,
        establishment_route: ExtentEstablishmentRouteId,
        capacity: ExtentCapacityId,
        qualification: ExtentQualificationId,
    ) -> Self {
        Self {
            provider_plan,
            invocation,
            establishment_route,
            capacity,
            qualification,
        }
    }

    pub const fn provider_plan(self) -> ExtentProviderPlanId {
        self.provider_plan
    }

    pub const fn invocation(self) -> ExtentProviderInvocationId {
        self.invocation
    }

    pub const fn establishment_route(self) -> ExtentEstablishmentRouteId {
        self.establishment_route
    }

    pub const fn capacity(self) -> ExtentCapacityId {
        self.capacity
    }

    pub const fn qualification(self) -> ExtentQualificationId {
        self.qualification
    }
}

impl ExtentProviderIssuance {
    /// Canonical-decoder convenience preserving the full typed
    /// evidence record while rejecting zero identities in every column.
    pub fn from_normalized_identities(identities: [u64; 13]) -> Result<Self, ExtentDiagnostic> {
        let [
            issuance,
            backing,
            provider,
            live,
            custody,
            alias,
            correspondence,
            trust,
            provider_plan,
            provider_invocation,
            establishment_route,
            capacity,
            qualification,
        ] = identities;
        Ok(Self::from_admitted_provider(
            ExtentIssuanceId::from_normalized_identity(issuance)?,
            ExtentBackingId::from_normalized_identity(backing)?,
            ExtentProviderId::from_normalized_identity(provider)?,
            ExtentLiveIssuancePremiseId::from_normalized_identity(live)?,
            ExtentCustodyRootId::from_normalized_identity(custody)?,
            ExtentAliasClassId::from_normalized_identity(alias)?,
            ExtentProviderCorrespondenceId::from_normalized_identity(correspondence)?,
            ExtentTrustProvenanceId::from_normalized_identity(trust)?,
            ExtentProviderInvocation::from_admitted_provider(
                ExtentProviderPlanId::from_normalized_identity(provider_plan)?,
                ExtentProviderInvocationId::from_normalized_identity(provider_invocation)?,
                ExtentEstablishmentRouteId::from_normalized_identity(establishment_route)?,
                ExtentCapacityId::from_normalized_identity(capacity)?,
                ExtentQualificationId::from_normalized_identity(qualification)?,
            ),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn from_admitted_provider(
        issuance: ExtentIssuanceId,
        backing: ExtentBackingId,
        provider: ExtentProviderId,
        live_issuance_premise: ExtentLiveIssuancePremiseId,
        custody_root: ExtentCustodyRootId,
        alias_class: ExtentAliasClassId,
        correspondence: ExtentProviderCorrespondenceId,
        trust_provenance: ExtentTrustProvenanceId,
        invocation: ExtentProviderInvocation,
    ) -> Self {
        Self {
            issuance,
            backing,
            provider,
            live_issuance_premise,
            custody_root,
            alias_class,
            correspondence,
            trust_provenance,
            invocation,
        }
    }

    pub const fn issuance(self) -> ExtentIssuanceId {
        self.issuance
    }

    pub const fn backing(self) -> ExtentBackingId {
        self.backing
    }

    pub const fn provider(self) -> ExtentProviderId {
        self.provider
    }

    pub const fn live_issuance_premise(self) -> ExtentLiveIssuancePremiseId {
        self.live_issuance_premise
    }

    pub const fn custody_root(self) -> ExtentCustodyRootId {
        self.custody_root
    }

    pub const fn alias_class(self) -> ExtentAliasClassId {
        self.alias_class
    }

    pub const fn correspondence(self) -> ExtentProviderCorrespondenceId {
        self.correspondence
    }

    pub const fn trust_provenance(self) -> ExtentTrustProvenanceId {
        self.trust_provenance
    }

    pub const fn invocation(self) -> ExtentProviderInvocation {
        self.invocation
    }
}

/// Passive identity of one exact established program-local root occurrence.
///
/// This copyable descriptor is suitable for lineage comparison and audit only.
/// It is not establishment authority: the orchestration layer must retain the
/// non-copyable installed occurrence and lifecycle lease for as long as any
/// Extent carrying this identity remains live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentProgramLocalOrigin {
    installed_code: u64,
    external_root: u64,
    root_slot: u64,
    schema_report_fingerprint: u64,
    lifecycle_ledger: u64,
    lifecycle_epoch: u64,
    entry_invocation: u64,
    subject_place: u64,
}

impl ExtentProgramLocalOrigin {
    pub fn from_normalized_identities(identities: [u64; 8]) -> Result<Self, ExtentDiagnostic> {
        let [
            installed_code,
            external_root,
            root_slot,
            schema_report_fingerprint,
            lifecycle_ledger,
            lifecycle_epoch,
            entry_invocation,
            subject_place,
        ] = identities;
        for (identity, label) in [
            (installed_code, "installed-code"),
            (external_root, "external-root"),
            (root_slot, "root-slot"),
            (schema_report_fingerprint, "program-local schema"),
            (lifecycle_ledger, "component lifecycle ledger"),
            (lifecycle_epoch, "component lifecycle epoch"),
            (entry_invocation, "entry invocation"),
            (subject_place, "subject place"),
        ] {
            nonzero_identity(identity, label)?;
        }
        Ok(Self {
            installed_code,
            external_root,
            root_slot,
            schema_report_fingerprint,
            lifecycle_ledger,
            lifecycle_epoch,
            entry_invocation,
            subject_place,
        })
    }

    pub const fn installed_code(self) -> u64 {
        self.installed_code
    }

    pub const fn external_root(self) -> u64 {
        self.external_root
    }

    pub const fn root_slot(self) -> u64 {
        self.root_slot
    }

    pub const fn schema_report_fingerprint(self) -> u64 {
        self.schema_report_fingerprint
    }

    pub const fn lifecycle_ledger(self) -> u64 {
        self.lifecycle_ledger
    }

    pub const fn lifecycle_epoch(self) -> u64 {
        self.lifecycle_epoch
    }

    pub const fn entry_invocation(self) -> u64 {
        self.entry_invocation
    }

    pub const fn subject_place(self) -> u64 {
        self.subject_place
    }
}

/// The only two origins permitted to create a fresh Extent authority account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtentRootOrigin {
    ProgramLocal(ExtentProgramLocalOrigin),
    ProviderIssued(ExtentProviderIssuance),
}

impl ExtentRootOrigin {
    pub const fn program_local(self) -> Option<ExtentProgramLocalOrigin> {
        match self {
            Self::ProgramLocal(origin) => Some(origin),
            Self::ProviderIssued(_) => None,
        }
    }

    pub const fn provider_issuance(self) -> Option<ExtentProviderIssuance> {
        match self {
            Self::ProgramLocal(_) => None,
            Self::ProviderIssued(issuance) => Some(issuance),
        }
    }
}
