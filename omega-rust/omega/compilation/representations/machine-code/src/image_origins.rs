//! Current image-address provenance, projected before machine encoding.
use crate::PlacedImageAddress;
use selected_instructions::{
    SelectedBlockId, SelectedImageAddressReference, SelectedInstructionId,
};
use semantic_vocabulary::{MachineId, OperationId, ScalarType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAddressOrigin {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub reference: SelectedImageAddressReference,
    pub result: ScalarType,
}
impl ImageAddressOrigin {
    pub fn key(&self) -> (MachineId, SelectedBlockId, SelectedInstructionId) {
        (self.machine, self.block, self.instruction)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAddressOrigins {
    pub rows: Vec<ImageAddressOrigin>,
}

impl ImageAddressOrigins {
    pub fn indexes(reference: SelectedImageAddressReference) -> bool {
        matches!(
            reference,
            SelectedImageAddressReference::MachineDescriptor { .. }
                | SelectedImageAddressReference::ImplementationTable { .. }
                | SelectedImageAddressReference::NativeCodeLayout { .. }
        )
    }

    pub fn require(
        &self,
        address: &PlacedImageAddress,
        result: Option<ScalarType>,
    ) -> Result<(), &'static str> {
        let key = (address.machine, address.block, address.instruction);
        let index = self
            .rows
            .binary_search_by_key(&key, |row| row.key())
            .map_err(|_| "placed image address has no current instruction")?;
        let row = &self.rows[index];
        if row.operation != address.operation || row.reference != address.reference {
            return Err("placed image address differs from its current selection");
        }
        if result.is_some_and(|result| result != row.result) {
            return Err("current image address has another result type");
        }
        Ok(())
    }
}
