use crate::{OptimizationPassIdentity, OptimizationRuleIdentity};
use std::fmt;

const RULE_CONTRACT_MAGIC: &[u8; 8] = b"OMGRUL\0\0";
const RULE_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// Deterministic analysis products understood by the pass manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AnalysisKind {
    ControlFlowGraph = 1,
    Dominators = 2,
    PostDominators = 3,
    LoopForest = 4,
    StronglyConnectedComponents = 5,
    UseDefinition = 6,
    ExecutableEdges = 7,
    ScalarConstants = 8,
    ValueRanges = 9,
    EffectSummaries = 10,
    CallGraph = 11,
    PlaceAliases = 12,
    MemoryVersions = 13,
    OwnershipFrontiers = 14,
    CleanupFrontiers = 15,
    EscapeAndAddressStability = 16,
    ValueLiveness = 17,
    PlaceLiveness = 18,
    TargetCosts = 19,
    RegisterLiveness = 20,
}

impl AnalysisKind {
    pub const ALL: [Self; 20] = [
        Self::ControlFlowGraph,
        Self::Dominators,
        Self::PostDominators,
        Self::LoopForest,
        Self::StronglyConnectedComponents,
        Self::UseDefinition,
        Self::ExecutableEdges,
        Self::ScalarConstants,
        Self::ValueRanges,
        Self::EffectSummaries,
        Self::CallGraph,
        Self::PlaceAliases,
        Self::MemoryVersions,
        Self::OwnershipFrontiers,
        Self::CleanupFrontiers,
        Self::EscapeAndAddressStability,
        Self::ValueLiveness,
        Self::PlaceLiveness,
        Self::TargetCosts,
        Self::RegisterLiveness,
    ];

    const fn bit(self) -> u64 {
        1_u64 << (self as u8 - 1)
    }
}

const KNOWN_ANALYSIS_BITS: u64 = {
    let mut bits = 0;
    let mut index = 0;
    while index < AnalysisKind::ALL.len() {
        bits |= AnalysisKind::ALL[index].bit();
        index += 1;
    }
    bits
};

/// Canonical set of analysis requirements. Iteration always follows the
/// closed `AnalysisKind::ALL` order, never insertion or hash-map order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AnalysisSet(u64);

impl AnalysisSet {
    pub fn new(analyses: impl IntoIterator<Item = AnalysisKind>) -> Self {
        let mut bits = 0;
        for analysis in analyses {
            bits |= analysis.bit();
        }
        Self(bits)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, analysis: AnalysisKind) -> bool {
        self.0 & analysis.bit() != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn iter(self) -> impl Iterator<Item = AnalysisKind> {
        AnalysisKind::ALL
            .into_iter()
            .filter(move |analysis| self.contains(*analysis))
    }

    pub const fn encode(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let bits = u64::from_le_bytes(encoded.try_into().map_err(|_| {
            CoreContractDecodeError::WrongLength {
                expected: 8,
                actual: encoded.len(),
            }
        })?);
        if bits & !KNOWN_ANALYSIS_BITS != 0 {
            return Err(CoreContractDecodeError::UnknownAnalysisBits(
                bits & !KNOWN_ANALYSIS_BITS,
            ));
        }
        Ok(Self(bits))
    }
}

/// Analyses invalidated atomically by one accepted rewrite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AnalysisInvalidationSet(AnalysisSet);

impl AnalysisInvalidationSet {
    pub fn new(analyses: impl IntoIterator<Item = AnalysisKind>) -> Self {
        Self(AnalysisSet::new(analyses))
    }

    pub const fn contains(self, analysis: AnalysisKind) -> bool {
        self.0.contains(analysis)
    }

    pub fn iter(self) -> impl Iterator<Item = AnalysisKind> {
        self.0.iter()
    }

    pub const fn encode(self) -> [u8; 8] {
        self.0.encode()
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        AnalysisSet::decode(encoded).map(Self)
    }
}

/// Independent validation strength required before a candidate may commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OptimizationSafetyClass {
    StructuralIdentity = 1,
    ExactOperationSemantics = 2,
    ProofCertified = 3,
    OwnershipCertified = 4,
    TranslationValidated = 5,
}

