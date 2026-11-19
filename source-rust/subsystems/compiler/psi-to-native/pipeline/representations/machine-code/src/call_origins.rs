//! Call provenance carried from physical planning. These rows contain call
//! contracts and exact instruction coordinates, not a function control graph.

use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{EdgeId, MachineId, OperationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCallOrigins {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub physical_identity: [u8; 32],
    pub functions: Vec<PhysicalFunctionCallOrigins>,
}

impl PhysicalCallOrigins {
    pub fn function(&self, machine: MachineId) -> Option<&PhysicalFunctionCallOrigins> {
        let mut rows = self.functions.iter().filter(|row| row.machine == machine);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFunctionCallOrigins {
    pub machine: MachineId,
    pub calls: Vec<PhysicalInternalCallOrigin>,
    pub structural_types: Option<native_vocabulary::structural_type_catalog::StructuralTypeCatalog>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalInternalCallOrigin {
    pub owner: target_vocabulary::CallSiteOwner,
    pub instruction: SelectedInstructionId,
    pub call: target_vocabulary::LegalizedScalarCall,
    // A retained contract may be unused after optimization. Absence is not a
    // call: a consumer demanding this origin must require one exact instruction.
    pub occurrence: Option<PhysicalCallInstructionOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCallInstructionOrigin {
    pub target: Option<selected_instructions::SelectedCallTarget>,
    pub block: SelectedBlockId,
    pub callee: Option<MachineId>,
    pub operations: Vec<OperationId>,
    pub edges: Vec<EdgeId>,
}
