#![forbid(unsafe_code)]

//! Conservation model for authority over concrete address-space ranges.
//!
//! An `Extent` is not an allocator and an address is not authority. Root
//! extents enter through admitted providers; ordinary operations may only
//! split, attenuate, borrow, or rejoin authority already present.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(u64);

impl AddressSpaceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "address-space")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentProvenanceId(u64);

impl ExtentProvenanceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "extent-provenance")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappingEraId(u64);

impl MappingEraId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "mapping-era")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentLineageId(u64);

impl ExtentLineageId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "extent-lineage")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

macro_rules! normalized_extent_identity {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
                nonzero_identity(identity, $label)?;
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_extent_identity!(ExtentIssuanceId, "extent-issuance");
normalized_extent_identity!(ExtentBackingId, "extent-backing");
normalized_extent_identity!(ExtentProviderId, "extent-provider");
normalized_extent_identity!(ExtentLiveIssuancePremiseId, "extent-live-issuance-premise");
normalized_extent_identity!(ExtentCustodyRootId, "extent-custody-root");
normalized_extent_identity!(ExtentAliasClassId, "extent-alias-class");
normalized_extent_identity!(
    ExtentProviderCorrespondenceId,
    "extent-provider-correspondence"
);
normalized_extent_identity!(ExtentTrustProvenanceId, "extent-trust-provenance");
normalized_extent_identity!(ExtentProviderPlanId, "extent-provider-plan");
normalized_extent_identity!(ExtentProviderInvocationId, "extent-provider-invocation");
normalized_extent_identity!(ExtentEstablishmentRouteId, "extent-establishment-route");
normalized_extent_identity!(ExtentCapacityId, "extent-capacity");
normalized_extent_identity!(ExtentQualificationId, "extent-qualification");
normalized_extent_identity!(
    ExtentContentInterpretationId,
    "extent-content-interpretation"
);
normalized_extent_identity!(
    ExtentContentValidityReceiptId,
    "extent-content-validity-receipt"
);
normalized_extent_identity!(
    ExtentContentCustodyReceiptId,
    "extent-content-custody-receipt"
);
normalized_extent_identity!(ResidentClaimId, "resident-claim");

/// Exact semantic interpretation selected for provider-existing content.
///
/// The compact fingerprint remains useful for compatibility reporting, but
/// consumers must also rejoin the collision-resistant commitment before the
/// provider's content-validity evidence can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentContentInterpretation {
    compatibility_fingerprint: ExtentContentInterpretationId,
    commitment: [u8; 32],
}

impl ExtentContentInterpretation {
    pub const fn from_sha256_commitment(
        compatibility_fingerprint: ExtentContentInterpretationId,
        commitment: [u8; 32],
    ) -> Self {
        Self {
            compatibility_fingerprint,
            commitment,
        }
    }

    pub const fn compatibility_fingerprint(self) -> ExtentContentInterpretationId {
        self.compatibility_fingerprint
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

use std::collections::BTreeSet;

mod mapping;

pub use mapping::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentRightId(u64);

impl ExtentRightId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "extent-right")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// An open, normalized set of grant-established rights.
///
/// The compiler does not bless a READ/WRITE/EXECUTE enumeration here. Target
/// and provider packages define right identities; admission controls which
/// sets may enter a root grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtentRights(BTreeSet<ExtentRightId>);

impl ExtentRights {
    pub const fn none() -> Self {
        Self(BTreeSet::new())
    }

    pub fn from_normalized_identities(rights: impl IntoIterator<Item = ExtentRightId>) -> Self {
        Self(rights.into_iter().collect())
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.union(&other.0).copied().collect())
    }

    pub fn contains(&self, required: &Self) -> bool {
        required.0.is_subset(&self.0)
    }

    pub fn identities(&self) -> impl Iterator<Item = ExtentRightId> + '_ {
        self.0.iter().copied()
    }
}

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

/// One-shot authority to mint exactly one root extent.
///
/// Compiler code constructs this from one of the two exact root origins.
/// Omega source never receives a constructor for either this grant or `Extent`
/// itself. Program-local callers must additionally retain the exact installed
/// occurrence account; this grant carries only its passive origin descriptor.
#[derive(Debug, PartialEq, Eq)]
pub struct ExtentRootGrant {
    origin: ExtentRootOrigin,
    lineage: ExtentLineageId,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
}

/// One-shot provider evidence that an exact freshly introduced Extent already
/// contains valid content under one normalized interpretation and that
/// custody of that content transfers with the Extent.
///
/// This grant is deliberately non-`Clone` and can only be emitted while
/// consuming a provider-issued [`ExtentRootGrant`]. Program-local
/// capacity cannot acquire provider content validity through this route.
#[derive(Debug, PartialEq, Eq)]
pub struct ProviderExistingContentGrant {
    origin: ExtentRootOrigin,
    lineage_root: ExtentLineageId,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    interpretation: ExtentContentInterpretation,
    resident_claim: ResidentClaimId,
    validity_receipt: ExtentContentValidityReceiptId,
    custody_receipt: ExtentContentCustodyReceiptId,
}

impl ProviderExistingContentGrant {
    pub const fn origin(&self) -> ExtentRootOrigin {
        self.origin
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage_root
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.provenance
    }

    pub const fn era(&self) -> MappingEraId {
        self.era
    }

    pub const fn interpretation(&self) -> ExtentContentInterpretation {
        self.interpretation
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.resident_claim
    }

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.validity_receipt
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.custody_receipt
    }
}

/// Runtime geometry whose source-language `no_wrap(base, length)` predicate
/// has already been checked in proof-level (non-wrapping) arithmetic.
///
/// Keeping this as a separate value lets an entry installer validate every
/// inbound range before it imports any complete `Extent::Granted` fact.  It
/// carries geometry only; authority still comes exclusively from consuming an
/// admitted [`ExtentRootGrant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedExtentGeometry {
    base: u64,
    length: u64,
}

impl ValidatedExtentGeometry {
    pub fn check(base: u64, length: u64) -> Result<Self, ExtentDiagnostic> {
        validate_range(base, length)?;
        Ok(Self { base, length })
    }

