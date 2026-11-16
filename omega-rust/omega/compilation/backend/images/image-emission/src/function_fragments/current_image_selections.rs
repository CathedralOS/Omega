//! Match placed descriptor/layout addresses to the current instruction columns.
//! Only this selected family is indexed; no earlier function or analysis graph
//! is needed to match its carried source occurrence and result carrier.

use machine_code::PlacedImageAddress;
use post_allocation_machine::PhysicalFunction;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlockId, SelectedImageAddressReference, SelectedInstructionId, SelectedInstructionKind,
};
use semantic_vocabulary::{MachineId, OperationId, ScalarType};

use super::Error;

type Key = (MachineId, SelectedBlockId, SelectedInstructionId);
type Row = (Key, OperationId, SelectedImageAddressReference, ScalarType);

pub(crate) struct CurrentImageSelections {
    rows: Vec<Row>,
}

impl CurrentImageSelections {
    fn indexes(reference: SelectedImageAddressReference) -> bool {
        matches!(
            reference,
            SelectedImageAddressReference::MachineDescriptor { .. }
                | SelectedImageAddressReference::ImplementationTable { .. }
                | SelectedImageAddressReference::NativeCodeLayout { .. }
        )
    }

    pub(super) fn prepare_for<'input>(
        addresses: &[PlacedImageAddress],
        functions: impl IntoIterator<Item = &'input PhysicalFunction>,
    ) -> Result<Option<Self>, Error> {
        if !addresses
            .iter()
            .any(|address| Self::indexes(address.reference))
        {
            return Ok(None);
        }
        Self::new(functions).map(Some)
    }

    pub(super) fn new<'input>(
        functions: impl IntoIterator<Item = &'input PhysicalFunction>,
    ) -> Result<Self, Error> {
        let mut rows = Vec::new();
        for function in functions {
            for block in &function.blocks {
                for instruction in &block.instructions {
                    let SelectedInstructionKind::ImageAddress { reference } = instruction.kind
                    else {
                        continue;
                    };
                    if !Self::indexes(reference) {
                        continue;
                    }
                    let [operation] = instruction.provenance.operations.as_slice() else {
                        return Err(Error::Mismatch(
                            "current image address has no exact source occurrence",
                        ));
                    };
                    let output = instruction
                        .operand(0)
                        .filter(|operand| {
                            matches!(
                                operand.access,
                                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                            )
                        })
                        .ok_or(Error::Mismatch(
                            "current image address has no result operand",
                        ))?;
                    let register = function
                        .virtual_registers
                        .get(output.virtual_register.0 as usize)
                        .filter(|register| register.id == output.virtual_register)
                        .ok_or(Error::Mismatch(
                            "current image address has an unreadable result register",
                        ))?;
                    rows.push((
                        (function.machine, block.id, instruction.id),
                        *operation,
                        reference,
                        register.scalar_type,
                    ));
                }
            }
        }
        rows.sort_unstable_by_key(|row| row.0);
        if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::Mismatch(
                "current image address repeats its instruction owner",
            ));
        }
        Ok(Self { rows })
    }

    pub(super) fn require(
        &self,
        address: &PlacedImageAddress,
        result: Option<ScalarType>,
    ) -> Result<(), Error> {
        let key = (address.machine, address.block, address.instruction);
        let index = self
            .rows
            .binary_search_by_key(&key, |row| row.0)
            .map_err(|_| Error::Mismatch("placed image address has no current instruction"))?;
        let row = &self.rows[index];
        if row.1 != address.operation || row.2 != address.reference {
            return Err(Error::Mismatch(
                "placed image address differs from its current selection",
            ));
        }
        if result.is_some_and(|result| result != row.3) {
            return Err(Error::Mismatch(
                "current image address has another result type",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{
        SelectedBlock, SelectedBlockOrigin, SelectedFunctionContracts, SelectedInstruction,
        SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{BlockId, CallableContractId, EdgeId, MachineDescriptorId, ValueId};
    use std::sync::Arc;

    // These are instruction-column fixtures, not executable admission inputs.
    fn fixture() -> (PhysicalFunction, PlacedImageAddress, ScalarType) {
        let machine = MachineId::new(1).unwrap();
        let operation = OperationId::new(1).unwrap();
        let result = ScalarType::Callable(CallableContractId::new(1).unwrap());
        let reference = SelectedImageAddressReference::MachineDescriptor {
            descriptor: MachineDescriptorId::new(1).unwrap(),
        };
        let instruction = SelectedInstructionId(0);
        let register = VirtualRegisterId(0);
        let class = register_model::RegisterClassId(0);
        let function = PhysicalFunction {
            contracts: Arc::new(SelectedFunctionContracts {
                machine,
                attachment: None,
                provenance: Default::default(),
                structural: None,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                virtual_registers: vec![VirtualRegister {
                    id: register,
                    scalar_type: result,
                    class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction,
                        source_value: ValueId::new(1).unwrap(),
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                }],
            }),
            entry_block: SelectedBlockId(0),
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![SelectedInstruction {
                    id: instruction,
                    kind: SelectedInstructionKind::ImageAddress { reference },
                    constraint: register_model::RegisterConstraintKey::instruction(0),
                    operands: vec![SelectedOperand {
                        operand: 0,
                        virtual_register: register,
                        access: RegisterOperandAccess::Def,
                        class,
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    }],
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance {
                        operations: vec![operation],
                        ..Default::default()
                    },
                }],
                terminator: SelectedTerminator::Return {
                    instruction: SelectedInstruction {
                        id: SelectedInstructionId(1),
                        kind: SelectedInstructionKind::ReturnUnit,
                        constraint: register_model::RegisterConstraintKey::ret(0),
                        operands: Vec::new(),
                        implicit_uses: Vec::new(),
                        implicit_defs: Vec::new(),
                        clobbers: Vec::new(),
                        provenance: SelectedInstructionProvenance {
                            edges: vec![EdgeId::new(1).unwrap()],
                            ..Default::default()
                        },
                    },
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }]
            .into(),
        };
        let address = PlacedImageAddress {
            machine,
            block: SelectedBlockId(0),
            instruction,
            reference,
            operation,
            instruction_function_offset: 0,
            instruction_section_offset: 0,
            instruction_byte_count: 7,
            fields: machine_code::ImageAddressFields::X86_64Relative32 { patch_offset: 3 },
        };
        (function, address, result)
    }

    #[test]
    fn placed_addresses_keep_exact_current_owners_references_and_types() {
        let (function, address, result) = fixture();
        let rows = CurrentImageSelections::new(std::slice::from_ref(&function)).unwrap();
        rows.require(&address, Some(result)).unwrap();
        assert!(
            CurrentImageSelections::prepare_for(&[], std::iter::once(&function))
                .unwrap()
                .is_none()
        );
        let mut scratch = function.clone();
        scratch.virtual_registers[0].origin = VirtualRegisterOrigin::InstructionScratch {
            instruction: address.instruction,
            operand: 0,
        };
        CurrentImageSelections::new(&[scratch])
            .unwrap()
            .require(&address, Some(result))
            .unwrap();
        let mut changed = address;
        changed.operation = OperationId::new(2).unwrap();
        assert!(rows.require(&changed, Some(result)).is_err());
        changed = address;
        changed.reference = SelectedImageAddressReference::MachineDescriptor {
            descriptor: MachineDescriptorId::new(2).unwrap(),
        };
        assert!(rows.require(&changed, Some(result)).is_err());
        changed = address;
        changed.block = SelectedBlockId(1);
        assert!(rows.require(&changed, Some(result)).is_err());
        assert!(rows.require(&address, Some(ScalarType::Boolean)).is_err());
        let mut unreadable = function.clone();
        unreadable.virtual_registers.clear();
        assert!(CurrentImageSelections::new(&[unreadable]).is_err());
        let mut repeated = function.clone();
        let instruction = repeated.blocks[0].instructions[0].clone();
        repeated.blocks[0].instructions.push(instruction);
        assert!(CurrentImageSelections::new(&[repeated]).is_err());
        let mut no_origin = function;
        no_origin.blocks[0].instructions[0]
            .provenance
            .operations
            .clear();
        assert!(CurrentImageSelections::new(&[no_origin]).is_err());
    }
}
