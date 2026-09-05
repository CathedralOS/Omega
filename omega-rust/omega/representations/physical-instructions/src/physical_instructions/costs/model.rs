use selected_instructions::{MachineLatencyKnowledge, MachineSizeKnowledge};
use target::NativeTarget;

/// Closed schema for descriptive target costs. It is identity, not authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetCostModelVersion {
    MachineKnowledgeV1,
}

/// Stable, domain-separated identity of one target-bound cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetCostModelIdentity([u8; 32]);

impl TargetCostModelIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Descriptive size knowledge retained without converting bounds into facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonAuthoritativeMachineSizeCost {
    ExactBytes(u16),
    EncoderResolved {
        minimum_bytes: u16,
        maximum_bytes: Option<u16>,
    },
}

impl NonAuthoritativeMachineSizeCost {
    pub const fn minimum_bytes(self) -> u16 {
        match self {
            Self::ExactBytes(bytes)
            | Self::EncoderResolved {
                minimum_bytes: bytes,
                ..
            } => bytes,
        }
    }

    pub const fn maximum_bytes(self) -> Option<u16> {
        match self {
            Self::ExactBytes(bytes) => Some(bytes),
            Self::EncoderResolved { maximum_bytes, .. } => maximum_bytes,
        }
    }

    pub const fn exact_bytes(self) -> Option<u16> {
        match self {
            Self::ExactBytes(bytes) => Some(bytes),
            Self::EncoderResolved { .. } => None,
        }
    }
}

/// Latency is deliberately unavailable until target-owned stable data exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonAuthoritativeLatencyCost {
    Unavailable,
}

/// One deterministic observation tied to the exact model that described it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeMachineCost {
    model: TargetCostModelIdentity,
    size: NonAuthoritativeMachineSizeCost,
    latency: NonAuthoritativeLatencyCost,
}

impl NonAuthoritativeMachineCost {
    pub const fn model(self) -> TargetCostModelIdentity {
        self.model
    }

    pub const fn size(self) -> NonAuthoritativeMachineSizeCost {
        self.size
    }

    pub const fn latency(self) -> NonAuthoritativeLatencyCost {
        self.latency
    }
}

/// Exact target binding for a descriptive cost vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetCostModel {
    target: NativeTarget,
    version: TargetCostModelVersion,
    identity: TargetCostModelIdentity,
}

impl TargetCostModel {
    pub(crate) const fn new(
        target: NativeTarget,
        version: TargetCostModelVersion,
        identity: TargetCostModelIdentity,
    ) -> Self {
        Self {
            target,
            version,
            identity,
        }
    }

    pub const fn target(self) -> NativeTarget {
        self.target
    }

    pub const fn version(self) -> TargetCostModelVersion {
        self.version
    }

    pub const fn identity(self) -> TargetCostModelIdentity {
        self.identity
    }

    /// Preserve machine knowledge as descriptive input. This function does
    /// not rank alternatives or authorize an optimization.
    pub const fn observe(
        self,
        size: MachineSizeKnowledge,
        latency: MachineLatencyKnowledge,
    ) -> NonAuthoritativeMachineCost {
        let size = match size {
            MachineSizeKnowledge::ExactBytes(bytes) => {
                NonAuthoritativeMachineSizeCost::ExactBytes(bytes)
            }
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes,
            } => NonAuthoritativeMachineSizeCost::EncoderResolved {
                minimum_bytes,
                maximum_bytes,
            },
        };
        let latency = match latency {
            MachineLatencyKnowledge::StableBaselineUnavailable => {
                NonAuthoritativeLatencyCost::Unavailable
            }
        };
        NonAuthoritativeMachineCost {
            model: self.identity,
            size,
            latency,
        }
    }
}
