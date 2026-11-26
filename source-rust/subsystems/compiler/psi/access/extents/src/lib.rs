//! Extent coordinates, rights and interpretation commitments carried by programs.
//! None of these values issues, loans, maps or mutates live storage authority.
#![forbid(unsafe_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentDiagnostic(pub String);

impl std::fmt::Display for ExtentDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExtentDiagnostic {}

use arena::NormalizedIdentitySet;

macro_rules! normalized_extent_identity {
    ($(#[$meta:meta])* $name:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);
        impl record_storage::Record for $name {
            fn write(&self, output: &mut record_storage::RecordWriter) -> Result<(), record_storage::RecordError> { record_storage::Record::write(&self.0, output) }
            fn read<Context>(input: &mut record_storage::RecordReader<'_, Context>) -> Result<Self, record_storage::RecordError> {
                Self::from_normalized_identity(record_storage::Record::read(input)?)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
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
pub type ExtentRights = NormalizedIdentitySet<ExtentRightId>;
normalized_extent_identity! {
    ActivationClaimSiteId, "activation-claim-site"
}
normalized_extent_identity! { ExternalBorrowerId, "external-borrower" }
normalized_extent_identity! { ExternalLoanId, "external-loan" }
normalized_extent_identity! { ExternalCompletionFactId, "external-completion-fact" }
normalized_extent_identity! { ExternalReachReceiptId, "external-reach receipt" }
normalized_extent_identity! { OrderingEventId, "ordering-event" }
normalized_extent_identity! {
    DeviceInstanceId, "device-instance"
}
normalized_extent_identity! {
    RuntimeScopeId, "runtime-scope"
}
normalized_extent_identity! { MappingGrantId, "mapping-grant" }
normalized_extent_identity! { MappingId, "mapping" }
normalized_extent_identity! { TranslationActivationFactId, "translation-activation-fact" }
normalized_extent_identity! { TranslationCompletionFactId, "translation-completion-fact" }
normalized_extent_identity! { PeerWriteRevocationFactId, "peer-write-revocation-fact" }
fn nonzero_identity(identity: u64, name: &str) -> Result<(), ExtentDiagnostic> {
    if identity == 0 {
        return Err(ExtentDiagnostic(format!(
            "normalized {name} identity cannot be zero"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoanPolarity {
    Shared,
    Exclusive,
}
