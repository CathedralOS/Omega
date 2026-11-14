// Domain-separated 32-byte optimization identities (`canonical_identity!`,
// `CanonicalIdentityEncoder`).
//
// Every identity hashes its unique NUL-terminated domain string, the `u64`
// little-endian canonical byte length, and the canonical byte sequence.
// `CanonicalIdentityEncoder` allows callers that know `canonical_len` up front
// to stream the identical byte sequence without materializing a contiguous
// buffer.

pub(crate) const IDENTITY_WIDTH: usize = 32;

fn domain_digest(domain: &[u8], canonical: &[u8]) -> [u8; IDENTITY_WIDTH] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
    digest.bytes(canonical);
    digest.finish()
}

pub struct CanonicalIdentityEncoder(crate::digest_encoder::DomainDigestEncoder);

impl CanonicalIdentityEncoder {
    fn new(domain: &[u8], canonical_len: u64) -> Self {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
        digest.u64(canonical_len);
        Self(digest)
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }

    fn finish(self) -> [u8; IDENTITY_WIDTH] {
        self.0.finish()
    }
}

macro_rules! canonical_identity {
    ($name:ident, $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; super::IDENTITY_WIDTH]);

        impl $name {
            pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
                Self(super::domain_digest($domain, canonical))
            }

            pub fn canonical_encoder(canonical_len: u64) -> super::CanonicalIdentityEncoder {
                super::CanonicalIdentityEncoder::new($domain, canonical_len)
            }

            pub fn from_canonical_encoder(encoder: super::CanonicalIdentityEncoder) -> Self {
                Self(encoder.finish())
            }

            pub const fn from_bytes(bytes: [u8; super::IDENTITY_WIDTH]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; super::IDENTITY_WIDTH] {
                self.0
            }

            pub fn encode(self) -> [u8; super::IDENTITY_WIDTH] {
                self.0
            }

            pub fn decode(encoded: &[u8]) -> Result<Self, super::IdentityDecodeError> {
                let bytes: [u8; super::IDENTITY_WIDTH] =
                    encoded
                        .try_into()
                        .map_err(|_| super::IdentityDecodeError::WrongLength {
                            expected: super::IDENTITY_WIDTH,
                            actual: encoded.len(),
                        })?;
                Ok(Self(bytes))
            }
        }
    };
}

macro_rules! s {
    ($name:ident => $domain:literal) => {
        canonical_identity!($name, $domain);
    };
}

mod artifacts;
mod bundle;
mod decisions;
mod error;
mod rules_and_facts;
mod stages;

pub use artifacts::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedOrdinaryCallableEntryManifestIdentity, OptimizedTerminalOrdinaryCallableEntryIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
pub use bundle::{OptimizationIdentityBundle, OptimizationIdentityBundleIdentity};
pub use decisions::{
    OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationUnitIdentity, OptimizationValidatorIdentity, OptimizationWorkloadProfileIdentity,
    TargetCostModelIdentity, TransformationLedgerIdentity,
};
pub(crate) use error::IdentityDecodeError;
pub use rules_and_facts::{
    AcceptedObligationFactIdentity, OptimizationCandidateIdentity, OptimizationPassIdentity,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OwnershipFrontierFactIdentity,
    ProofQuestionIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
pub use stages::{
    FunctionRelativeOptimizationRealizationManifestIdentity, NativeOptimizationProjectionIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity, PostAllocationOptimizationManifestIdentity,
    PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

#[cfg(test)]
mod tests;