    pub const fn base(self) -> u64 {
        self.base
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

impl ExtentRootGrant {
    pub const fn from_admitted_provider(
        provider_issuance: ExtentProviderIssuance,
        lineage: ExtentLineageId,
        address_space: AddressSpaceId,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
    ) -> Self {
        Self {
            origin: ExtentRootOrigin::ProviderIssued(provider_issuance),
            lineage,
            address_space,
            rights,
            provenance,
            era,
        }
    }

    #[doc(hidden)]
    pub const fn from_established_program_local(
        origin: ExtentProgramLocalOrigin,
        lineage: ExtentLineageId,
        address_space: AddressSpaceId,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
    ) -> Self {
        Self {
            origin: ExtentRootOrigin::ProgramLocal(origin),
            lineage,
            address_space,
            rights,
            provenance,
            era,
        }
    }

    pub const fn origin(&self) -> ExtentRootOrigin {
        self.origin
    }

    /// Report-only identity of the root lineage this one-shot grant will mint.
    /// Observing it does not duplicate or consume the grant.
    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage
    }

    pub fn mint(self, base: u64, length: u64) -> Result<Extent, MintError> {
        let geometry = match ValidatedExtentGeometry::check(base, length) {
            Ok(geometry) => geometry,
            Err(diagnostic) => {
                return Err(MintError {
                    grant: self,
                    diagnostic,
                });
            }
        };
        Ok(self.mint_validated(geometry))
    }

    /// Consume one provider-issued root grant into both its exact Extent and
    /// one-shot existing-content authority.
    ///
    /// The interpretation is provider-admitted input here; its consumer must
    /// compare both its report fingerprint and strong commitment with the
    /// actual normalized placement selected for the Extent. Failure returns
    /// the complete root grant for retry.
    pub fn mint_provider_existing_content(
        self,
        base: u64,
        length: u64,
        interpretation: ExtentContentInterpretation,
        resident_claim: ResidentClaimId,
        validity_receipt: ExtentContentValidityReceiptId,
        custody_receipt: ExtentContentCustodyReceiptId,
    ) -> Result<(Extent, ProviderExistingContentGrant), ExistingContentMintError> {
        let geometry = match ValidatedExtentGeometry::check(base, length) {
            Ok(geometry) => geometry,
            Err(diagnostic) => {
                return Err(ExistingContentMintError {
                    grant: self,
                    diagnostic,
                });
            }
        };
        if !matches!(self.origin, ExtentRootOrigin::ProviderIssued(_)) {
            return Err(ExistingContentMintError {
                grant: self,
                diagnostic: ExtentDiagnostic(
                    "program-local roots cannot issue provider existing-content evidence".into(),
                ),
            });
        }
        let content = ProviderExistingContentGrant {
            origin: self.origin,
            lineage_root: self.lineage,
            base: geometry.base,
            length: geometry.length,
            address_space: self.address_space,
            provenance: self.provenance,
            era: self.era,
            interpretation,
            resident_claim,
            validity_receipt,
            custody_receipt,
        };
        Ok((self.mint_validated(geometry), content))
    }

    /// Consume admitted authority for geometry whose `no_wrap` obligation was
    /// checked before this operation began.
    pub fn mint_validated(self, geometry: ValidatedExtentGeometry) -> Extent {
        let Self {
            origin,
            lineage,
            address_space,
            rights,
            provenance,
            era,
        } = self;
        Extent {
            base: geometry.base,
            length: geometry.length,
            address_space,
            rights,
            provenance,
            era,
            origin,
            lineage: Lineage {
                root: lineage,
                path: Vec::new(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitBranch {
    Lower,
    Upper,
}

#[derive(Debug, PartialEq, Eq)]
struct Lineage {
    root: ExtentLineageId,
    path: Vec<SplitBranch>,
}

/// Opaque authority over one concrete address-space range.
///
/// This Rust carrier is deliberately non-`Clone`; Omega's `[linear]` checker
/// supplies the source-language must-consume rule. Every consuming operation
/// below returns the authority on failure rather than silently dropping it.
#[derive(Debug, PartialEq, Eq)]
pub struct Extent {
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    origin: ExtentRootOrigin,
    lineage: Lineage,
}

impl Extent {
    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn end(&self) -> u64 {
        self.base + self.length
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    pub const fn rights(&self) -> &ExtentRights {
        &self.rights
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.provenance
    }

    pub const fn era(&self) -> MappingEraId {
        self.era
    }

    pub const fn origin(&self) -> ExtentRootOrigin {
        self.origin
    }

    pub const fn provider_issuance(&self) -> Option<ExtentProviderIssuance> {
        self.origin.provider_issuance()
    }

    pub const fn program_local_origin(&self) -> Option<ExtentProgramLocalOrigin> {
        self.origin.program_local()
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage.root
    }

    /// Whether this value is the exact unsplit root of its passive lineage.
    /// This is report/retirement structure only; it never establishes origin.
    pub fn is_lineage_root(&self) -> bool {
        self.lineage.path.is_empty()
    }

    pub fn split_at(self, lower_length: u64) -> Result<(Self, Self), SplitError> {
        if lower_length == 0 || lower_length >= self.length {
            return Err(SplitError {
                extent: self,
                diagnostic: ExtentDiagnostic(
                    "split point must produce two nonempty child extents".into(),
                ),
            });
        }

        Ok(self.split_at_validated(lower_length))
    }

    fn split_at_validated(self, lower_length: u64) -> (Self, Self) {
        debug_assert!(lower_length > 0 && lower_length < self.length);

        let upper_base = self.base + lower_length;
        let upper_length = self.length - lower_length;
        let mut lower_path = self.lineage.path.clone();
        lower_path.push(SplitBranch::Lower);
        let mut upper_path = self.lineage.path;
        upper_path.push(SplitBranch::Upper);

        let lower = Self {
            base: self.base,
            length: lower_length,
            address_space: self.address_space,
            rights: self.rights.clone(),
            provenance: self.provenance,
            era: self.era,
            origin: self.origin,
            lineage: Lineage {
                root: self.lineage.root,
                path: lower_path,
            },
        };
        let upper = Self {
            base: upper_base,
            length: upper_length,
            address_space: self.address_space,
            rights: self.rights,
            provenance: self.provenance,
            era: self.era,
            origin: self.origin,
            lineage: Lineage {
                root: self.lineage.root,
                path: upper_path,
            },
        };
        (lower, upper)
    }

    /// Extract one independently owned subextent while retaining every byte
    /// of the parent authority in an explicit conserved partition.
    ///
    /// Borrowed layout/static views should use [`Extent::loan`] instead. This
    /// operation is for an allocation or other subresource that genuinely
    /// leaves the parent's ownership domain.
    pub fn partition_owned(
        self,
        offset: u64,
        length: u64,
    ) -> Result<OwnedExtentPartition, OwnedPartitionError> {
        if length == 0 {
            return Err(OwnedPartitionError {
                extent: self,
                diagnostic: ExtentDiagnostic(
                    "owned subextent must carry nonempty authority".into(),
                ),
            });
        }
        let Some(end) = offset.checked_add(length) else {
            return Err(OwnedPartitionError {
                extent: self,
                diagnostic: ExtentDiagnostic("owned subextent range overflows".into()),
            });
        };
        if end > self.length {
            let parent_length = self.length;
            return Err(OwnedPartitionError {
                extent: self,
                diagnostic: ExtentDiagnostic(format!(
                    "owned subextent {offset}..{end} exceeds {parent_length}-byte parent"
                )),
            });
        }

        let (before, selected, after) = match (offset, end == self.length) {
            (0, true) => (None, self, None),
            (0, false) => {
                let (selected, after) = self.split_at_validated(length);
                (None, selected, Some(after))
            }
            (_, true) => {
                let (before, selected) = self.split_at_validated(offset);
                (Some(before), selected, None)
            }
            (_, false) => {
                let (before, tail) = self.split_at_validated(offset);
                let (selected, after) = tail.split_at_validated(length);
                (Some(before), selected, Some(after))
            }
        };
        Ok(OwnedExtentPartition {
            before,
            selected,
            after,
        })
    }

    pub fn attenuate(self, rights: ExtentRights) -> Result<Self, AttenuationError> {
        if !self.rights.contains(&rights) {
            return Err(AttenuationError {
                extent: self,
                diagnostic: ExtentDiagnostic("attenuation cannot add extent rights".into()),
            });
        }
        Ok(Self { rights, ..self })
    }

    pub fn merge(self, other: Self) -> Result<Self, Box<MergeError>> {
        if let Err(diagnostic) = validate_merge(&self, &other) {
            return Err(Box::new(MergeError {
                first: self,
                second: other,
                diagnostic,
            }));
        }

        let (lower, upper) = if self.base < other.base {
            (self, other)
        } else {
            (other, self)
        };
        let mut parent_path = lower.lineage.path;
        parent_path.pop();
        Ok(Self {
            base: lower.base,
            length: lower.length + upper.length,
            address_space: lower.address_space,
            rights: lower.rights,
            provenance: lower.provenance,
            era: lower.era,
            origin: lower.origin,
            lineage: Lineage {
                root: lower.lineage.root,
                path: parent_path,
            },
        })
    }

    pub fn loan(&self, offset: u64, length: u64) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        ExtentLoan::shared(self, offset, length)
    }

    pub fn loan_mut(
        &mut self,
        offset: u64,
        length: u64,
    ) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        ExtentLoan::exclusive(self, offset, length)
    }
}

/// One exact owned extraction and all authority needed to account for its
/// parent. Private fields prevent callers from silently dropping a remainder
/// while inspecting the partition; `into_parts` explicitly transfers every
/// resulting claim.
#[derive(Debug)]
pub struct OwnedExtentPartition {
    before: Option<Extent>,
    selected: Extent,
    after: Option<Extent>,
}

impl OwnedExtentPartition {
    pub const fn before(&self) -> Option<&Extent> {
        self.before.as_ref()
    }

    pub const fn selected(&self) -> &Extent {
        &self.selected
    }

    pub const fn after(&self) -> Option<&Extent> {
        self.after.as_ref()
    }

    pub fn into_parts(self) -> (Option<Extent>, Extent, Option<Extent>) {
        (self.before, self.selected, self.after)
    }

    /// Recompose an unmodified partition into its exact parent authority.
    pub fn rejoin(self) -> Extent {
        let mut restored = self.selected;
        if let Some(after) = self.after {
            restored = restored
                .merge(after)
                .expect("private owned partition retains exact upper sibling");
        }
        if let Some(before) = self.before {
            restored = before
                .merge(restored)
                .expect("private owned partition retains exact lower sibling");
        }
        restored
    }
}

fn validate_merge(first: &Extent, second: &Extent) -> Result<(), ExtentDiagnostic> {
    if first.address_space != second.address_space
        || first.rights != second.rights
        || first.provenance != second.provenance
        || first.era != second.era
    {
        return Err(ExtentDiagnostic(
            "merge requires identical space, rights, provenance, and era".into(),
        ));
    }
    if first.lineage.root != second.lineage.root {
        return Err(ExtentDiagnostic(
            "numeric adjacency cannot merge independent authority lineages".into(),
        ));
    }
    if first.origin != second.origin {
        return Err(ExtentDiagnostic(
            "merge requires identical exact root-origin evidence".into(),
        ));
    }
    let Some((first_branch, first_parent)) = first.lineage.path.split_last() else {
        return Err(ExtentDiagnostic(
            "root extents have no merge sibling".into(),
        ));
    };
    let Some((second_branch, second_parent)) = second.lineage.path.split_last() else {
        return Err(ExtentDiagnostic(
            "root extents have no merge sibling".into(),
        ));
    };
    if first_parent != second_parent || first_branch == second_branch {
        return Err(ExtentDiagnostic(
            "merge requires the two children of one conserved split".into(),
        ));
    }

    let (lower, upper) = if first.base < second.base {
        (first, second)
    } else {
        (second, first)
    };
    let Some(lower_end) = lower.base.checked_add(lower.length) else {
        return Err(ExtentDiagnostic("lower extent range overflows".into()));
    };
    if lower_end != upper.base
        || !matches!(lower.lineage.path.last(), Some(SplitBranch::Lower))
        || !matches!(upper.lineage.path.last(), Some(SplitBranch::Upper))
    {
        return Err(ExtentDiagnostic(
            "merge children do not restore their split geometry".into(),
        ));
    }
    lower
        .length
        .checked_add(upper.length)
        .ok_or_else(|| ExtentDiagnostic("merged extent length overflows".into()))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanPolarity {
    Shared,
    Exclusive,
}

enum LoanBacking<'a> {
    Shared(&'a Extent),
    Exclusive(&'a mut Extent),
}

/// One borrow-carrying subrange carrier. Its polarity derives from the parent
/// borrow rather than becoming a second nominal loan type.
pub struct ExtentLoan<'a> {
    backing: LoanBacking<'a>,
    base: u64,
    length: u64,
}

impl<'a> ExtentLoan<'a> {
    fn shared(extent: &'a Extent, offset: u64, length: u64) -> Result<Self, ExtentDiagnostic> {
        let base = validate_subrange(extent, offset, length)?;
        Ok(Self {
            backing: LoanBacking::Shared(extent),
            base,
            length,
        })
    }

    fn exclusive(
        extent: &'a mut Extent,
        offset: u64,
        length: u64,
    ) -> Result<Self, ExtentDiagnostic> {
        let base = validate_subrange(extent, offset, length)?;
        Ok(Self {
            backing: LoanBacking::Exclusive(extent),
            base,
            length,
        })
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn polarity(&self) -> LoanPolarity {
        match self.backing {
            LoanBacking::Shared(_) => LoanPolarity::Shared,
            LoanBacking::Exclusive(_) => LoanPolarity::Exclusive,
        }
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.address_space,
            LoanBacking::Exclusive(extent) => extent.address_space,
        }
    }

    pub const fn rights(&self) -> &ExtentRights {
        match &self.backing {
            LoanBacking::Shared(extent) => &extent.rights,
            LoanBacking::Exclusive(extent) => &extent.rights,
        }
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.provenance,
            LoanBacking::Exclusive(extent) => extent.provenance,
        }
    }

    pub const fn era(&self) -> MappingEraId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.era,
            LoanBacking::Exclusive(extent) => extent.era,
        }
    }

    pub const fn origin(&self) -> ExtentRootOrigin {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.origin,
            LoanBacking::Exclusive(extent) => extent.origin,
        }
    }

    pub const fn provider_issuance(&self) -> Option<ExtentProviderIssuance> {
        self.origin().provider_issuance()
    }

    pub const fn program_local_origin(&self) -> Option<ExtentProgramLocalOrigin> {
        self.origin().program_local()
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        match &self.backing {
            LoanBacking::Shared(extent) => extent.lineage.root,
            LoanBacking::Exclusive(extent) => extent.lineage.root,
        }
    }
}

impl std::fmt::Debug for ExtentLoan<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExtentLoan")
            .field("base", &self.base)
            .field("length", &self.length)
            .field("polarity", &self.polarity())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalBorrowerId(u64);

impl ExternalBorrowerId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-borrower")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalLoanId(u64);

impl ExternalLoanId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-loan")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLoanDirection {
    /// The external borrower observes memory. CPU reads remain compatible, but
    /// ordinary CPU mutation is excluded by the carried shared borrow.
    DeviceReads,
    /// The external borrower may mutate memory. The carried exclusive borrow
    /// excludes all CPU access until completion.
    DeviceWrites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalCompletionFactId(u64);

impl ExternalCompletionFactId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-completion-fact")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalReachReceiptId(u64);

impl ExternalReachReceiptId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "external-reach receipt")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Why the provider may assert that the invisible borrower can reach only the
/// exact range named by one external loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalReachMechanism {
    /// Admission trusts the borrower's checked/validated descriptor contract.
    AdmittedBorrowerContract,
    /// An IOMMU or equivalent hardware boundary enforces the exact range.
    HardwareIsolation,
}

