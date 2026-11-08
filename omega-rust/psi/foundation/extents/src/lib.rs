/*
We model conserved authority over concrete address-space ranges (`Extent`),
where an `Extent` is an linear capability carrier rather than an allocator or a
raw numeric address. Fresh root extents enter solely through admitted provider
issuances or program-local origins (`roots`), and every subsequent operation
(`split`, `partition`, `attenuate`, `loan`, `merge`, external lending, or
address-space mapping) conserves authority and returns the unconsumed carrier
on failure.
*/

/*
Systems code that manipulates DMA buffers, MMIO ranges, page mappings, and
runtime-sized activation storage requires static proof that address-space
authority is neither forged from raw integers nor leaked across device or
translation handoffs. Grouping nonzero identity sets (`identities`), borrow
loans (`loans`), activation claim ledgers (`activation_claims`), external device
loans (`external_loans`), device ordering events (`ordering_events`), and
translation mappings (`mapping`) in one target-neutral crate gives Psi and
Omega a single conservation calculus.
*/

/*
`normalized_extent_identity!` enforces nonzero `u64` identities across all
provenance, lineage, era, and obligation records. `extent` owns the core
linear carrier, `loans_and_claims` owns activation claims, external borrower
loans, and device ordering admission, and `mapping` owns address-space
translation grants and revocation receipts.
*/

#![forbid(unsafe_code)]
#![allow(
    clippy::result_large_err,
    reason = "extent failures deliberately return the conserved authority needed for retry"
)]

mod loans_and_claims;
use loans_and_claims::{activation_claims, external_loans, ordering_events};
mod extent;
mod identities {
    use std::collections::BTreeSet;

    use crate::extent::diagnostic::{ExtentDiagnostic, nonzero_identity};

