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
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

#[cfg(test)]
mod tests;
