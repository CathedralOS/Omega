use crate::StructuralPathSegment;
use language_semantics::CarryPolicy;
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, IeeeFloatFormat, IntegerCarrier, IntegerSign, MachineId,
    OperationId, PlaceId, ScalarType, StructuralTypeId, SuspensionCrossingId, ValueId,
};
use sha2::{Digest, Sha256};

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
    pub live_values: Vec<TerminalSuspensionLiveValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionCallTarget {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
    DynamicDescriptor {
        ordinal: u32,
    },
    DynamicParameter {
        parameter_ordinal: u32,
        requirement_slot: u32,
    },
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
    Structural {
        place: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionValueType {
    Scalar(ScalarType),
    Structural(StructuralTypeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSuspensionStorage {
    Persistent,
    Parameter,
    Local,
    CallArgument,
}

pub fn suspension_frontier_commitment(plan: &TerminalSuspensionCallPlan) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"terminal-suspension-frontier-v1\0");
    hash.update(plan.operation.get().to_le_bytes());
    hash.update(plan.crossing.get().to_le_bytes());
    match plan.target {
        TerminalSuspensionCallTarget::Machine(machine) => {
            hash.update([1]);
            hash.update(machine.get().to_le_bytes());
        }
        TerminalSuspensionCallTarget::Boundary(boundary) => {
            hash.update([2]);
            hash.update(boundary.get().to_le_bytes());
        }
        TerminalSuspensionCallTarget::DynamicDescriptor { ordinal } => {
            hash.update([3]);
            hash.update(ordinal.to_le_bytes());
        }
        TerminalSuspensionCallTarget::DynamicParameter {
            parameter_ordinal,
            requirement_slot,
        } => {
            hash.update([4]);
            hash.update(parameter_ordinal.to_le_bytes());
            hash.update(requirement_slot.to_le_bytes());
        }
    }
    hash_carry_policy(&mut hash, plan.effective);
    hash.update(plan.live_value_count.to_le_bytes());
    hash.update((plan.live_values.len() as u64).to_le_bytes());
    for live in &plan.live_values {
        match &live.place {
            TerminalSuspensionPlace::Scalar(value) => {
                hash.update([1]);
                hash.update(value.get().to_le_bytes());
            }
            TerminalSuspensionPlace::Structural { place, path } => {
                hash.update([2]);
                hash.update(place.get().to_le_bytes());
                hash.update((path.len() as u64).to_le_bytes());
                for segment in path {
                    match segment {
                        StructuralPathSegment::Field(field) => {
                            hash.update([1]);
                            hash.update((field.len() as u64).to_le_bytes());
                            hash.update(field.as_bytes());
                        }
                        StructuralPathSegment::FixedIndex(index) => {
                            hash.update([2]);
                            hash.update(index.to_le_bytes());
                        }
                    }
                }
            }
        }
        match live.value_type {
            TerminalSuspensionValueType::Scalar(scalar) => {
                hash.update([1]);
                hash_scalar_type(&mut hash, scalar);
            }
            TerminalSuspensionValueType::Structural(structural) => {
                hash.update([2]);
                hash.update(structural.get().to_le_bytes());
            }
        }
        hash.update([match live.storage {
            TerminalSuspensionStorage::Persistent => 1,
            TerminalSuspensionStorage::Parameter => 2,
            TerminalSuspensionStorage::Local => 3,
            TerminalSuspensionStorage::CallArgument => 4,
        }]);
        hash.update(live.claim_count.to_le_bytes());
        hash.update((live.claims.len() as u64).to_le_bytes());
        for claim in &live.claims {
            hash.update(claim.get().to_le_bytes());
        }
        hash_carry_policy(&mut hash, live.effective);
    }
    hash.finalize().into()
}

fn hash_scalar_type(hash: &mut Sha256, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => hash.update([1]),
        ScalarType::Integer(integer) => {
            hash.update([2]);
            hash.update([match integer.carrier() {
                IntegerCarrier::Fixed => 1,
                IntegerCarrier::Address => 2,
            }]);
            hash.update([match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            }]);
            hash.update(integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            hash.update([3]);
            hash.update([match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            }]);
        }
    }
}

fn hash_carry_policy(hash: &mut Sha256, policy: CarryPolicy) {
    hash.update([
        u8::from(policy.suspension == language_semantics::CarrySuspension::Allowed),
        u8::from(policy.cpu == language_semantics::CarryCpu::Any),
        u8::from(policy.host_thread == language_semantics::CarryHostThread::Any),
        u8::from(policy.address == language_semantics::CarryAddress::Movable),
    ]);
}
