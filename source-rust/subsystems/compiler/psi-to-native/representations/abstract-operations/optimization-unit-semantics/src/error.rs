/*
The descriptor, reference and callable-value queries refuse a unit they cannot
answer for with `OptimizationUnitValidationError`, naming the operation by its
`OperationSite` where one applies.
*/

use semantic_vocabulary::{BlockId, MachineId, OperationId, StructuralTypeId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSite {
    pub machine: MachineId, // Machine whose function contains the rejected operation.
    pub block: BlockId,     // Basic block containing the rejected operation.
    pub node: u32,          // Zero-based node index within the block.
}

impl OperationSite {
    pub const fn new(machine: MachineId, block: BlockId, node: u32) -> Self {
        Self {
            machine,
            block,
            node,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitValidationError {
    CallableValueProvenance(&'static str),
    InvalidActivationStorage {
        machine: MachineId,
        reason: &'static str,
    },
    InvalidOwnedValueTransfer {
        machine: MachineId,
        operation: OperationId,
        reason: &'static str,
    },
    FunctionResultMismatch(MachineId),
    UnorderedMachine(MachineId),
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    InvalidWriteOnlyPrimitiveStore(OperationSite),
    InvalidStructuralScalarFieldStore(OperationSite),
    InvalidByteSequenceRead(OperationSite),
    InvalidByteSequenceWrite(OperationSite),
    InvalidByteSequenceSubslice(OperationSite),
    InvalidByteSequenceLength(OperationSite),
    UnorderedBlock {
        machine: MachineId,
        block: BlockId,
    },
    UnreachableBlock {
        machine: MachineId,
        block: BlockId,
    },
    DuplicateValue(ValueId),
    UseBeforeDefinition {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    TerminalIdentityMismatch,
}

impl std::fmt::Display for OptimizationUnitValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi optimization unit: {self:?}")
    }
}

impl std::error::Error for OptimizationUnitValidationError {}