impl OptimizationSafetyClass {
    pub const fn encode(self) -> [u8; 1] {
        [self as u8]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let [tag] = encoded else {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 1,
                actual: encoded.len(),
            });
        };
        match tag {
            1 => Ok(Self::StructuralIdentity),
            2 => Ok(Self::ExactOperationSemantics),
            3 => Ok(Self::ProofCertified),
            4 => Ok(Self::OwnershipCertified),
            5 => Ok(Self::TranslationValidated),
            tag => Err(CoreContractDecodeError::UnknownSafetyClass(*tag)),
        }
    }
}

/// Stable, machine-readable explanation for a deterministic pass decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OptimizationReasonCode {
    Inapplicable = 1,
    NotProfitable = 2,
    MissingAnalysis = 3,
    UnsupportedVocabulary = 4,
    ValidationFailed = 5,
    ProofIncomplete = 6,
    OwnershipEvidenceIncomplete = 7,
    WorkBudgetExhausted = 8,
    CandidateLimitReached = 9,
    ConvergenceLimitReached = 10,
    PolicyRejected = 11,
    Superseded = 12,
}

impl OptimizationReasonCode {
    pub const ALL: [Self; 12] = [
        Self::Inapplicable,
        Self::NotProfitable,
        Self::MissingAnalysis,
        Self::UnsupportedVocabulary,
        Self::ValidationFailed,
        Self::ProofIncomplete,
        Self::OwnershipEvidenceIncomplete,
        Self::WorkBudgetExhausted,
        Self::CandidateLimitReached,
        Self::ConvergenceLimitReached,
        Self::PolicyRejected,
        Self::Superseded,
    ];

    const fn from_tag(tag: u8) -> Result<Self, CoreContractDecodeError> {
        match tag {
            1 => Ok(Self::Inapplicable),
            2 => Ok(Self::NotProfitable),
            3 => Ok(Self::MissingAnalysis),
            4 => Ok(Self::UnsupportedVocabulary),
            5 => Ok(Self::ValidationFailed),
            6 => Ok(Self::ProofIncomplete),
            7 => Ok(Self::OwnershipEvidenceIncomplete),
            8 => Ok(Self::WorkBudgetExhausted),
            9 => Ok(Self::CandidateLimitReached),
            10 => Ok(Self::ConvergenceLimitReached),
            11 => Ok(Self::PolicyRejected),
            12 => Ok(Self::Superseded),
            tag => Err(CoreContractDecodeError::UnknownReasonCode(tag)),
        }
    }
}

/// Final independent disposition of one proposed rewrite candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationCandidateVerdict {
    Applied,
    Skipped(OptimizationReasonCode),
    Rejected(OptimizationReasonCode),
}

impl OptimizationCandidateVerdict {
    pub const fn encode(self) -> [u8; 2] {
        match self {
            Self::Applied => [1, 0],
            Self::Skipped(reason) => [2, reason as u8],
            Self::Rejected(reason) => [3, reason as u8],
        }
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let [disposition, reason] = encoded else {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 2,
                actual: encoded.len(),
            });
        };
        match (*disposition, *reason) {
            (1, 0) => Ok(Self::Applied),
            (1, reason) => Err(CoreContractDecodeError::UnexpectedReason(reason)),
            (2, reason) => Ok(Self::Skipped(OptimizationReasonCode::from_tag(reason)?)),
            (3, reason) => Ok(Self::Rejected(OptimizationReasonCode::from_tag(reason)?)),
            (disposition, _) => Err(CoreContractDecodeError::UnknownVerdict(disposition)),
        }
    }
}

/// Hard ceilings for one named pass group. All axes are explicit and nonzero;
/// exhaustion is a deterministic pass failure, never permission to publish a
/// partial candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationWorkBudget {
    pub(crate) rule_evaluations: u64,
    pub(crate) candidates: u64,
    pub(crate) validation_steps: u64,
    pub(crate) commits: u64,
    pub(crate) iterations: u64,
}

impl OptimizationWorkBudget {
    pub fn new(
        rule_evaluations: u64,
        candidates: u64,
        validation_steps: u64,
        commits: u64,
        iterations: u64,
    ) -> Result<Self, InvalidOptimizationWorkBudget> {
        let budget = Self {
            rule_evaluations,
            candidates,
            validation_steps,
            commits,
            iterations,
        };
        if [
            budget.rule_evaluations,
            budget.candidates,
            budget.validation_steps,
            budget.commits,
            budget.iterations,
        ]
        .contains(&0)
        {
            return Err(InvalidOptimizationWorkBudget);
        }
        Ok(budget)
    }