/// Provider-authored, per-transfer evidence of external-agent confinement.
/// Numeric geometry alone is insufficient: provenance and mapping era bind an
/// identical address range to the exact authority that was lent.
#[derive(Debug, PartialEq, Eq)]
pub struct ExternalReachReceipt {
    identity: ExternalReachReceiptId,
    loan: ExternalLoanId,
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    lineage: ExtentLineageId,
    rights: ExtentRights,
    base: u64,
    length: u64,
    mechanism: ExternalReachMechanism,
    confined_to_range: bool,
}

impl ExternalReachReceipt {
    pub fn from_admitted_provider(
        identity: ExternalReachReceiptId,
        loan_identity: ExternalLoanId,
        grant: &ExternalLoanGrant,
        loan: &ExtentLoan<'_>,
        mechanism: ExternalReachMechanism,
        confined_to_range: bool,
    ) -> Self {
        Self {
            identity,
            loan: loan_identity,
            borrower: grant.borrower,
            direction: grant.direction,
            address_space: loan.address_space(),
            provenance: loan.provenance(),
            era: loan.era(),
            lineage: loan.lineage_root(),
            rights: loan.rights().clone(),
            base: loan.base(),
            length: loan.length(),
            mechanism,
            confined_to_range,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionObligations(BTreeSet<ExternalCompletionFactId>);

impl CompletionObligations {
    pub fn from_normalized_facts(
        facts: impl IntoIterator<Item = ExternalCompletionFactId>,
    ) -> Self {
        Self(facts.into_iter().collect())
    }

    pub fn facts(&self) -> impl Iterator<Item = ExternalCompletionFactId> + '_ {
        self.0.iter().copied()
    }
}

/// Reusable admitted policy for lending matching extents to one external
/// borrower. The per-transfer `ExternalLoanId` distinguishes completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLoanGrant {
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    required_rights: ExtentRights,
    completion: CompletionObligations,
}

impl ExternalLoanGrant {
    pub fn from_admitted_provider(
        borrower: ExternalBorrowerId,
        direction: ExternalLoanDirection,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        required_rights: ExtentRights,
        completion: CompletionObligations,
    ) -> Self {
        Self {
            borrower,
            direction,
            address_space,
            provenance,
            required_rights,
            completion,
        }
    }
}

/// Linear proxy for a borrower the Omega checker cannot inspect.
///
/// The token owns the Rust loan that excludes incompatible CPU access. Omega's
/// `[linear]` rule makes completion mandatory in the source language.
#[derive(Debug)]
pub struct ExternalLoan<'extent> {
    identity: ExternalLoanId,
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    completion: CompletionObligations,
    reach_receipt: ExternalReachReceiptId,
    reach_mechanism: ExternalReachMechanism,
    loan: ExtentLoan<'extent>,
}

