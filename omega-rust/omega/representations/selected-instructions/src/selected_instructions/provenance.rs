//! Semantic operations, values, edges, proof obligations and fuel retained by an instruction.
use optimization_unit::FuelSettlement;
use semantic_vocabulary::{EdgeId, ObligationId, OperationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedInstructionProvenance {
    pub operations: Vec<OperationId>,
    pub values: Vec<ValueId>,
    pub edges: Vec<EdgeId>,
    pub obligations: Vec<ObligationId>,
    pub fuel: Vec<FuelSettlement>,
}
