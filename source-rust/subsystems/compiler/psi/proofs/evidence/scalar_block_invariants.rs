use semantic_vocabulary::{BlockId, EdgeId, MachineId, ObligationId, Proposition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarBlockInvariant {
    pub machine: MachineId,
    pub header: BlockId,
    pub predicate: Proposition,
    pub arrivals: Vec<ScalarBlockInvariantArrival>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarBlockInvariantArrival {
    pub edge: EdgeId,
    pub obligation: ObligationId,
}