impl<'extent> ExternalLoan<'extent> {
    pub const fn identity(&self) -> ExternalLoanId {
        self.identity
    }

    pub const fn borrower(&self) -> ExternalBorrowerId {
        self.borrower
    }

    pub const fn direction(&self) -> ExternalLoanDirection {
        self.direction
    }

    pub const fn reach_receipt(&self) -> ExternalReachReceiptId {
        self.reach_receipt
    }

    pub const fn reach_mechanism(&self) -> ExternalReachMechanism {
        self.reach_mechanism
    }

    pub const fn base(&self) -> u64 {
        self.loan.base()
    }

    pub const fn length(&self) -> u64 {
        self.loan.length()
    }

    pub fn complete(
        self,
        receipt: ExternalCompletionReceipt,
    ) -> Result<ExternalLoanCompletion, Box<ExternalCompletionError<'extent>>> {
        let mismatch = if receipt.loan != self.identity {
            Some("completion receipt names a different external loan")
        } else if receipt.borrower != self.borrower {
            Some("completion receipt names a different external borrower")
        } else if receipt.direction != self.direction {
            Some("completion receipt names a different transfer direction")
        } else if receipt.reach_receipt != self.reach_receipt {
            Some("completion receipt names different confinement evidence")
        } else if receipt.address_space != self.loan.address_space() {
            Some("completion receipt names a different address space")
        } else if receipt.provenance != self.loan.provenance() {
            Some("completion receipt names different extent provenance")
        } else if receipt.era != self.loan.era() {
            Some("completion receipt names a stale mapping era")
        } else if receipt.lineage != self.loan.lineage_root() {
            Some("completion receipt names a different extent authority lineage")
        } else if receipt.rights != *self.loan.rights() {
            Some("completion receipt names different attenuated extent rights")
        } else if receipt.base != self.loan.base() || receipt.length != self.loan.length() {
            Some("completion receipt names a different lent extent range")
        } else if !receipt.borrow_released {
            Some("completion receipt does not establish external-borrow release")
        } else if !self.completion.0.is_subset(&receipt.established_facts) {
            Some("completion receipt lacks facts required by the external-loan grant")
        } else {
            None
        };

