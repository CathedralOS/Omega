use super::OptimizationSelectionIdentity;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

const IDENTITY_WIDTH: usize = 32;
const BUNDLE_MAGIC: &[u8; 8] = b"OMGIDB\0\0";
const BUNDLE_VERSION: u32 = 1;

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
        pub struct $name([u8; IDENTITY_WIDTH]);

        impl $name {
            /// Derive this identity from the owning component's canonical,
            /// relocation-independent encoding.
            pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
                Self(domain_digest($domain, canonical))
            }

            pub const fn from_bytes(bytes: [u8; IDENTITY_WIDTH]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; IDENTITY_WIDTH] {
                self.0
            }

            pub fn encode(self) -> [u8; IDENTITY_WIDTH] {
                self.0
            }

            pub fn decode(encoded: &[u8]) -> Result<Self, IdentityDecodeError> {
                let bytes: [u8; IDENTITY_WIDTH] =
                    encoded
                        .try_into()
                        .map_err(|_| IdentityDecodeError::WrongLength {
                            expected: IDENTITY_WIDTH,
                            actual: encoded.len(),
                        })?;
                Ok(Self(bytes))
            }
        }
    };
}

canonical_identity!(
    OptimizationRuleIdentity,
    b"omega.optimization-rule-identity.v1\0"
);
canonical_identity!(
    OptimizationPassIdentity,
    b"omega.optimization-pass-identity.v1\0"
);
canonical_identity!(
    OptimizationCandidateIdentity,
    b"omega.optimization-candidate-identity.v1\0"
);
canonical_identity!(
    ScalarConstantFactIdentity,
    b"omega.scalar-constant-fact-identity.v1\0"
);
canonical_identity!(
    AcceptedObligationFactIdentity,
    b"omega.accepted-obligation-fact-identity.v1\0"
);
canonical_identity!(ProofQuestionIdentity, b"omega.proof-question-identity.v1\0");
canonical_identity!(
    OwnershipFrontierFactIdentity,
    b"omega.ownership-frontier-fact-identity.v1\0"
);
canonical_identity!(
    PrePhysicalOptimizationManifestIdentity,
    b"omega.pre-physical-optimization-manifest-identity.v2\0"
);
canonical_identity!(
    PostAllocationOptimizationManifestIdentity,
    b"omega.post-allocation-optimization-manifest-identity.v1\0"
);
canonical_identity!(
    SelectedLoweringOptimizationCompletionIdentity,
    b"omega.selected-lowering-optimization-completion-identity.v1\0"
);
canonical_identity!(
    FunctionRelativeOptimizationRealizationManifestIdentity,
    b"omega.function-relative-optimization-realization-manifest-identity.v1\0"
);
canonical_identity!(
    TerminalFunctionFragmentEmissionIdentity,
    b"omega.terminal-function-fragment-emission-identity.v1\0"
);
canonical_identity!(
    FunctionFragmentEmissionManifestIdentity,
    b"omega.function-fragment-emission-manifest-identity.v1\0"
);
canonical_identity!(
    TerminalRelocationFreeTextSectionIdentity,
    b"omega.terminal-relocation-free-text-section-identity.v1\0"
);
canonical_identity!(
    FunctionFragmentTextSectionManifestIdentity,
    b"omega.function-fragment-text-section-manifest-identity.v1\0"
);
canonical_identity!(
    TerminalRelocationFreeObjectPlanIdentity,
    b"omega.terminal-relocation-free-object-plan-identity.v1\0"
);
canonical_identity!(
    TerminalRelocationFreeObjectContainerIdentity,
    b"omega.terminal-relocation-free-object-container-identity.v1\0"
);
canonical_identity!(
    FunctionFragmentObjectContainerManifestIdentity,
    b"omega.function-fragment-object-container-manifest-identity.v1\0"
);
canonical_identity!(
    OptimizedTerminalObjectArtifactIdentity,
    b"omega.optimized-terminal-object-artifact-identity.v1\0"
);
canonical_identity!(
    OptimizedTerminalObjectArtifactManifestIdentity,
    b"omega.optimized-terminal-object-artifact-manifest-identity.v1\0"
);
canonical_identity!(
    OptimizedTerminalOrdinaryCallableEntryIdentity,
    b"omega.optimized-terminal-ordinary-callable-entry-identity.v1\0"
);
canonical_identity!(
    OptimizedTerminalOrdinaryCallableEntryManifestIdentity,
    b"omega.optimized-terminal-ordinary-callable-entry-manifest-identity.v1\0"
);
canonical_identity!(
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    b"omega.optimized-program-storage-semantic-wrapper-object-identity.v1\0"
);
canonical_identity!(
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    b"omega.optimized-program-storage-semantic-wrapper-object-container-identity.v1\0"
);
canonical_identity!(
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    b"omega.optimized-program-storage-semantic-wrapper-object-manifest-identity.v1\0"
);
canonical_identity!(
    OptimizationDecisionIdentity,
    b"omega.optimization-decision-identity.v1\0"
);
canonical_identity!(
    OptimizationDecisionSchemaIdentity,
    b"omega.optimization-decision-schema-identity.v1\0"
);
canonical_identity!(
    OptimizationDecisionTargetIdentity,
    b"omega.optimization-decision-target-identity.v1\0"
);
canonical_identity!(
    OptimizationValidatorIdentity,
    b"omega.optimization-validator-identity.v1\0"
);
canonical_identity!(
    OptimizationUnitIdentity,
    b"omega.optimization-unit-identity.v1\0"
);
canonical_identity!(
    OptimizationRuleSetIdentity,
    b"omega.optimization-rule-set-identity.v1\0"
);
canonical_identity!(
    TargetCostModelIdentity,
    b"omega.target-cost-model-identity.v1\0"
);
canonical_identity!(
    OptimizationDecisionLogIdentity,
    b"omega.optimization-decision-log-identity.v1\0"
);
canonical_identity!(
    OptimizationWorkloadProfileIdentity,
    b"omega.optimization-workload-profile-identity.v1\0"
);
canonical_identity!(
    TransformationLedgerIdentity,
    b"omega.transformation-ledger-identity.v1\0"
);
canonical_identity!(
    OptimizationIdentityBundleIdentity,
    b"omega.optimization-identity-bundle-identity.v1\0"
);
canonical_identity!(
    OptimizedAbstractPlanProjectionIdentity,
    b"omega.optimized-abstract-plan-projection-identity.v2\0"
);

