//! Provider-neutral storage retained at checked suspending calls.
//!
//! Structural frontiers keep the ordinary operand's location and access
//! together. Pointer-equivalent owned and borrowed operands must not share a
//! frontier commitment. This records existing authority; loan ancestry,
//! backing lifetime and runtime preservation still require their own joins.

use crate::{StructuralAccess, StructuralArgument, StructuralPathSegment};
use language_semantics::CarryPolicy;
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, IeeeFloatFormat, IntegerCarrier, IntegerSign, MachineId,
    OperationId, ScalarType, StructuralTypeId, SuspensionCrossingId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalSuspensionCallSite {
    pub operation: OperationId,
    pub crossing: SuspensionCrossingId,
    pub target: TerminalSuspensionCallTarget,
    pub frontier_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalSuspensionCallPlan {
    pub operation: OperationId,
    pub crossing: SuspensionCrossingId,
    pub target: TerminalSuspensionCallTarget,
    pub effective: CarryPolicy,
    /// Independent checked-frontier cardinality commitment. The verifier
    /// compares this with the encoded roster before inspecting any member.
    pub live_value_count: u32,
    /// Ordered demand occurrences, not a set of physical storage coordinates.
    /// Distinct actual positions may retain the same storage; each occurrence
    /// keeps its claims and policy and contributes to the committed count.
    pub live_values: Vec<TerminalSuspensionLiveValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionCallTarget {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalSuspensionLiveValue {
    pub place: TerminalSuspensionPlace,
    pub value_type: TerminalSuspensionValueType,
    pub storage: TerminalSuspensionStorage,
    /// Complete live Terminal claims attached to this exact place.
    pub claim_count: u32,
    pub claims: Vec<ClaimId>,
    pub effective: CarryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionPlace {
    Scalar(ValueId),
    /// Retain the ordinary storage operand, including its access. A physical
    /// pointer and path do not distinguish ownership from a loan; this row
    /// records the existing authority without establishing backing or pinning.
    Structural(StructuralArgument),
}

impl TerminalSuspensionPlace {
    /// Scalar identities are retained uses even when they occur only in a
    /// projected storage coordinate rather than as a carried scalar value.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) {
        match self {
            Self::Scalar(value) => visit(*value),
            Self::Structural(argument) => {
                for segment in &argument.path {
                    if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                        visit(*index);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionValueType {
    Scalar(ScalarType),
    Structural(StructuralTypeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionStorage {
    Parameter,
    Local,
    CallArgument,
}

pub fn suspension_frontier_commitment(plan: &TerminalSuspensionCallPlan) -> [u8; 32] {
    let mut hash =
        crate::digest_encoder::DomainDigestEncoder::new(b"terminal-suspension-frontier-v4\0");
    hash.raw(&plan.operation.get().to_le_bytes());
    hash.raw(&plan.crossing.get().to_le_bytes());
    match plan.target {
        TerminalSuspensionCallTarget::Machine(machine) => {
            hash.byte(1);
            hash.raw(&machine.get().to_le_bytes());
        }
        TerminalSuspensionCallTarget::Boundary(boundary) => {
            hash.byte(2);
            hash.raw(&boundary.get().to_le_bytes());
        }
    }
    hash_carry_policy(&mut hash, plan.effective);
    hash.raw(&plan.live_value_count.to_le_bytes());
    hash.u64(plan.live_values.len() as u64);
    for live in &plan.live_values {
        match &live.place {
            TerminalSuspensionPlace::Scalar(value) => {
                hash.byte(1);
                hash.raw(&value.get().to_le_bytes());
            }
            TerminalSuspensionPlace::Structural(argument) => {
                hash.byte(2);
                hash.raw(&argument.place.get().to_le_bytes());
                hash.byte(match argument.access {
                    StructuralAccess::Owned => 1,
                    StructuralAccess::SharedBorrow => 2,
                    StructuralAccess::MutableBorrow => 3,
                    StructuralAccess::WriteOnlyBorrow => 4,
                });
                hash.u64(argument.path.len() as u64);
                for segment in &argument.path {
                    match segment {
                        StructuralPathSegment::Case(case) => {
                            hash.byte(7);
                            hash.raw(&case.get().to_le_bytes());
                        }
                        StructuralPathSegment::Field(field) => {
                            hash.byte(1);
                            hash.u64(field.len() as u64);
                            hash.raw(field.as_bytes());
                        }
                        StructuralPathSegment::FixedIndex(index) => {
                            hash.byte(2);
                            hash.raw(&index.to_le_bytes());
                        }
                        StructuralPathSegment::Referent => hash.byte(3),
                        StructuralPathSegment::FixedByteRange { start, end } => {
                            hash.byte(4);
                            hash.raw(&start.to_le_bytes());
                            hash.raw(&end.to_le_bytes());
                        }
                        StructuralPathSegment::RuntimeIndex { index, obligation } => {
                            hash.byte(5);
                            hash.raw(&index.get().to_le_bytes());
                            hash.raw(&obligation.get().to_le_bytes());
                        }
                    }
                }
            }
        }
        match live.value_type {
            TerminalSuspensionValueType::Scalar(scalar) => {
                hash.byte(1);
                hash_scalar_type(&mut hash, scalar);
            }
            TerminalSuspensionValueType::Structural(structural) => {
                hash.byte(2);
                hash.raw(&structural.get().to_le_bytes());
            }
        }
        hash.byte(match live.storage {
            TerminalSuspensionStorage::Parameter => 2,
            TerminalSuspensionStorage::Local => 3,
            TerminalSuspensionStorage::CallArgument => 4,
        });
        hash.raw(&live.claim_count.to_le_bytes());
        hash.u64(live.claims.len() as u64);
        for claim in &live.claims {
            hash.raw(&claim.get().to_le_bytes());
        }
        hash_carry_policy(&mut hash, live.effective);
    }
    hash.finish()
}

fn hash_scalar_type(hash: &mut crate::digest_encoder::DomainDigestEncoder, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => hash.byte(1),
        ScalarType::Integer(integer) => {
            hash.byte(2);
            hash.byte(match integer.carrier() {
                IntegerCarrier::Fixed => 1,
                IntegerCarrier::Address => 2,
            });
            hash.byte(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            hash.raw(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            hash.byte(3);
            hash.byte(match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
        }
    }
}

fn hash_carry_policy(hash: &mut crate::digest_encoder::DomainDigestEncoder, policy: CarryPolicy) {
    hash.byte(u8::from(
        policy.suspension == language_semantics::CarrySuspension::Allowed,
    ));
    hash.byte(u8::from(policy.cpu == language_semantics::CarryCpu::Any));
    hash.byte(u8::from(
        policy.host_thread == language_semantics::CarryHostThread::Any,
    ));
    hash.byte(u8::from(
        policy.address == language_semantics::CarryAddress::Movable,
    ));
}