        if let Some(message) = mismatch {
            return Err(Box::new(ExternalCompletionError {
                loan: self,
                receipt,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }

        Ok(ExternalLoanCompletion {
            loan: self.identity,
            borrower: self.borrower,
            reach_receipt: self.reach_receipt,
            base: self.loan.base(),
            length: self.loan.length(),
        })
    }
}

pub fn begin_external_loan<'extent>(
    loan: ExtentLoan<'extent>,
    identity: ExternalLoanId,
    grant: &ExternalLoanGrant,
    reach_receipt: Option<ExternalReachReceipt>,
) -> Result<ExternalLoan<'extent>, Box<ExternalLoanStartError<'extent>>> {
    let mismatch = if loan.address_space() != grant.address_space {
        Some("extent address space does not match external-loan grant")
    } else if loan.provenance() != grant.provenance {
        Some("extent provenance does not match external-loan grant")
    } else if !loan.rights().contains(&grant.required_rights) {
        Some("extent lacks rights required by external-loan grant")
    } else if grant.direction == ExternalLoanDirection::DeviceReads
        && loan.polarity() != LoanPolarity::Shared
    {
        Some("device-read lending requires a shared extent loan")
    } else if grant.direction == ExternalLoanDirection::DeviceWrites
        && loan.polarity() != LoanPolarity::Exclusive
    {
        Some("device-write lending requires an exclusive extent loan")
    } else {
        None
    };

    if let Some(message) = mismatch {
        return Err(Box::new(ExternalLoanStartError {
            loan,
            reach_receipt,
            diagnostic: ExtentDiagnostic(message.into()),
        }));
    }

    let reach_mismatch = match reach_receipt.as_ref() {
        None => Some("external borrower requires exact-range reach evidence or hardware isolation"),
        Some(receipt) if receipt.loan != identity => {
            Some("external-reach receipt names a different external loan")
        }
        Some(receipt) if receipt.borrower != grant.borrower => {
            Some("external-reach receipt names a different borrower")
        }
        Some(receipt) if receipt.direction != grant.direction => {
            Some("external-reach receipt names a different transfer direction")
        }
        Some(receipt) if receipt.address_space != loan.address_space() => {
            Some("external-reach receipt names a different address space")
        }
        Some(receipt) if receipt.provenance != loan.provenance() => {
            Some("external-reach receipt names different extent provenance")
        }
        Some(receipt) if receipt.era != loan.era() => {
            Some("external-reach receipt names a stale mapping era")
        }
        Some(receipt) if receipt.lineage != loan.lineage_root() => {
            Some("external-reach receipt names a different extent authority lineage")
        }
        Some(receipt) if receipt.rights != *loan.rights() => {
            Some("external-reach receipt names different attenuated extent rights")
        }
        Some(receipt) if receipt.base != loan.base() || receipt.length != loan.length() => {
            Some("external borrower reach is not the exact lent extent range")
        }
        Some(receipt) if !receipt.confined_to_range => {
            Some("external-reach receipt does not establish range confinement")
        }
        Some(_) => None,
    };
    if let Some(message) = reach_mismatch {
        return Err(Box::new(ExternalLoanStartError {
            loan,
            reach_receipt,
            diagnostic: ExtentDiagnostic(message.into()),
        }));
    }
    let reach_receipt = reach_receipt.expect("validated external-reach receipt");

    Ok(ExternalLoan {
        identity,
        borrower: grant.borrower,
        direction: grant.direction,
        completion: grant.completion.clone(),
        reach_receipt: reach_receipt.identity,
        reach_mechanism: reach_receipt.mechanism,
        loan,
    })
}

/// Provider-authored evidence that the invisible borrower released its loan.
/// Construction belongs to the admitted boundary provider, never the borrower.
#[derive(Debug, PartialEq, Eq)]
pub struct ExternalCompletionReceipt {
    loan: ExternalLoanId,
    borrower: ExternalBorrowerId,
    direction: ExternalLoanDirection,
    reach_receipt: ExternalReachReceiptId,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    lineage: ExtentLineageId,
    rights: ExtentRights,
    base: u64,
    length: u64,
    borrow_released: bool,
    established_facts: BTreeSet<ExternalCompletionFactId>,
}

impl ExternalCompletionReceipt {
    /// Bind completion evidence to the exact live external loan instead of
    /// asking the provider to restate its replay-sensitive authority facts.
    pub fn from_admitted_provider(
        loan: &ExternalLoan<'_>,
        borrow_released: bool,
        established_facts: impl IntoIterator<Item = ExternalCompletionFactId>,
    ) -> Self {
        Self {
            loan: loan.identity,
            borrower: loan.borrower,
            direction: loan.direction,
            reach_receipt: loan.reach_receipt,
            address_space: loan.loan.address_space(),
            provenance: loan.loan.provenance(),
            era: loan.loan.era(),
            lineage: loan.loan.lineage_root(),
            rights: loan.loan.rights().clone(),
            base: loan.loan.base(),
            length: loan.loan.length(),
            borrow_released,
            established_facts: established_facts.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLoanCompletion {
    pub loan: ExternalLoanId,
    pub borrower: ExternalBorrowerId,
    pub reach_receipt: ExternalReachReceiptId,
    pub base: u64,
    pub length: u64,
}

#[derive(Debug)]
pub struct ExternalLoanStartError<'extent> {
    loan: ExtentLoan<'extent>,
    reach_receipt: Option<ExternalReachReceipt>,
    diagnostic: ExtentDiagnostic,
}

impl<'extent> ExternalLoanStartError<'extent> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_loan(self) -> ExtentLoan<'extent> {
        self.loan
    }

    pub fn into_parts(self) -> (ExtentLoan<'extent>, Option<ExternalReachReceipt>) {
        (self.loan, self.reach_receipt)
    }
}

#[derive(Debug)]
pub struct ExternalCompletionError<'extent> {
    loan: ExternalLoan<'extent>,
    receipt: ExternalCompletionReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'extent> ExternalCompletionError<'extent> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ExternalLoan<'extent>, ExternalCompletionReceipt) {
        (self.loan, self.receipt)
    }
}

fn validate_subrange(extent: &Extent, offset: u64, length: u64) -> Result<u64, ExtentDiagnostic> {
    if length == 0 {
        return Err(ExtentDiagnostic("extent loan cannot be empty".into()));
    }
    let end = offset
        .checked_add(length)
        .ok_or_else(|| ExtentDiagnostic("extent loan range overflows".into()))?;
    if end > extent.length {
        return Err(ExtentDiagnostic(format!(
            "extent loan {offset}..{end} exceeds {}-byte parent",
            extent.length
        )));
    }
    extent
        .base
        .checked_add(offset)
        .ok_or_else(|| ExtentDiagnostic("extent loan base overflows".into()))
}

fn validate_range(base: u64, length: u64) -> Result<(), ExtentDiagnostic> {
    if length == 0 {
        return Err(ExtentDiagnostic(
            "root extent must carry nonempty authority".into(),
        ));
    }
    base.checked_add(length)
        .ok_or_else(|| ExtentDiagnostic("extent range overflows address width".into()))?;
    Ok(())
}