impl OptimizationRuleSetIdentity {
    /// Bind the complete normalized execution order. Order is meaningful and
    /// therefore is not sorted here; callers must supply the pass manager's
    /// canonical order. Duplicate rule identities are never canonical.
    pub fn from_ordered_rules(
        rules: &[OptimizationRuleIdentity],
    ) -> Result<Self, DuplicateOptimizationRuleIdentity> {
        let mut seen = BTreeSet::new();
        let mut canonical = Vec::with_capacity(8 + rules.len() * IDENTITY_WIDTH);
        canonical.extend_from_slice(
            &u64::try_from(rules.len())
                .expect("ordered optimization rule count fits u64")
                .to_le_bytes(),
        );
        for rule in rules {
            if !seen.insert(*rule) {
                return Err(DuplicateOptimizationRuleIdentity(*rule));
            }
            canonical.extend_from_slice(&rule.bytes());
        }
        Ok(Self::from_canonical_bytes(&canonical))
    }
}

/// Complete identities required to replay or cache one optimization result.
///
/// Report rendering policy is intentionally absent. Optional authoritative
/// inputs carry explicit presence tags so absence cannot collide with an
/// all-zero or otherwise valid digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationIdentityBundle {
    selections: OptimizationSelectionIdentity,
    rule_set: OptimizationRuleSetIdentity,
    target_cost_model: TargetCostModelIdentity,
    decision_log: Option<OptimizationDecisionLogIdentity>,
    workload_profile: Option<OptimizationWorkloadProfileIdentity>,
    transformation_ledger: TransformationLedgerIdentity,
}