    pub fn encode(self) -> [u8; 40] {
        let mut encoded = [0; 40];
        for (index, value) in [
            self.rule_evaluations,
            self.candidates,
            self.validation_steps,
            self.commits,
            self.iterations,
        ]
        .into_iter()
        .enumerate()
        {
            encoded[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }
        encoded
    }

    pub const fn rule_evaluations(self) -> u64 {
        self.rule_evaluations
    }

    pub const fn candidates(self) -> u64 {
        self.candidates
    }

    pub const fn validation_steps(self) -> u64 {
        self.validation_steps
    }

    pub const fn commits(self) -> u64 {
        self.commits
    }

    pub const fn iterations(self) -> u64 {
        self.iterations
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        if encoded.len() != 40 {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 40,
                actual: encoded.len(),
            });
        }
        let value = |index: usize| {
            u64::from_le_bytes(
                encoded[index * 8..index * 8 + 8]
                    .try_into()
                    .expect("checked work-budget width"),
            )
        };
        Self::new(value(0), value(1), value(2), value(3), value(4))
            .map_err(|_| CoreContractDecodeError::ZeroWorkBudget)
    }
}

/// Stable declaration consumed by an ordered registry and pass manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationRuleContract {
    identity: OptimizationRuleIdentity,
    pass: OptimizationPassIdentity,
    version: u32,
    required_analyses: AnalysisSet,
    invalidated_analyses: AnalysisInvalidationSet,
    safety_class: OptimizationSafetyClass,
}

impl OptimizationRuleContract {
    pub fn new(
        identity: OptimizationRuleIdentity,
        pass: OptimizationPassIdentity,
        version: u32,
        required_analyses: AnalysisSet,
        invalidated_analyses: AnalysisInvalidationSet,
        safety_class: OptimizationSafetyClass,
    ) -> Result<Self, InvalidOptimizationRuleContract> {
        if version == 0 {
            return Err(InvalidOptimizationRuleContract::ZeroVersion);
        }
        Ok(Self {
            identity,
            pass,
            version,
            required_analyses,
            invalidated_analyses,
            safety_class,
        })
    }

    pub fn encode(self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(97);
        encoded.extend_from_slice(RULE_CONTRACT_MAGIC);
        encoded.extend_from_slice(&RULE_CONTRACT_SCHEMA_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&self.pass.bytes());
        encoded.extend_from_slice(&self.version.to_le_bytes());
        encoded.extend_from_slice(&self.required_analyses.encode());
        encoded.extend_from_slice(&self.invalidated_analyses.encode());
        encoded.extend_from_slice(&self.safety_class.encode());
        encoded
    }

    pub const fn identity(self) -> OptimizationRuleIdentity {
        self.identity
    }