fn nonzero_identity(identity: u64, name: &str) -> Result<(), ExtentDiagnostic> {
    if identity == 0 {
        return Err(ExtentDiagnostic(format!(
            "normalized {name} identity cannot be zero"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentDiagnostic(pub String);

impl std::fmt::Display for ExtentDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExtentDiagnostic {}

#[derive(Debug, PartialEq, Eq)]
pub struct MintError {
    grant: ExtentRootGrant,
    diagnostic: ExtentDiagnostic,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExistingContentMintError {
    grant: ExtentRootGrant,
    diagnostic: ExtentDiagnostic,
}

impl ExistingContentMintError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}

impl MintError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SplitError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OwnedPartitionError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

impl OwnedPartitionError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

impl SplitError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AttenuationError {
    extent: Extent,
    diagnostic: ExtentDiagnostic,
}

impl AttenuationError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MergeError {
    first: Extent,
    second: Extent,
    diagnostic: ExtentDiagnostic,
}

impl MergeError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extents(self) -> (Extent, Extent) {
        (self.first, self.second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn provider_issuance(seed: u64) -> ExtentProviderIssuance {
        provider_issuance_for_invocation(seed, seed)
    }

    fn provider_issuance_for_invocation(
        issuance_seed: u64,
        invocation_seed: u64,
    ) -> ExtentProviderIssuance {
        let base = issuance_seed * 16;
        let invocation_base = invocation_seed * 16;
        ExtentProviderIssuance::from_normalized_identities([
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
            invocation_base + 9,
            invocation_base + 10,
            invocation_base + 11,
            invocation_base + 12,
            invocation_base + 13,
        ])
        .expect("normalized provider issuance")
    }

    fn program_local_origin(seed: u64) -> ExtentProgramLocalOrigin {
        let base = seed * 16;
        ExtentProgramLocalOrigin::from_normalized_identities([
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
        ])
        .expect("normalized program-local origin")
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    fn grant(lineage: u64, base: u64, length: u64) -> Extent {
        root_grant(lineage).mint(base, length).expect("root extent")
    }

    fn root_grant(lineage: u64) -> ExtentRootGrant {
        ExtentRootGrant::from_admitted_provider(
            provider_issuance(lineage),
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
    }

    fn local_root_grant(lineage: u64, origin: u64) -> ExtentRootGrant {
        ExtentRootGrant::from_established_program_local(
            program_local_origin(origin),
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
    }

    #[test]
    fn validated_geometry_checks_no_wrap_before_authority_is_consumed() {
        let image = ValidatedExtentGeometry::check(0x1000, 0x800).expect("image geometry");
        let storage = ValidatedExtentGeometry::check(0x4000, 0x1000).expect("storage geometry");
        assert_eq!((image.base(), image.length()), (0x1000, 0x800));

        let overflow = ValidatedExtentGeometry::check(u64::MAX, 2)
            .expect_err("proof-level addition must not wrap");
        assert!(overflow.0.contains("overflows address width"));

        let image = root_grant(1).mint_validated(image);
        let storage = root_grant(2).mint_validated(storage);
        assert_eq!((image.base(), image.length()), (0x1000, 0x800));
        assert_eq!((storage.base(), storage.length()), (0x4000, 0x1000));
    }

    #[test]
    fn split_and_merge_conserve_one_authority_lineage() {
        let root = grant(1, 0x1000, 0x1000);
        let (lower, upper) = root.split_at(0x400).expect("split");
        assert_eq!((lower.base(), lower.length()), (0x1000, 0x400));
        assert_eq!((upper.base(), upper.length()), (0x1400, 0xc00));

        let restored = upper
            .merge(lower)
            .expect("sibling merge is order independent");
        assert_eq!((restored.base(), restored.length()), (0x1000, 0x1000));
        assert_eq!(restored.lineage_root().normalized_identity(), 1);
        assert_eq!(restored.provider_issuance(), Some(provider_issuance(1)));
    }

    #[test]
    fn owned_partition_retains_gaps_and_recomposes_exact_parent() {
        let partition = grant(1, 0x1000, 0x1000)
            .partition_owned(0x300, 0x500)
            .expect("middle allocation");
        assert_eq!(
            partition
                .before()
                .map(|extent| (extent.base(), extent.length())),
            Some((0x1000, 0x300))
        );
        assert_eq!(
            (partition.selected().base(), partition.selected().length()),
            (0x1300, 0x500)
        );
        assert_eq!(
            partition
                .after()
                .map(|extent| (extent.base(), extent.length())),
            Some((0x1800, 0x800))
        );

        let restored = partition.rejoin();
        assert_eq!((restored.base(), restored.length()), (0x1000, 0x1000));
        assert_eq!(restored.lineage_root().normalized_identity(), 1);
    }

    #[test]
    fn owned_partition_handles_parent_edges_without_empty_claims() {
        let lower = grant(1, 0, 100)
            .partition_owned(0, 40)
            .expect("lower allocation");
        assert!(lower.before().is_none());
        assert_eq!(lower.selected().length(), 40);
        assert_eq!(lower.after().map(Extent::length), Some(60));

        let upper = lower
            .rejoin()
            .partition_owned(40, 60)
            .expect("upper allocation");
        assert_eq!(upper.before().map(Extent::length), Some(40));
        assert_eq!(upper.selected().length(), 60);
        assert!(upper.after().is_none());

        let whole = upper
            .rejoin()
            .partition_owned(0, 100)
            .expect("whole allocation");
        assert!(whole.before().is_none());
        assert!(whole.after().is_none());
        assert_eq!(whole.selected().length(), 100);
    }

    #[test]
    fn failed_owned_partition_returns_original_authority() {
        let error = grant(1, 0x1000, 64)
            .partition_owned(60, 8)
            .expect_err("out-of-range owned allocation");
        assert!(error.diagnostic().0.contains("exceeds"));
        let original = error.into_extent();
        assert_eq!((original.base(), original.length()), (0x1000, 64));
    }

    #[test]
    fn nested_children_must_rejoin_before_their_parent_can_merge() {
        let (lower, upper) = grant(1, 0, 100).split_at(40).expect("root split");
        let (lower_a, lower_b) = lower.split_at(10).expect("nested split");

        let error = lower_a.merge(upper).expect_err("not siblings");
        let (lower_a, upper) = (*error).into_extents();
        let lower = lower_a.merge(lower_b).expect("restore lower child");
        let root = lower.merge(upper).expect("restore root");
        assert_eq!((root.base(), root.length()), (0, 100));
    }

    #[test]
    fn adjacency_never_merges_independent_grants() {
        let first = grant(1, 0, 64);
        let second = grant(2, 64, 64);
        let error = first.merge(second).expect_err("adjacency is not authority");
        assert!(error.diagnostic().0.contains("independent authority"));
        let (first, second) = (*error).into_extents();
        assert_eq!(first.length() + second.length(), 128);
    }

    #[test]
    fn attenuation_never_restores_or_widens_rights() {
        let extent = grant(1, 0, 64)
            .attenuate(rights(&[100]))
            .expect("remove write authority");
        let error = extent
            .attenuate(rights(&[100, 101]))
            .expect_err("cannot restore removed right");
        assert!(error.diagnostic().0.contains("cannot add"));
        assert_eq!(error.into_extent().rights(), &rights(&[100]));
    }

    #[test]
    fn incompatible_sibling_facts_cannot_launder_authority() {
        let (lower, upper) = grant(1, 0, 64).split_at(32).expect("split");
        let lower = lower
            .attenuate(rights(&[100]))
            .expect("attenuate lower child");
        let error = lower.merge(upper).expect_err("rights differ");
        assert!(error.diagnostic().0.contains("identical space, rights"));
    }

    #[test]
    fn equal_geometry_and_lineage_cannot_merge_different_provider_issuance() {
        let first = root_grant(1).mint(0, 32).expect("first provider root");
        let second = ExtentRootGrant::from_admitted_provider(
            provider_issuance(2),
            id(1, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
        .mint(32, 32)
        .expect("second provider root");

        let error = first
            .merge(second)
            .expect_err("matching numbers cannot erase provider issuance drift");
        assert!(error.diagnostic().0.contains("root-origin"));
    }

    #[test]
    fn matching_supply_cannot_merge_different_provider_invocations() {
        let first = root_grant(1).mint(0, 32).expect("first provider root");
        let second = ExtentRootGrant::from_admitted_provider(
            provider_issuance_for_invocation(1, 2),
            id(1, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
        .mint(32, 32)
        .expect("second provider root");

        let error = first
            .merge(second)
            .expect_err("matching supply cannot erase provider invocation drift");
        assert!(error.diagnostic().0.contains("root-origin"));
    }

    #[test]
    fn program_local_roots_conserve_their_exact_occurrence_origin() {
        let root = local_root_grant(7, 3)
            .mint(0x2000, 64)
            .expect("program-local root");
        assert_eq!(root.provider_issuance(), None);
        assert_eq!(root.program_local_origin(), Some(program_local_origin(3)));

        let loan = root.loan(8, 8).expect("local root loan");
        assert_eq!(loan.program_local_origin(), Some(program_local_origin(3)));
        drop(loan);

        let (lower, upper) = root.split_at(32).expect("local root split");
        let restored = lower.merge(upper).expect("local root rejoin");
        assert_eq!(
            restored.program_local_origin(),
            Some(program_local_origin(3))
        );
    }

    #[test]
    fn provider_and_local_origins_never_recompose() {
        let provider = root_grant(1).mint(0, 32).expect("provider root");
        let local = local_root_grant(1, 1).mint(32, 32).expect("local root");

        let error = provider
            .merge(local)
            .expect_err("equal lineage and geometry cannot erase origin kind");
        assert!(error.diagnostic().0.contains("root-origin"));
    }

    #[test]
    fn independent_program_local_occurrences_never_recompose() {
        let first = local_root_grant(1, 1)
            .mint(0, 32)
            .expect("first local root");
        let second = local_root_grant(1, 2)
            .mint(32, 32)
            .expect("second local root");

        let error = first
            .merge(second)
            .expect_err("equal lineage and geometry cannot erase local occurrence");
        assert!(error.diagnostic().0.contains("root-origin"));
    }

    #[test]
    fn loans_are_bounded_and_derive_parent_borrow_polarity() {
        let mut extent = grant(1, 0x1000, 64);
        let shared = extent.loan(4, 8).expect("shared loan");
        assert_eq!((shared.base(), shared.length()), (0x1004, 8));
        assert_eq!(shared.polarity(), LoanPolarity::Shared);
        assert_eq!(shared.provider_issuance(), Some(provider_issuance(1)));
        drop(shared);

        let exclusive = extent.loan_mut(16, 8).expect("exclusive loan");
        assert_eq!(exclusive.polarity(), LoanPolarity::Exclusive);
        drop(exclusive);

        assert!(extent.loan(60, 8).is_err());
    }

    #[test]
    fn failed_split_returns_the_original_authority() {
        let extent = grant(1, 0, 64);
        let error = extent.split_at(64).expect_err("empty upper child");
        assert_eq!(error.into_extent().length(), 64);
    }

    #[test]
    fn failed_mint_returns_the_admitted_root_grant() {
        let error = root_grant(1).mint(u64::MAX, 2).expect_err("overflow");
        assert!(error.diagnostic().0.contains("overflows"));
        let extent = error.into_grant().mint(0, 64).expect("retry valid mint");
        assert_eq!(extent.length(), 64);
    }

    #[test]
    fn provider_root_mints_exact_existing_content_authority_once() {
        let (extent, content) = root_grant(81)
            .mint_provider_existing_content(
                0x8000,
                64,
                ExtentContentInterpretation::from_sha256_commitment(
                    id(82, ExtentContentInterpretationId::from_normalized_identity),
                    [0x82; 32],
                ),
                id(85, ResidentClaimId::from_normalized_identity),
                id(83, ExtentContentValidityReceiptId::from_normalized_identity),
                id(84, ExtentContentCustodyReceiptId::from_normalized_identity),
            )
            .expect("provider existing-content root");
        assert_eq!(content.origin(), extent.origin());
        assert_eq!(content.lineage_root(), extent.lineage_root());
        assert_eq!((content.base(), content.length()), (0x8000, 64));
        assert_eq!(content.address_space(), extent.address_space());
        assert_eq!(content.provenance(), extent.provenance());
        assert_eq!(content.era(), extent.era());
        assert_eq!(
            content
                .interpretation()
                .compatibility_fingerprint()
                .normalized_identity(),
            82
        );
        assert_eq!(content.resident_claim().normalized_identity(), 85);
        assert_eq!(content.validity_receipt().normalized_identity(), 83);
        assert_eq!(content.custody_receipt().normalized_identity(), 84);
    }

    #[test]
    fn coincident_provider_content_issuances_retain_distinct_resident_claims() {
        let (_, first) = root_grant(91)
            .mint_provider_existing_content(
                0xa000,
                64,
                ExtentContentInterpretation::from_sha256_commitment(
                    id(92, ExtentContentInterpretationId::from_normalized_identity),
                    [0x92; 32],
                ),
                id(93, ResidentClaimId::from_normalized_identity),
                id(94, ExtentContentValidityReceiptId::from_normalized_identity),
                id(95, ExtentContentCustodyReceiptId::from_normalized_identity),
            )
            .expect("first resident provider issuance");
        let (_, second) = root_grant(96)
            .mint_provider_existing_content(
                0xa000,
                64,
                ExtentContentInterpretation::from_sha256_commitment(
                    id(92, ExtentContentInterpretationId::from_normalized_identity),
                    [0x92; 32],
                ),
                id(97, ResidentClaimId::from_normalized_identity),
                id(94, ExtentContentValidityReceiptId::from_normalized_identity),
                id(95, ExtentContentCustodyReceiptId::from_normalized_identity),
            )
            .expect("second resident provider issuance");

        assert_eq!(
            (first.base(), first.length()),
            (second.base(), second.length())
        );
        assert_eq!(first.interpretation(), second.interpretation());
        assert_eq!(first.validity_receipt(), second.validity_receipt());
        assert_eq!(first.custody_receipt(), second.custody_receipt());
        assert_ne!(first.origin(), second.origin());
        assert_ne!(first.resident_claim(), second.resident_claim());
    }

    #[test]
    fn program_local_root_cannot_mint_provider_existing_content_authority() {
        let error = local_root_grant(85, 86)
            .mint_provider_existing_content(
                0x9000,
                64,
                ExtentContentInterpretation::from_sha256_commitment(
                    id(87, ExtentContentInterpretationId::from_normalized_identity),
                    [0x87; 32],
                ),
                id(90, ResidentClaimId::from_normalized_identity),
                id(88, ExtentContentValidityReceiptId::from_normalized_identity),
                id(89, ExtentContentCustodyReceiptId::from_normalized_identity),
            )
            .expect_err("local capacity cannot assert provider-held content");
        assert!(error.diagnostic().0.contains("program-local"));
        let extent = error
            .into_grant()
            .mint(0x9000, 64)
            .expect("rejected content route returns the root grant");
        assert!(extent.program_local_origin().is_some());
    }

    fn external_grant(
        direction: ExternalLoanDirection,
        completion: CompletionObligations,
    ) -> ExternalLoanGrant {
        ExternalLoanGrant::from_admitted_provider(
            id(500, ExternalBorrowerId::from_normalized_identity),
            direction,
            id(10, AddressSpaceId::from_normalized_identity),
            id(20, ExtentProvenanceId::from_normalized_identity),
            rights(&[100]),
            completion,
        )
    }

    fn loan_id(identity: u64) -> ExternalLoanId {
        id(identity, ExternalLoanId::from_normalized_identity)
    }

    fn completion_fact(identity: u64) -> ExternalCompletionFactId {
        id(identity, ExternalCompletionFactId::from_normalized_identity)
    }

    fn reach_receipt(
        identity: ExternalLoanId,
        grant: &ExternalLoanGrant,
        loan: &ExtentLoan<'_>,
        mechanism: ExternalReachMechanism,
    ) -> ExternalReachReceipt {
        ExternalReachReceipt::from_admitted_provider(
            id(800, ExternalReachReceiptId::from_normalized_identity),
            identity,
            grant,
            loan,
            mechanism,
            true,
        )
    }

    #[test]
    fn external_read_loan_requires_completion_facts_before_releasing_borrow() {
        let extent = grant(1, 0x1000, 64);
        let loan = extent.loan(0, 32).expect("shared DMA source");
        let grant = external_grant(
            ExternalLoanDirection::DeviceReads,
            CompletionObligations::from_normalized_facts([
                completion_fact(700),
                completion_fact(701),
            ]),
        );
        let reach = reach_receipt(
            loan_id(600),
            &grant,
            &loan,
            ExternalReachMechanism::AdmittedBorrowerContract,
        );
        let transfer = begin_external_loan(loan, loan_id(600), &grant, Some(reach))
            .expect("admitted external read");
        assert_eq!(transfer.direction(), ExternalLoanDirection::DeviceReads);

        let incomplete = ExternalCompletionReceipt::from_admitted_provider(
            &transfer,
            true,
            [completion_fact(701)],
        );
        let error = transfer
            .complete(incomplete)
            .expect_err("required device fence is missing");
        assert!(error.diagnostic().0.contains("lacks facts"));
        let (transfer, _) = (*error).into_parts();

        let complete = ExternalCompletionReceipt::from_admitted_provider(
            &transfer,
            true,
            [completion_fact(700), completion_fact(701)],
        );
        let completion = transfer.complete(complete).expect("completed DMA read");
        assert_eq!((completion.base, completion.length), (0x1000, 32));
    }

    #[test]
    fn external_write_loan_derives_exclusive_cpu_exclusion() {
        let mut extent = grant(1, 0x2000, 64);
        let shared = extent.loan(0, 16).expect("shared loan");
        let write_grant = external_grant(
            ExternalLoanDirection::DeviceWrites,
            CompletionObligations::default(),
        );
        let shared_reach = reach_receipt(
            loan_id(601),
            &write_grant,
            &shared,
            ExternalReachMechanism::HardwareIsolation,
        );
        let error = begin_external_loan(shared, loan_id(601), &write_grant, Some(shared_reach))
            .expect_err("device mutation needs exclusive custody");
        assert!(error.diagnostic().0.contains("exclusive"));
        drop((*error).into_loan());

        let exclusive = extent.loan_mut(0, 16).expect("exclusive loan");
        let exclusive_reach = reach_receipt(
            loan_id(602),
            &write_grant,
            &exclusive,
            ExternalReachMechanism::HardwareIsolation,
        );
        let transfer =
            begin_external_loan(exclusive, loan_id(602), &write_grant, Some(exclusive_reach))
                .expect("admitted external write");
        let receipt = ExternalCompletionReceipt::from_admitted_provider(&transfer, true, []);
        let completion = transfer.complete(receipt).expect("completed DMA write");
        assert_eq!(completion.borrower.normalized_identity(), 500);
        assert_eq!(completion.reach_receipt.normalized_identity(), 800);
    }

    #[test]
    fn external_agent_reach_must_equal_the_lent_extent_and_fail_closed() {
        let extent = grant(1, 0x3000, 128);
        let loan = extent.loan(32, 32).expect("DMA subrange");
        let read_grant = external_grant(
            ExternalLoanDirection::DeviceReads,
            CompletionObligations::default(),
        );
        let missing = begin_external_loan(loan, loan_id(603), &read_grant, None)
            .expect_err("an invisible borrower without reach evidence must reject");
        assert!(missing.diagnostic().0.contains("reach evidence"));
        let loan = (*missing).into_loan();

        let mut overbroad_reach = reach_receipt(
            loan_id(603),
            &read_grant,
            &loan,
            ExternalReachMechanism::HardwareIsolation,
        );
        overbroad_reach.base = 0x3000;
        overbroad_reach.length = 128;
        let overbroad = begin_external_loan(loan, loan_id(603), &read_grant, Some(overbroad_reach))
            .expect_err("whole-parent reach exceeds the exact lent subrange");
        assert!(overbroad.diagnostic().0.contains("exact lent extent range"));
        let loan = (*overbroad).into_loan();

        let exact_reach = reach_receipt(
            loan_id(603),
            &read_grant,
            &loan,
            ExternalReachMechanism::HardwareIsolation,
        );
        let transfer = begin_external_loan(loan, loan_id(603), &read_grant, Some(exact_reach))
            .expect("exact subrange isolation");
        assert_eq!((transfer.base(), transfer.length()), (0x3020, 32));
        assert_eq!(
            transfer.reach_mechanism(),
            ExternalReachMechanism::HardwareIsolation
        );
    }

    #[test]
    fn external_completion_cannot_replay_after_lent_authority_drift() {
        let first_extent = grant(1, 0x4000, 64);
        let read_grant = external_grant(
            ExternalLoanDirection::DeviceReads,
            CompletionObligations::default(),
        );
        let first_loan = first_extent.loan(0, 16).expect("first DMA range");
        let first_reach = reach_receipt(
            loan_id(604),
            &read_grant,
            &first_loan,
            ExternalReachMechanism::HardwareIsolation,
        );
        let first = begin_external_loan(first_loan, loan_id(604), &read_grant, Some(first_reach))
            .expect("first external loan");
        let stale = ExternalCompletionReceipt::from_admitted_provider(&first, true, []);

        let second_extent = grant(2, 0x4000, 64);
        let second_loan = second_extent.loan(0, 16).expect("second DMA range");
        let second_reach = reach_receipt(
            loan_id(604),
            &read_grant,
            &second_loan,
            ExternalReachMechanism::HardwareIsolation,
        );
        let second =
            begin_external_loan(second_loan, loan_id(604), &read_grant, Some(second_reach))
                .expect("second external loan");

        let error = second
            .complete(stale)
            .expect_err("completion for another authority lineage must not replay");
        assert!(error.diagnostic().0.contains("authority lineage"));
    }
}