impl OptimizationIdentityBundle {
    pub const fn new(
        selections: OptimizationSelectionIdentity,
        rule_set: OptimizationRuleSetIdentity,
        target_cost_model: TargetCostModelIdentity,
        decision_log: Option<OptimizationDecisionLogIdentity>,
        workload_profile: Option<OptimizationWorkloadProfileIdentity>,
        transformation_ledger: TransformationLedgerIdentity,
    ) -> Self {
        Self {
            selections,
            rule_set,
            target_cost_model,
            decision_log,
            workload_profile,
            transformation_ledger,
        }
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn rule_set(self) -> OptimizationRuleSetIdentity {
        self.rule_set
    }

    pub const fn target_cost_model(self) -> TargetCostModelIdentity {
        self.target_cost_model
    }

    pub const fn decision_log(self) -> Option<OptimizationDecisionLogIdentity> {
        self.decision_log
    }

    pub const fn workload_profile(self) -> Option<OptimizationWorkloadProfileIdentity> {
        self.workload_profile
    }

    pub const fn transformation_ledger(self) -> TransformationLedgerIdentity {
        self.transformation_ledger
    }

    pub fn encode(self) -> Vec<u8> {
        let optional_width = |value: bool| 1 + usize::from(value) * IDENTITY_WIDTH;
        let mut encoded = Vec::with_capacity(
            12 + IDENTITY_WIDTH * 4
                + optional_width(self.decision_log.is_some())
                + optional_width(self.workload_profile.is_some()),
        );
        encoded.extend_from_slice(BUNDLE_MAGIC);
        encoded.extend_from_slice(&BUNDLE_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.selections.bytes());
        encoded.extend_from_slice(&self.rule_set.bytes());
        encoded.extend_from_slice(&self.target_cost_model.bytes());
        encode_optional(
            &mut encoded,
            self.decision_log.map(|identity| identity.bytes()),
        );
        encode_optional(
            &mut encoded,
            self.workload_profile.map(|identity| identity.bytes()),
        );
        encoded.extend_from_slice(&self.transformation_ledger.bytes());
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, IdentityBundleDecodeError> {
        let mut cursor = BundleCursor::new(encoded);
        if cursor.take(8)? != BUNDLE_MAGIC {
            return Err(IdentityBundleDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != BUNDLE_VERSION {
            return Err(IdentityBundleDecodeError::UnsupportedVersion(version));
        }
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let rule_set = OptimizationRuleSetIdentity::from_bytes(cursor.array()?);
        let target_cost_model = TargetCostModelIdentity::from_bytes(cursor.array()?);
        let decision_log = cursor
            .optional()?
            .map(OptimizationDecisionLogIdentity::from_bytes);
        let workload_profile = cursor
            .optional()?
            .map(OptimizationWorkloadProfileIdentity::from_bytes);
        let transformation_ledger = TransformationLedgerIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(IdentityBundleDecodeError::TrailingBytes);
        }
        Ok(Self::new(
            selections,
            rule_set,
            target_cost_model,
            decision_log,
            workload_profile,
            transformation_ledger,
        ))
    }

    pub fn identity(self) -> OptimizationIdentityBundleIdentity {
        OptimizationIdentityBundleIdentity::from_canonical_bytes(&self.encode())
    }
}

fn encode_optional(encoded: &mut Vec<u8>, identity: Option<[u8; IDENTITY_WIDTH]>) {
    match identity {
        None => encoded.push(0),
        Some(identity) => {
            encoded.push(1);
            encoded.extend_from_slice(&identity);
        }
    }
}

struct BundleCursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> BundleCursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], IdentityBundleDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(IdentityBundleDecodeError::Truncated)?;
        let bytes = self
            .encoded
            .get(self.position..end)
            .ok_or(IdentityBundleDecodeError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], IdentityBundleDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| IdentityBundleDecodeError::Truncated)
    }

    fn optional(&mut self) -> Result<Option<[u8; IDENTITY_WIDTH]>, IdentityBundleDecodeError> {
        match self.array::<1>()?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.array()?)),
            tag => Err(IdentityBundleDecodeError::InvalidOptionalTag(tag)),
        }
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.position
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateOptimizationRuleIdentity(pub OptimizationRuleIdentity);

impl fmt::Display for DuplicateOptimizationRuleIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("normalized optimization rule set contains a duplicate identity")
    }
}

impl std::error::Error for DuplicateOptimizationRuleIdentity {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityDecodeError {
    WrongLength { expected: usize, actual: usize },
}

impl fmt::Display for IdentityDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => {
                write!(formatter, "identity is {actual} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for IdentityDecodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityBundleDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidOptionalTag(u8),
    TrailingBytes,
}

impl fmt::Display for IdentityBundleDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("optimization identity bundle is truncated"),
            Self::WrongMagic => formatter.write_str("optimization identity bundle has wrong magic"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "optimization identity bundle version {version} is unsupported"
            ),
            Self::InvalidOptionalTag(tag) => write!(
                formatter,
                "optimization identity bundle has invalid optional-presence tag {tag}"
            ),
            Self::TrailingBytes => {
                formatter.write_str("optimization identity bundle has trailing bytes")
            }
        }
    }
}

