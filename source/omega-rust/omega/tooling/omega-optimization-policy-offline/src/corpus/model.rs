use omega_optimization_core::{
    OptimizationDecisionLogIdentity, OptimizationDecisionSchemaIdentity, OptimizationUnitIdentity,
};
use omega_optimization_policy::{
    ExternalDecisionAction, ExternalDecisionContext, ExternalDecisionPoint,
    ExternalDecisionSchemaError,
};

use super::identity::{DecisionSurfaceIdentity, OfflinePolicyCorpusIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OfflinePolicySplit {
    Training,
    Evaluation,
    Regression,
}

impl OfflinePolicySplit {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Training => 1,
            Self::Evaluation => 2,
            Self::Regression => 3,
        }
    }

    pub(super) const fn from_tag(tag: u8) -> Result<Self, OfflinePolicyCorpusError> {
        match tag {
            1 => Ok(Self::Training),
            2 => Ok(Self::Evaluation),
            3 => Ok(Self::Regression),
            _ => Err(OfflinePolicyCorpusError::UnknownSplit(tag)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePolicyDecisionExample {
    pub(super) surface: DecisionSurfaceIdentity,
    pub(super) log: OptimizationDecisionLogIdentity,
    pub(super) point_ordinal: u32,
    pub(super) source: OptimizationUnitIdentity,
    pub(super) split: OfflinePolicySplit,
    pub(super) context: ExternalDecisionContext,
    pub(super) point: ExternalDecisionPoint,
}

impl OfflinePolicyDecisionExample {
    pub const fn surface(&self) -> DecisionSurfaceIdentity {
        self.surface
    }

    pub const fn log(&self) -> OptimizationDecisionLogIdentity {
        self.log
    }

    pub const fn point_ordinal(&self) -> u32 {
        self.point_ordinal
    }

    pub const fn source(&self) -> OptimizationUnitIdentity {
        self.source
    }

    pub const fn split(&self) -> OfflinePolicySplit {
        self.split
    }

    pub const fn context(&self) -> ExternalDecisionContext {
        self.context
    }

    pub const fn point(&self) -> &ExternalDecisionPoint {
        &self.point
    }

    pub const fn recorded_action(&self) -> ExternalDecisionAction {
        self.point.action()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflinePolicyCorpusReceipt {
    pub(super) identity: OfflinePolicyCorpusIdentity,
    pub(super) schema: OptimizationDecisionSchemaIdentity,
    pub(super) log_count: u32,
    pub(super) source_count: u32,
    pub(super) decision_count: u32,
    pub(super) split_counts: [u32; 3],
}

impl OfflinePolicyCorpusReceipt {
    pub const fn identity(self) -> OfflinePolicyCorpusIdentity {
        self.identity
    }

    pub const fn schema(self) -> OptimizationDecisionSchemaIdentity {
        self.schema
    }

    pub const fn log_count(self) -> u32 {
        self.log_count
    }

    pub const fn source_count(self) -> u32 {
        self.source_count
    }

    pub const fn decision_count(self) -> u32 {
        self.decision_count
    }

    pub const fn decisions_in(self, split: OfflinePolicySplit) -> u32 {
        self.split_counts[(split.tag() - 1) as usize]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOfflinePolicyCorpus {
    pub(super) candidate: CorpusCandidate,
    pub(super) examples: Vec<OfflinePolicyDecisionExample>,
    pub(super) receipt: OfflinePolicyCorpusReceipt,
}

impl ValidatedOfflinePolicyCorpus {
    pub const fn identity(&self) -> OfflinePolicyCorpusIdentity {
        self.receipt.identity
    }

    pub const fn receipt(&self) -> OfflinePolicyCorpusReceipt {
        self.receipt
    }

    pub fn examples(&self) -> &[OfflinePolicyDecisionExample] {
        &self.examples
    }

    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(&self.candidate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CapturedLog {
    pub(super) split: OfflinePolicySplit,
    pub(super) encoded: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CorpusCandidate {
    pub(super) claimed_identity: OfflinePolicyCorpusIdentity,
    pub(super) logs: Vec<CapturedLog>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflinePolicyCorpusError {
    ExternalSchema(ExternalDecisionSchemaError),
    EmptyCorpus,
    EmptyDecisionLog,
    WrongExternalSchema,
    NonCanonicalExternalLog,
    DuplicateLog,
    NonCanonicalLogs,
    DuplicateDecisionSurface,
    SourceSplitMismatch,
    SourceSplitLeakage,
    CountOverflow,
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownSplit(u8),
    CorpusIdentityMismatch,
    TrailingBytes,
}

impl From<ExternalDecisionSchemaError> for OfflinePolicyCorpusError {
    fn from(error: ExternalDecisionSchemaError) -> Self {
        Self::ExternalSchema(error)
    }
}

impl std::fmt::Display for OfflinePolicyCorpusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid offline optimization-policy corpus: {self:?}"
        )
    }
}

impl std::error::Error for OfflinePolicyCorpusError {}