    pub const fn pass(self) -> OptimizationPassIdentity {
        self.pass
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub const fn required_analyses(self) -> AnalysisSet {
        self.required_analyses
    }

    pub const fn invalidated_analyses(self) -> AnalysisInvalidationSet {
        self.invalidated_analyses
    }

    pub const fn safety_class(self) -> OptimizationSafetyClass {
        self.safety_class
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        if encoded.len() != 97 {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 97,
                actual: encoded.len(),
            });
        }
        if &encoded[..8] != RULE_CONTRACT_MAGIC {
            return Err(CoreContractDecodeError::WrongMagic);
        }
        let schema = u32::from_le_bytes(encoded[8..12].try_into().expect("fixed schema width"));
        if schema != RULE_CONTRACT_SCHEMA_VERSION {
            return Err(CoreContractDecodeError::UnsupportedVersion(schema));
        }
        let identity = OptimizationRuleIdentity::from_bytes(
            encoded[12..44]
                .try_into()
                .expect("fixed rule identity width"),
        );
        let pass = OptimizationPassIdentity::from_bytes(
            encoded[44..76]
                .try_into()
                .expect("fixed pass identity width"),
        );
        let version = u32::from_le_bytes(encoded[76..80].try_into().expect("fixed version width"));
        let required_analyses = AnalysisSet::decode(&encoded[80..88])?;
        let invalidated_analyses = AnalysisInvalidationSet::decode(&encoded[88..96])?;
        let safety_class = OptimizationSafetyClass::decode(&encoded[96..97])?;
        Self::new(
            identity,
            pass,
            version,
            required_analyses,
            invalidated_analyses,
            safety_class,
        )
        .map_err(|error| match error {
            InvalidOptimizationRuleContract::ZeroVersion => {
                CoreContractDecodeError::ZeroRuleVersion
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidOptimizationRuleContract {
    ZeroVersion,
}

impl fmt::Display for InvalidOptimizationRuleContract {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("optimization rule version must be nonzero")
    }
}

impl std::error::Error for InvalidOptimizationRuleContract {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidOptimizationWorkBudget;

impl fmt::Display for InvalidOptimizationWorkBudget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("every optimization work-budget axis must be nonzero")
    }
}

impl std::error::Error for InvalidOptimizationWorkBudget {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreContractDecodeError {
    WrongLength { expected: usize, actual: usize },
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownAnalysisBits(u64),
    UnknownSafetyClass(u8),
    UnknownReasonCode(u8),
    UnknownVerdict(u8),
    UnexpectedReason(u8),
    ZeroWorkBudget,
    ZeroRuleVersion,
}

impl fmt::Display for CoreContractDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid optimization core contract encoding: {self:?}"
        )
    }
}

impl std::error::Error for CoreContractDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_sets_are_ordered_and_reject_unknown_bits() {
        let set = AnalysisSet::new([
            AnalysisKind::ValueLiveness,
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
        ]);
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            vec![
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::ValueLiveness,
            ]
        );
        assert_eq!(AnalysisSet::decode(&set.encode()), Ok(set));
        assert!(matches!(
            AnalysisSet::decode(&(1_u64 << 63).to_le_bytes()),
            Err(CoreContractDecodeError::UnknownAnalysisBits(_))
        ));
    }

    #[test]
    fn safety_verdict_and_budget_encodings_are_total() {
        for safety in [
            OptimizationSafetyClass::StructuralIdentity,
            OptimizationSafetyClass::ExactOperationSemantics,
            OptimizationSafetyClass::ProofCertified,
            OptimizationSafetyClass::OwnershipCertified,
            OptimizationSafetyClass::TranslationValidated,
        ] {
            assert_eq!(
                OptimizationSafetyClass::decode(&safety.encode()),
                Ok(safety)
            );
        }
        for verdict in [
            OptimizationCandidateVerdict::Applied,
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable),
            OptimizationCandidateVerdict::Rejected(OptimizationReasonCode::ValidationFailed),
        ] {
            assert_eq!(
                OptimizationCandidateVerdict::decode(&verdict.encode()),
                Ok(verdict)
            );
        }
        for reason in OptimizationReasonCode::ALL {
            let verdict = OptimizationCandidateVerdict::Rejected(reason);
            assert_eq!(
                OptimizationCandidateVerdict::decode(&verdict.encode()),
                Ok(verdict)
            );
        }
        assert_eq!(
            OptimizationCandidateVerdict::decode(&[1, 1]),
            Err(CoreContractDecodeError::UnexpectedReason(1))
        );

        let budget = OptimizationWorkBudget::new(10, 20, 30, 4, 5).unwrap();
        assert_eq!(OptimizationWorkBudget::decode(&budget.encode()), Ok(budget));
        assert_eq!(
            OptimizationWorkBudget::new(0, 1, 1, 1, 1),
            Err(InvalidOptimizationWorkBudget)
        );
    }

    #[test]
    fn rule_contract_round_trip_binds_every_axis() {
        let contract = OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"cfg/fold-branch/v1"),
            OptimizationPassIdentity::from_canonical_bytes(b"cfg-cleanup/v1"),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::ScalarConstants,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .unwrap();
        let encoded = contract.encode();
        assert_eq!(encoded.len(), 97);
        assert_eq!(OptimizationRuleContract::decode(&encoded), Ok(contract));

        let mut unknown_analysis = encoded;
        unknown_analysis[87] |= 0x80;
        assert!(matches!(
            OptimizationRuleContract::decode(&unknown_analysis),
            Err(CoreContractDecodeError::UnknownAnalysisBits(_))
        ));
    }
}
