//! Boundary publication obligations fixed before encoding.
//! An anchor identifies an encoded occurrence; no source instruction graph is retained.

use selected_instructions::{
    SelectedBlockId, SelectedBoundarySettlementPayload, SelectedInstructionId,
};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalBoundarySettlements {
    pub functions: Vec<PhysicalBoundarySettlementFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalBoundarySettlementFunction {
    pub machine: MachineId,
    pub rows: Result<Vec<PhysicalBoundarySettlement>, &'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalBoundarySettlement {
    pub block: SelectedBlockId,
    pub anchor: SelectedInstructionId,
    pub settlement: SelectedBoundarySettlementPayload,
    pub read_result_declaration: Option<terminal_psi::StructuralTypeDeclaration>,
}

impl PhysicalBoundarySettlements {
    pub fn function(
        &self,
        machine: MachineId,
    ) -> Result<&[PhysicalBoundarySettlement], &'static str> {
        let mut matching = self.functions.iter().filter(|row| row.machine == machine);
        let function = matching
            .next()
            .ok_or("settlements have no physical function")?;
        if matching.next().is_some() {
            return Err("settlements have duplicate physical functions");
        }
        function.rows.as_deref().map_err(|reason| *reason)
    }
}