impl std::error::Error for IdentityBundleDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Optimization, OptimizationSelections};

    fn bundle() -> OptimizationIdentityBundle {
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup])
            .expect("unique selection")
            .identity();
        let rules = [
            OptimizationRuleIdentity::from_canonical_bytes(b"cfg-cleanup/v1"),
            OptimizationRuleIdentity::from_canonical_bytes(b"branch-fold/v2"),
        ];
        OptimizationIdentityBundle::new(
            selections,
            OptimizationRuleSetIdentity::from_ordered_rules(&rules).expect("unique rules"),
            TargetCostModelIdentity::from_canonical_bytes(b"target-cost/x86-64/v1"),
            Some(OptimizationDecisionLogIdentity::from_canonical_bytes(
                b"decision-log",
            )),
            None,
            TransformationLedgerIdentity::from_canonical_bytes(b"ledger"),
        )
    }

    #[test]
    fn identity_domains_are_distinct_for_equal_canonical_bytes() {
        assert_ne!(
            OptimizationRuleIdentity::from_canonical_bytes(b"same").bytes(),
            TargetCostModelIdentity::from_canonical_bytes(b"same").bytes()
        );
        assert_ne!(
            OptimizationDecisionLogIdentity::from_canonical_bytes(b"same").bytes(),
            TransformationLedgerIdentity::from_canonical_bytes(b"same").bytes()
        );
        let rule = OptimizationRuleIdentity::from_canonical_bytes(b"same").bytes();
        for identity in [
            OptimizationPassIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationCandidateIdentity::from_canonical_bytes(b"same").bytes(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"same").bytes(),
            PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"same").bytes(),
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"same").bytes(),
            FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"same")
                .bytes(),
            TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"same").bytes(),
            FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"same").bytes(),
            TerminalRelocationFreeObjectPlanIdentity::from_canonical_bytes(b"same").bytes(),
            TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(b"same").bytes(),
            FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedTerminalObjectArtifactIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedTerminalObjectArtifactManifestIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedTerminalOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"same")
                .bytes(),
            OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(b"same")
                .bytes(),
            OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                b"same",
            )
            .bytes(),
            OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
                b"same",
            )
            .bytes(),
            OptimizationDecisionIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationDecisionSchemaIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationDecisionTargetIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationValidatorIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationUnitIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationRuleSetIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationWorkloadProfileIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationIdentityBundleIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(b"same").bytes(),
        ] {
            assert_ne!(rule, identity);
        }
    }

    #[test]
    fn every_fixed_width_identity_round_trips() {
        macro_rules! round_trip {
            ($identity:ty) => {{
                let identity = <$identity>::from_canonical_bytes(stringify!($identity).as_bytes());
                assert_eq!(<$identity>::decode(&identity.encode()), Ok(identity));
            }};
        }
        round_trip!(OptimizationRuleIdentity);
        round_trip!(OptimizationPassIdentity);
        round_trip!(OptimizationCandidateIdentity);
        round_trip!(ScalarConstantFactIdentity);
        round_trip!(AcceptedObligationFactIdentity);
        round_trip!(ProofQuestionIdentity);
        round_trip!(OwnershipFrontierFactIdentity);
        round_trip!(PrePhysicalOptimizationManifestIdentity);
        round_trip!(PostAllocationOptimizationManifestIdentity);
        round_trip!(SelectedLoweringOptimizationCompletionIdentity);
        round_trip!(FunctionRelativeOptimizationRealizationManifestIdentity);
        round_trip!(TerminalFunctionFragmentEmissionIdentity);
        round_trip!(FunctionFragmentEmissionManifestIdentity);
        round_trip!(TerminalRelocationFreeTextSectionIdentity);
        round_trip!(FunctionFragmentTextSectionManifestIdentity);
        round_trip!(TerminalRelocationFreeObjectPlanIdentity);
        round_trip!(TerminalRelocationFreeObjectContainerIdentity);
        round_trip!(FunctionFragmentObjectContainerManifestIdentity);
        round_trip!(OptimizedTerminalObjectArtifactIdentity);
        round_trip!(OptimizedTerminalObjectArtifactManifestIdentity);
        round_trip!(OptimizedTerminalOrdinaryCallableEntryIdentity);
        round_trip!(OptimizedTerminalOrdinaryCallableEntryManifestIdentity);
        round_trip!(OptimizedProgramStorageSemanticWrapperObjectIdentity);
        round_trip!(OptimizedProgramStorageSemanticWrapperObjectContainerIdentity);
        round_trip!(OptimizedProgramStorageSemanticWrapperObjectManifestIdentity);
        round_trip!(OptimizationDecisionIdentity);
        round_trip!(OptimizationDecisionSchemaIdentity);
        round_trip!(OptimizationDecisionTargetIdentity);
        round_trip!(OptimizationValidatorIdentity);
        round_trip!(OptimizationUnitIdentity);
        round_trip!(OptimizationRuleSetIdentity);
        round_trip!(TargetCostModelIdentity);
        round_trip!(OptimizationDecisionLogIdentity);
        round_trip!(OptimizationWorkloadProfileIdentity);
        round_trip!(TransformationLedgerIdentity);
        round_trip!(OptimizationIdentityBundleIdentity);
        round_trip!(OptimizedAbstractPlanProjectionIdentity);
    }

    #[test]
    fn ordered_rule_set_binds_order_and_rejects_duplicates() {
        let first = OptimizationRuleIdentity::from_canonical_bytes(b"first");
        let second = OptimizationRuleIdentity::from_canonical_bytes(b"second");
        assert_ne!(
            OptimizationRuleSetIdentity::from_ordered_rules(&[first, second]).unwrap(),
            OptimizationRuleSetIdentity::from_ordered_rules(&[second, first]).unwrap()
        );
        assert_eq!(
            OptimizationRuleSetIdentity::from_ordered_rules(&[first, first]),
            Err(DuplicateOptimizationRuleIdentity(first))
        );
    }

    #[test]
    fn bundle_round_trip_and_optional_presence_are_canonical() {
        let bundle = bundle();
        let encoded = bundle.encode();
        assert_eq!(OptimizationIdentityBundle::decode(&encoded), Ok(bundle));
        assert_eq!(bundle.identity(), bundle.identity());

        let without_decisions = OptimizationIdentityBundle::new(
            bundle.selections(),
            bundle.rule_set(),
            bundle.target_cost_model(),
            None,
            bundle.workload_profile(),
            bundle.transformation_ledger(),
        );
        assert_ne!(bundle.identity(), without_decisions.identity());
    }

    #[test]
    fn every_bundle_component_changes_the_composite_identity() {
        let baseline = bundle();
        let changed = [
            OptimizationIdentityBundle::new(
                OptimizationSelections::new([Optimization::CopyPropagation])
                    .unwrap()
                    .identity(),
                baseline.rule_set(),
                baseline.target_cost_model(),
                baseline.decision_log(),
                baseline.workload_profile(),
                baseline.transformation_ledger(),
            ),
            OptimizationIdentityBundle::new(
                baseline.selections(),
                OptimizationRuleSetIdentity::from_canonical_bytes(b"rules-2"),
                baseline.target_cost_model(),
                baseline.decision_log(),
                baseline.workload_profile(),
                baseline.transformation_ledger(),
            ),
            OptimizationIdentityBundle::new(
                baseline.selections(),
                baseline.rule_set(),
                TargetCostModelIdentity::from_canonical_bytes(b"cost-2"),
                baseline.decision_log(),
                baseline.workload_profile(),
                baseline.transformation_ledger(),
            ),
            OptimizationIdentityBundle::new(
                baseline.selections(),
                baseline.rule_set(),
                baseline.target_cost_model(),
                Some(OptimizationDecisionLogIdentity::from_canonical_bytes(
                    b"decisions-2",
                )),
                baseline.workload_profile(),
                baseline.transformation_ledger(),
            ),
            OptimizationIdentityBundle::new(
                baseline.selections(),
                baseline.rule_set(),
                baseline.target_cost_model(),
                baseline.decision_log(),
                Some(OptimizationWorkloadProfileIdentity::from_canonical_bytes(
                    b"workload-2",
                )),
                baseline.transformation_ledger(),
            ),
            OptimizationIdentityBundle::new(
                baseline.selections(),
                baseline.rule_set(),
                baseline.target_cost_model(),
                baseline.decision_log(),
                baseline.workload_profile(),
                TransformationLedgerIdentity::from_canonical_bytes(b"ledger-2"),
            ),
        ];
        for candidate in changed {
            assert_ne!(baseline.identity(), candidate.identity());
        }
    }

    #[test]
    fn malformed_identity_and_bundle_encodings_reject() {
        assert_eq!(
            OptimizationRuleIdentity::decode(&[0; 31]),
            Err(IdentityDecodeError::WrongLength {
                expected: 32,
                actual: 31,
            })
        );
        let mut trailing = bundle().encode();
        trailing.push(0);
        assert_eq!(
            OptimizationIdentityBundle::decode(&trailing),
            Err(IdentityBundleDecodeError::TrailingBytes)
        );
        let mut invalid_tag = bundle().encode();
        let decision_tag = 12 + IDENTITY_WIDTH * 3;
        invalid_tag[decision_tag] = 2;
        assert_eq!(
            OptimizationIdentityBundle::decode(&invalid_tag),
            Err(IdentityBundleDecodeError::InvalidOptionalTag(2))
        );
    }
}
