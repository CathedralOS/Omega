use super::super::*;
use super::foundations::NodeLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockParameterIncomingBinding {
    pub source: BlockId,
    pub edge: EdgeId,
    pub argument: ValueId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantBlockParameterWitness {
    pub incoming: Vec<BlockParameterIncomingBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedundantBlockParameterRewrite {
    pub machine: MachineId,
    pub block: BlockId,
    pub position: u32,
    pub parameter: ValueId,
    pub replacement: ValueId,
    pub scalar_type: ScalarType,
}

/// Replace one Boolean-proven conditional with its exact selected edge. Both
/// edge identities are bound so replay cannot silently swap or discard a
/// different successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstantConditionalRewrite {
    pub location: NodeLocation,
    pub condition: ValueId,
    pub constant: bool,
    pub selected_edge: EdgeId,
    pub rejected_edge: EdgeId,
}

/// Thread one non-entry, single-incoming block whose only node is an
/// unconditional jump. The predecessor and removed jump are necessarily
/// co-executed, so both source edges remain realized at `predecessor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinearEmptyBlockRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub empty: NodeLocation,
    pub outgoing_edge: EdgeId,
    pub target: BlockId,
}

/// Thread one non-entry empty jump block through every exact incoming edge.
/// The outgoing source occurrence fans out to those mutually exclusive edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathQualifiedEmptyBlockRewrite {
    pub empty: NodeLocation,
    pub outgoing_edge: EdgeId,
    pub target: BlockId,
}

/// Merge the immediately adjacent, single-predecessor target block into an
/// unconditional predecessor. The target's block parameters are replaced by
/// the exact incoming bindings. The removed edge is realized at the first
/// moved operation or, for a conditional-only target, on both mutually
/// exclusive successor edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdjacentBlockMergeRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// One exact verifier-owned ownership fact consumed by an adjacent block
/// merge. Rows are canonical in source-site order; the rule-specific
/// validator reconstructs both the required site set and each fact identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierWitnessRow {
    pub site: OwnershipFrontierSite,
    pub fact: OwnershipFrontierFactIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierWitness {
    pub rows: Vec<OwnershipFrontierWitnessRow>,
}

/// Merge a non-adjacent, single-predecessor target block into its
/// unconditional predecessor. Unlike the adjacent form, this patch explicitly
/// authorizes movement across intervening source-roster blocks; execution
/// legality is still established from CFG dominance rather than roster order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonAdjacentBlockMergeRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// Fuse one unconditional jump into a shared, terminal-only target without
/// removing that target. The terminal occurrence is cloned onto the selected
/// incoming path and remains at the target for every other incoming path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedJumpFusionRewrite {
    pub predecessor: NodeLocation,
    pub incoming_edge: EdgeId,
    pub target: BlockId,
}

/// Remove the exact canonical complement of the independently reconstructed
/// executable-machine root closure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnreachablePrivateMachinesRewrite {
    pub machines: Vec<crate::PrunedMachineCustody>,
}
