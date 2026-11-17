//! Resolve placed descriptor and layout references against their carried
//! physical origins. The object producer cannot reopen instruction graphs.
use super::Error;
use machine_code::{PlacedImageAddress, image_origins::ImageAddressOrigins};
use semantic_vocabulary::ScalarType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CurrentImageSelections(ImageAddressOrigins);

impl CurrentImageSelections {
    pub(super) fn prepare_for(
        addresses: &[PlacedImageAddress],
        origins: &Result<ImageAddressOrigins, &'static str>,
    ) -> Result<Option<Self>, Error> {
        if !addresses
            .iter()
            .any(|address| ImageAddressOrigins::indexes(address.reference))
        {
            return Ok(None);
        }
        origins
            .clone()
            .map(|origins| Some(Self(origins)))
            .map_err(Error::Mismatch)
    }
    pub(super) fn require(
        &self,
        address: &PlacedImageAddress,
        result: Option<ScalarType>,
    ) -> Result<(), Error> {
        self.0.require(address, result).map_err(Error::Mismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::PlacedImageAddress;
    use post_allocation_machine::PhysicalFunction;
    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        SelectedBlockId, SelectedImageAddressReference, SelectedInstructionId,
        SelectedInstructionKind,
    };
    use semantic_vocabulary::{MachineId, OperationId, ScalarType};
    fn current(functions: &[PhysicalFunction]) -> Result<CurrentImageSelections, &'static str> {
        physical_planning::image_origins::capture(functions).map(CurrentImageSelections)
    }
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
                structural_block_parameters: Vec::new(),
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
        let rows = current(std::slice::from_ref(&function)).unwrap();
        rows.require(&address, Some(result)).unwrap();
        assert!(
            CurrentImageSelections::prepare_for(
                &[],
                &physical_planning::image_origins::capture(std::iter::once(&function))
            )
            .unwrap()
            .is_none()
        );
        let mut scratch = function.clone();
        scratch.virtual_registers[0].origin = VirtualRegisterOrigin::InstructionScratch {
            instruction: address.instruction,
            operand: 0,
        };
        current(&[scratch])
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
        assert!(current(&[unreadable]).is_err());
        let mut repeated = function.clone();
        let instruction = repeated.blocks[0].instructions[0].clone();
        repeated.blocks[0].instructions.push(instruction);
        assert!(current(&[repeated]).is_err());
        let mut no_origin = function;
        no_origin.blocks[0].instructions[0]
            .provenance
            .operations
            .clear();
        assert!(current(&[no_origin]).is_err());
    }
}
