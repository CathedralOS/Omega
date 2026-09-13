//! Optimizer module role: stage group. Canonical, domain-separated optimization identities.
//! Shared digest framing routes into rule/fact, stage, artifact, decision, and bundle ownership.

use sha2::{Digest, Sha256};

const IDENTITY_WIDTH: usize = 32;

fn domain_digest(domain: &[u8], canonical: &[u8]) -> [u8; IDENTITY_WIDTH] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(
        u64::try_from(canonical.len())
            .expect("canonical optimization identity input length fits u64")
            .to_le_bytes(),
    );
    digest.update(canonical);
    digest.finalize().into()
}

/// Incremental [`domain_digest`] input. The domain and total canonical length
/// are hashed up front, then the identical content byte sequence streams
/// through `update`; `finish` yields the same digest without materializing
/// the whole encoding. The digest frame requires the length before content,
/// so callers derive it from a prior count over the same deterministic walk.
pub struct CanonicalIdentityEncoder(Sha256);

impl CanonicalIdentityEncoder {
    fn new(domain: &[u8], canonical_len: u64) -> Self {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update(canonical_len.to_le_bytes());
        Self(digest)
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn finish(self) -> [u8; IDENTITY_WIDTH] {
        self.0.finalize().into()
    }
}

macro_rules! canonical_identity {
    ($name:ident, $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; super::IDENTITY_WIDTH]);

        impl $name {
            /// Derive this identity from the owner's canonical,
            /// relocation-independent encoding.
            pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
                Self(super::domain_digest($domain, canonical))
            }

            /// Streaming counterpart of `from_canonical_bytes`: returns an
            /// encoder seeded with this identity's domain and the total
            /// canonical length, for feeding the identical byte sequence.
            pub fn canonical_encoder(canonical_len: u64) -> super::CanonicalIdentityEncoder {
                super::CanonicalIdentityEncoder::new($domain, canonical_len)
            }

            /// Complete a streaming encode. The resulting identity equals
            /// `from_canonical_bytes` over the same byte sequence.
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
    OptimizedOrdinaryCallableEntryManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
pub use bundle::{
    IdentityBundleDecodeError, OptimizationIdentityBundle, OptimizationIdentityBundleIdentity,
};
pub use decisions::{
    OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationUnitIdentity, OptimizationValidatorIdentity, OptimizationWorkloadProfileIdentity,
    TargetCostModelIdentity, TransformationLedgerIdentity,
};
pub use error::IdentityDecodeError;
pub use rules_and_facts::{
    AcceptedObligationFactIdentity, DuplicateOptimizationRuleIdentity,
    OptimizationCandidateIdentity, OptimizationPassIdentity, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OwnershipFrontierFactIdentity, ProofQuestionIdentity,
    ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
pub use stages::{
    FunctionRelativeOptimizationRealizationManifestIdentity, NativeOptimizationProjectionIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};

#[cfg(test)]
mod tests;
