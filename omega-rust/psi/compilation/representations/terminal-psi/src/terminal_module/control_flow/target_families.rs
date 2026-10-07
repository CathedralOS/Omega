//! Target-scoped implementations are portable program data. A family names
//! its contract head and every admitted member; Omega chooses a member without
//! changing these identities or the canonical module. Opaque declarations keep
//! their own contract and supply mode rather than becoming empty checked bodies.

use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetMachineCallable {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMachineSupply {
    CheckedBody,
    Requirement,
    TopLevelRequirement,
    Boundary,
    AdmissionClaim,
    ExternalRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetMachineFamilyMember {
    /// Exact authored deployment tag, not an ISA or host-derived projection.
    pub target: String,
    pub callable: TargetMachineCallable,
    pub supply: TargetMachineSupply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetMachineFamily {
    pub canonical: TargetMachineCallable,
    /// Strictly ordered by target; neither reachability nor realization prunes it.
    pub members: Vec<TargetMachineFamilyMember>,
}
