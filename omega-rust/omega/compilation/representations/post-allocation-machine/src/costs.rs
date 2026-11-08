/*
We define the non-authoritative descriptive target cost model (`TargetCostModel`)
and its domain-separated identity (`omega.target-cost-model.identity.v1`).

Post-allocation optimization stages observe instruction size knowledge (`ExactBytes`
vs. `EncoderResolved { minimum_bytes, maximum_bytes }`) tied to a specific
`NativeTarget` without promoting encoder-resolved bounds into exact byte facts
or using cost observations for semantic admission.
*/

use selected_instructions::{MachineLatencyKnowledge, MachineSizeKnowledge};
use target::NativeTarget;

const TARGET_COST_MODEL_IDENTITY_V1: &[u8] = b"omega.target-cost-model.identity.v1";

pub fn target_cost_model(target: NativeTarget) -> TargetCostModel {
    let version = TargetCostModelVersion::MachineKnowledgeV1;
    TargetCostModel::new(target, version, target_cost_model_identity(target, version))
}

fn target_cost_model_identity(
    target: NativeTarget,
    version: TargetCostModelVersion,
) -> TargetCostModelIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(TARGET_COST_MODEL_IDENTITY_V1);
    digest.byte(match version {
        TargetCostModelVersion::MachineKnowledgeV1 => 1,
    });
    digest.raw(&target.wire_identity_bytes());
    TargetCostModelIdentity::from_bytes(digest.finish())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetCostModelVersion {
    MachineKnowledgeV1,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonAuthoritativeLatencyCost {
    Unavailable,
}

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

#[cfg(test)]
mod tests {
    use selected_instructions::{MachineLatencyKnowledge, MachineSizeKnowledge};
    use target::{Architecture, NativeTarget, ObjectFormat};

    use super::{
        NonAuthoritativeLatencyCost, NonAuthoritativeMachineSizeCost, TargetCostModelVersion,
        target_cost_model,
    };

    #[test]
    fn identity_is_deterministic_and_covers_the_complete_native_target() {
        let linux_x64 = target_cost_model(NativeTarget::linux_x64());
        assert_eq!(linux_x64, target_cost_model(NativeTarget::linux_x64()));
        assert_eq!(
            linux_x64.version(),
            TargetCostModelVersion::MachineKnowledgeV1
        );
        assert_eq!(linux_x64.target(), NativeTarget::linux_x64());
        assert_ne!(linux_x64.identity().bytes(), [0; 32]);

        for distinct in [
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
            NativeTarget {
                architecture: Architecture::X86_64,
                object_format: ObjectFormat::Elf,
                pointer_size: 4,
                pointer_alignment: 4,
            },
        ] {
            assert_ne!(
                linux_x64.identity(),
                target_cost_model(distinct).identity(),
                "target distinction {distinct:?} must remain in model identity",
            );
        }
    }

    #[test]
    fn exact_size_remains_exact_and_latency_remains_unavailable() {
        let model = target_cost_model(NativeTarget::linux_x64());
        let observation = model.observe(
            MachineSizeKnowledge::ExactBytes(3),
            MachineLatencyKnowledge::StableBaselineUnavailable,
        );

        assert_eq!(observation.model(), model.identity());
        assert_eq!(
            observation.size(),
            NonAuthoritativeMachineSizeCost::ExactBytes(3)
        );
        assert_eq!(observation.size().minimum_bytes(), 3);
        assert_eq!(observation.size().maximum_bytes(), Some(3));
        assert_eq!(observation.size().exact_bytes(), Some(3));
        assert_eq!(
            observation.latency(),
            NonAuthoritativeLatencyCost::Unavailable
        );
    }

    #[test]
    fn encoder_resolved_bounds_are_not_promoted_to_exact_costs() {
        let model = target_cost_model(NativeTarget::linux_arm64());
        for (maximum_bytes, expected_maximum) in [(Some(16), Some(16)), (None, None)] {
            let observation = model.observe(
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: 4,
                    maximum_bytes,
                },
                MachineLatencyKnowledge::StableBaselineUnavailable,
            );

            assert_eq!(
                observation.size(),
                NonAuthoritativeMachineSizeCost::EncoderResolved {
                    minimum_bytes: 4,
                    maximum_bytes,
                }
            );
            assert_eq!(observation.size().minimum_bytes(), 4);
            assert_eq!(observation.size().maximum_bytes(), expected_maximum);
            assert_eq!(observation.size().exact_bytes(), None);
        }
    }
}
