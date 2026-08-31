use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeLocation {
    pub machine: MachineId,
    pub block: BlockId,
    pub node: u32,
}

/// An exact occurrence of source semantic work in an optimization revision.
/// Successor edges are separate from their owner node because only the taken
/// arm executes. This distinction is required for path-dependent rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRealizationSite {
    Node(NodeLocation),
    Edge { machine: MachineId, edge: EdgeId },
}

impl PsiRealizationSite {
    pub const fn machine(self) -> MachineId {
        match self {
            Self::Node(location) => location.machine,
            Self::Edge { machine, .. } => machine,
        }
    }

    pub const fn node(self) -> Option<NodeLocation> {
        match self {
            Self::Node(location) => Some(location),
            Self::Edge { .. } => None,
        }
    }
}

/// The exact disposition of source semantic work after one accepted rewrite.
///
/// A realized row names a node in the output revision. A proven-unreachable
/// row instead names the node in the input revision that owned a removed source
/// site; the node itself may survive, as with one rejected conditional edge.
/// Removal is legal only because the validating rewrite proved that no
/// execution can reach that source site. Fuel rows retain the source schedule
/// amount in both cases; only a realized disposition is a logical charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProvenanceDisposition {
    RealizedAt(PsiRealizationSite),
    ProvenUnreachableAt(PsiRealizationSite),
}

impl ProvenanceDisposition {
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::RealizedAt(_) => 1,
            Self::ProvenUnreachableAt(_) => 2,
        }
    }

    pub const fn site(self) -> PsiRealizationSite {
        match self {
            Self::RealizedAt(site) | Self::ProvenUnreachableAt(site) => site,
        }
    }

    pub const fn is_realized(self) -> bool {
        matches!(self, Self::RealizedAt(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarSubstitution {
    pub from: ValueId,
    pub to: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRewrite {
    /// Exact occurrence in the input revision whose custody is transformed.
    pub input: PsiRealizationSite,
    pub disposition: ProvenanceDisposition,
    pub sources: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
}
