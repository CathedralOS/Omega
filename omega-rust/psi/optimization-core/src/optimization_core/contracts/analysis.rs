use super::CoreContractDecodeError;

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