    macro_rules! normalized_extent_identity {
        ($(#[$meta:meta])* $name:ident, $label:literal) => {
            $(#[$meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name(u64);
            impl compiler_storage::Record for $name {
                fn write(&self, output: &mut compiler_storage::RecordWriter) -> Result<(), compiler_storage::RecordError> { compiler_storage::Record::write(&self.0, output) }
                fn read<Context>(input: &mut compiler_storage::RecordReader<'_, Context>) -> Result<Self, compiler_storage::RecordError> {
                    Self::from_normalized_identity(compiler_storage::Record::read(input)?)
                        .map_err(|_| input.error(concat!($label, " identity has the reserved zero value")))
                }
            }

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

    pub(crate) use normalized_extent_identity;
    normalized_extent_identity! { AddressSpaceId, "address-space" }
    normalized_extent_identity! { ExtentProvenanceId, "extent-provenance" }
    normalized_extent_identity! { MappingEraId, "mapping-era" }
    normalized_extent_identity! { ExtentLineageId, "extent-lineage" }
    normalized_extent_identity! { ExtentIssuanceId, "extent-issuance" }
    normalized_extent_identity! { ExtentBackingId, "extent-backing" }
    normalized_extent_identity! { ExtentProviderId, "extent-provider" }
    normalized_extent_identity! { ExtentLiveIssuancePremiseId, "extent-live-issuance-premise" }
    normalized_extent_identity! { ExtentCustodyRootId, "extent-custody-root" }
    normalized_extent_identity! { ExtentAliasClassId, "extent-alias-class" }
    normalized_extent_identity! { ExtentProviderCorrespondenceId, "extent-provider-correspondence" }
    normalized_extent_identity! { ExtentTrustProvenanceId, "extent-trust-provenance" }
    normalized_extent_identity! { ExtentProviderPlanId, "extent-provider-plan" }
    normalized_extent_identity! { ExtentProviderInvocationId, "extent-provider-invocation" }
    normalized_extent_identity! { ExtentEstablishmentRouteId, "extent-establishment-route" }
    normalized_extent_identity! { ExtentCapacityId, "extent-capacity" }
    normalized_extent_identity! { ExtentQualificationId, "extent-qualification" }
    normalized_extent_identity! { ExtentContentInterpretationId, "extent-content-interpretation" }
    normalized_extent_identity! { ExtentContentValidityReceiptId, "extent-content-validity-receipt" }
    normalized_extent_identity! { ExtentContentCustodyReceiptId, "extent-content-custody-receipt" }
    normalized_extent_identity! { ResidentClaimId, "resident-claim" }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
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

    normalized_extent_identity! { ExtentRightId, "extent-right" }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NormalizedIdentitySet<Id>(BTreeSet<Id>);

    impl<Id> Default for NormalizedIdentitySet<Id> {
        fn default() -> Self {
            Self(BTreeSet::new())
        }
    }

    impl<Id: Copy + Ord> NormalizedIdentitySet<Id> {
        pub const fn none() -> Self {
            Self(BTreeSet::new())
        }

        pub fn from_normalized_identities(identities: impl IntoIterator<Item = Id>) -> Self {
            Self(identities.into_iter().collect())
        }

        pub fn union(&self, other: &Self) -> Self {
            Self(self.0.union(&other.0).copied().collect())
        }

        pub fn contains(&self, required: &Self) -> bool {
            required.0.is_subset(&self.0)
        }

        pub fn identities(&self) -> impl Iterator<Item = Id> + '_ {
            self.0.iter().copied()
        }
    }

    pub type ExtentRights = NormalizedIdentitySet<ExtentRightId>;
}
mod loans {
    use crate::extent::Extent;
    use crate::extent::diagnostic::ExtentDiagnostic;
    use crate::identities::{
        AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
    };
    use crate::roots::root_origins::{
        ExtentProgramLocalOrigin, ExtentProviderIssuance, ExtentRootOrigin,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LoanPolarity {
        Shared,
        Exclusive,
    }

    enum LoanBacking<'a> {
        Shared(&'a Extent),
        Exclusive(&'a mut Extent),
    }

    pub struct ExtentLoan<'a> {
        backing: LoanBacking<'a>,
        base: u64,
        length: u64,
    }

    impl<'a> ExtentLoan<'a> {
        pub(crate) fn shared(
            extent: &'a Extent,
            offset: u64,
            length: u64,
        ) -> Result<Self, ExtentDiagnostic> {
            let base = validate_subrange(extent, offset, length)?;
            Ok(Self {
                backing: LoanBacking::Shared(extent),
                base,
                length,
            })
        }

        pub(crate) fn exclusive(
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

    fn validate_subrange(
        extent: &Extent,
        offset: u64,
        length: u64,
    ) -> Result<u64, ExtentDiagnostic> {
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
}
mod mapping;
mod roots {
    pub(crate) mod root_grants {
        use crate::extent::diagnostic::{ExtentDiagnostic, validate_range};
        use crate::extent::{Extent, ExtentSharingMode, Lineage};
        use crate::identities::{
            AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentInterpretation,
            ExtentContentValidityReceiptId, ExtentLineageId, ExtentProvenanceId, ExtentRights,
            MappingEraId, ResidentClaimId,
        };
        use crate::roots::root_origins::{
            ExtentProgramLocalOrigin, ExtentProviderIssuance, ExtentRootOrigin,
        };

        #[derive(Debug, PartialEq, Eq)]
        pub struct ExtentRootGrant {
            origin: ExtentRootOrigin,
            lineage: ExtentLineageId,
            address_space: AddressSpaceId,
            rights: ExtentRights,
            provenance: ExtentProvenanceId,
            era: MappingEraId,
            sharing: ExtentSharingMode,
        }

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
                    sharing: ExtentSharingMode::Exclusive,
                }
            }

            pub const fn admitting_peer_shared_backing(mut self) -> Self {
                self.sharing = ExtentSharingMode::PeerShared;
                self
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
                    sharing: ExtentSharingMode::Exclusive,
                }
            }

            pub const fn origin(&self) -> ExtentRootOrigin {
                self.origin
            }

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

            pub fn mint_provider_existing_content(
                self,
                base: u64,
                length: u64,
                interpretation: ExtentContentInterpretation,
                resident_claim: ResidentClaimId,
                validity_receipt: ExtentContentValidityReceiptId,
                custody_receipt: ExtentContentCustodyReceiptId,
            ) -> Result<(Extent, ProviderExistingContentGrant), ExistingContentMintError>
            {
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
                            "program-local roots cannot issue provider existing-content evidence"
                                .into(),
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

            pub fn mint_validated(self, geometry: ValidatedExtentGeometry) -> Extent {
                let Self {
                    origin,
                    lineage,
                    address_space,
                    rights,
                    provenance,
                    era,
                    sharing,
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
                    sharing,
                }
            }
        }

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
    }

    pub(crate) mod root_origins {
        use crate::extent::diagnostic::{ExtentDiagnostic, nonzero_identity};
        use crate::identities::{
            ExtentAliasClassId, ExtentBackingId, ExtentCapacityId, ExtentCustodyRootId,
            ExtentEstablishmentRouteId, ExtentIssuanceId, ExtentLiveIssuancePremiseId,
            ExtentProviderCorrespondenceId, ExtentProviderId, ExtentProviderInvocationId,
            ExtentProviderPlanId, ExtentQualificationId, ExtentTrustProvenanceId,
        };

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
            pub fn from_normalized_identities(
                identities: [u64; 13],
            ) -> Result<Self, ExtentDiagnostic> {
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
            pub fn from_normalized_identities(
                identities: [u64; 8],
            ) -> Result<Self, ExtentDiagnostic> {
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
    }
}
pub use activation_claims::{
    ActivationClaim, ActivationClaimBranch, ActivationClaimId, ActivationClaimLedger,
    ActivationClaimProvenance, ActivationClaimRequest, ActivationClaimSiteId, ClaimBoundRow,
    ClaimDrainError, ClaimEstablishmentError, compose_claim_bounds,
    validate_committed_within_bound,
};
pub use extent::diagnostic::ExtentDiagnostic;
pub use extent::{
    AttenuationError, Extent, ExtentSharingMode, MergeError, OwnedExtentPartition,
    OwnedPartitionError, SplitError,
};
pub use external_loans::{
    CompletionObligations, ExternalBorrowerId, ExternalCompletionError, ExternalCompletionFactId,
    ExternalCompletionReceipt, ExternalLoan, ExternalLoanCompletion, ExternalLoanDirection,
    ExternalLoanGrant, ExternalLoanId, ExternalLoanStartError, ExternalReachMechanism,
    ExternalReachReceipt, ExternalReachReceiptId, begin_external_loan,
};
pub use identities::{
    AddressSpaceId, ExtentAliasClassId, ExtentBackingId, ExtentCapacityId,
    ExtentContentCustodyReceiptId, ExtentContentInterpretation, ExtentContentInterpretationId,
    ExtentContentValidityReceiptId, ExtentCustodyRootId, ExtentEstablishmentRouteId,
    ExtentIssuanceId, ExtentLineageId, ExtentLiveIssuancePremiseId, ExtentProvenanceId,
    ExtentProviderCorrespondenceId, ExtentProviderId, ExtentProviderInvocationId,
    ExtentProviderPlanId, ExtentQualificationId, ExtentRightId, ExtentRights,
    ExtentTrustProvenanceId, MappingEraId, NormalizedIdentitySet, ResidentClaimId,
};
pub use loans::{ExtentLoan, LoanPolarity};
pub use mapping::{
    BorrowedMappingError, MapActivationError, MappedExtent, MappedRangeReceiptContext,
    MappingGrant, MappingGrantId, MappingId, MappingReceiptContext, MappingSourceMode,
    MappingTransitionReceipt, OwnedMappingError, PeerWriteRevocationError,
    PeerWriteRevocationFactId, PeerWriteRevocationObligations, PeerWriteRevocationReceipt,
    PeerWriteRevocationStartError, PendingMap, PendingPeerWriteRevocation, PendingUnmap,
    TranslationActivationFactId, TranslationActivationReceipt, TranslationCompletionFactId,
    TranslationInstallObligations, TranslationReleaseObligations, TranslationReleaseReceipt,
    UnmapCompletionError, UnmappedExtents, map_borrowed, map_owned,
};
pub use ordering_events::{
    DeviceInstanceId, DeviceOrderingRole, OrderingAuthority, OrderingCoordinateKind,
    OrderingCoverageAdmission, OrderingCoverageRejection, OrderingCoverageRequirement,
    OrderingEvent, OrderingEventId, OrderingRange, RuntimeScopeId,
    admit_acquisition_after_completion, admit_role_coverage,
};
pub use roots::root_grants::{
    ExistingContentMintError, ExtentRootGrant, MintError, ProviderExistingContentGrant,
    ValidatedExtentGeometry,
};
pub use roots::root_origins::{
    ExtentProgramLocalOrigin, ExtentProviderInvocation, ExtentProviderIssuance, ExtentRootOrigin,
};

#[cfg(test)]
mod tests {
    mod conservation {
        use super::{
            grant, id, local_root_grant, program_local_origin, provider_issuance, rights,
            root_grant,
        };
        use crate::{
            AddressSpaceId, Extent, ExtentLineageId, ExtentProvenanceId, ExtentRootGrant,
            MappingEraId, ValidatedExtentGeometry,
        };

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
        fn program_local_roots_conserve_their_exact_occurrence_origin() {
            let root = local_root_grant(7, 3)
                .mint(0x2000, 64)
                .expect("program-local root");
            assert_eq!(root.provider_issuance(), None);
            assert_eq!(root.program_local_origin(), Some(program_local_origin(3)));

            let loan = root.loan(8, 8).expect("local root loan");
            assert_eq!(loan.program_local_origin(), Some(program_local_origin(3)));

            let (lower, upper) = root.split_at(32).expect("local root split");
            let restored = lower.merge(upper).expect("local root rejoin");
            assert_eq!(
                restored.program_local_origin(),
                Some(program_local_origin(3))
            );
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
    }
    mod existing_content {
        use super::{id, local_root_grant, root_grant};
        use crate::{
            ExtentContentCustodyReceiptId, ExtentContentInterpretation,
            ExtentContentInterpretationId, ExtentContentValidityReceiptId, ResidentClaimId,
        };

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
    }
    mod loans {
        use super::{grant, provider_issuance, root_grant};
        use crate::{ExtentSharingMode, LoanPolarity};

        #[test]
        fn loans_are_bounded_and_derive_parent_borrow_polarity() {
            let mut extent = grant(1, 0x1000, 64);
            let shared = extent.loan(4, 8).expect("shared loan");
            assert_eq!((shared.base(), shared.length()), (0x1004, 8));
            assert_eq!(shared.polarity(), LoanPolarity::Shared);
            assert_eq!(shared.provider_issuance(), Some(provider_issuance(1)));

            let exclusive = extent.loan_mut(16, 8).expect("exclusive loan");
            assert_eq!(exclusive.polarity(), LoanPolarity::Exclusive);

            assert!(extent.loan(60, 8).is_err());
        }

        #[test]
        fn empty_loans_retain_identity_and_obey_parent_bounds() {
            let mut extent = grant(1, 0x1000, 64);
            for offset in [0, 32, 64] {
                let shared = extent.loan(offset, 0).expect("contained empty shared loan");
                assert_eq!((shared.base(), shared.length()), (0x1000 + offset, 0));
                assert_eq!(shared.provider_issuance(), Some(provider_issuance(1)));
                assert_eq!(shared.polarity(), LoanPolarity::Shared);
                let exclusive = extent
                    .loan_mut(offset, 0)
                    .expect("contained empty exclusive loan");
                assert_eq!(exclusive.polarity(), LoanPolarity::Exclusive);
                assert_eq!(exclusive.provider_issuance(), Some(provider_issuance(1)));
            }
            assert!(extent.loan(65, 0).is_err());
            assert!(extent.loan_mut(65, 0).is_err());
            assert!(extent.loan(u64::MAX, 1).is_err());
        }

        #[test]
        fn peer_writable_backing_never_yields_an_exclusive_borrow() {
            let mut extent = root_grant(1)
                .admitting_peer_shared_backing()
                .mint(0x1000, 64)
                .expect("peer-shared root extent");
            assert_eq!(extent.sharing(), ExtentSharingMode::PeerShared);

            let refused = extent
                .loan_mut(0, 8)
                .expect_err("writable peer has no exclusive borrow");
            assert!(refused.0.contains("writable peer"));
            assert!(extent.loan_mut(0, 0).is_err());

            let shared = extent.loan(0, 8).expect("shared reads remain");
            assert_eq!(shared.polarity(), LoanPolarity::Shared);

            let (mut lower, mut upper) = extent.split_at(32).expect("peer-shared split");
            assert!(lower.loan_mut(0, 8).is_err());
            assert!(upper.loan_mut(0, 8).is_err());
            let partition = upper.partition_owned(0, 16).expect("peer-shared partition");
            assert_eq!(
                partition.selected().sharing(),
                ExtentSharingMode::PeerShared
            );
            let (before, mut selected, after) = partition.into_parts();
            assert!(selected.loan_mut(0, 8).is_err());
            for piece in [before, after].into_iter().flatten() {
                assert_eq!(piece.sharing(), ExtentSharingMode::PeerShared);
            }
            assert!(lower.loan(0, 8).is_ok());
        }
    }

    use crate::{
        AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProgramLocalOrigin,
        ExtentProvenanceId, ExtentProviderIssuance, ExtentRightId, ExtentRights, ExtentRootGrant,
        MappingEraId,
    };

    pub(crate) fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    pub(crate) fn provider_issuance(seed: u64) -> ExtentProviderIssuance {
        provider_issuance_for_invocation(seed, seed)
    }

    pub(crate) fn provider_issuance_for_invocation(
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

    pub(crate) fn program_local_origin(seed: u64) -> ExtentProgramLocalOrigin {
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

    pub(crate) fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    pub(crate) fn grant(lineage: u64, base: u64, length: u64) -> Extent {
        root_grant(lineage).mint(base, length).expect("root extent")
    }

    pub(crate) fn root_grant(lineage: u64) -> ExtentRootGrant {
        ExtentRootGrant::from_admitted_provider(
            provider_issuance(lineage),
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
    }

    pub(crate) fn local_root_grant(lineage: u64, origin: u64) -> ExtentRootGrant {
        ExtentRootGrant::from_established_program_local(
            program_local_origin(origin),
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(10, AddressSpaceId::from_normalized_identity),
            rights(&[100, 101]),
            id(20, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
    }
}
