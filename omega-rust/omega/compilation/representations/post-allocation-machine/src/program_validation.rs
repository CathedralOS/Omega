//! Explicit replay checks the current physical columns against their original
//! admitted artifacts. Ordinary encoding never reconstructs those artifacts.

use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::AllocatedCalleeSavedRequirements;
use selected_instructions::{
    CurrentSelectedProgram, selected_instruction_plan_identity, terminator_instruction,
};

use crate::{
    PostAllocationMachinePlan, PostAllocationMachineProgram, post_allocation_machine_identity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalProgramValidationError {
    SourceMismatch,
    MachineMismatch,
    RegisterMismatch,
    PreservationMismatch,
    FunctionRosterMismatch,
    BlockRosterMismatch,
    InstructionRosterMismatch,
    IdentityMismatch,
}

/// Original admitted owners used only by an independent correspondence check.
#[derive(Clone, Copy)]
pub struct PhysicalProgramReplay<'source> {
    pub selected: CurrentSelectedProgram<'source>,
    pub machine: &'source PostAllocationMachinePlan,
    pub environment: &'source ValidatedTargetRegisterEnvironment,
    pub preservation: &'source AllocatedCalleeSavedRequirements,
}

pub fn validate_current_program(
    program: &PostAllocationMachineProgram,
    replay: PhysicalProgramReplay<'_>,
) -> Result<(), PhysicalProgramValidationError> {
    use PhysicalProgramValidationError as Error;
    let PhysicalProgramReplay {
        selected,
        machine,
        environment,
        preservation: original_preservation,
    } = replay;
    let physical = environment.physical();
    if selected.identity != selected_instruction_plan_identity(selected.plan)
        || program.contracts != selected.plan.contracts
        || machine.selected != selected.identity
        || program.psi != selected.plan.psi
        || program.work_schedule != selected.work_schedule
        || program.entry != selected.plan.entry
        || selected.plan.target != machine.target
    {
        return Err(Error::SourceMismatch);
    }
    if program.machine.as_ref() != machine
        || machine.identity != post_allocation_machine_identity(machine)
    {
        return Err(Error::MachineMismatch);
    }
    if &program.environment != environment
        || program.physical() != physical
        || machine.physical_register_model != physical.identity()
        || program.environment.identity() != machine.register_environment
        || program.environment.target() != machine.target
    {
        return Err(Error::RegisterMismatch);
    }
    let preservation = program.preservation.plan();
    if &program.preservation != original_preservation
        || program.preservation.identity()
            != register_homes::allocated_callee_saved_requirement_identity(preservation)
        || preservation.selected != machine.selected
        || preservation.homes != machine.homes
        || preservation.post_allocation_manifest != machine.post_allocation_manifest
        || preservation.register_environment != machine.register_environment
        || preservation.physical_register_model != physical.identity()
        || preservation.target != machine.target
    {
        return Err(Error::PreservationMismatch);
    }
    if program.functions.len() != selected.plan.functions.len()
        || program.functions.len() != machine.functions.len()
    {
        return Err(Error::FunctionRosterMismatch);
    }
    for ((current, original), physical) in program
        .functions
        .iter()
        .zip(selected.plan.functions.iter())
        .zip(&machine.functions)
    {
        if current.machine != physical.machine
            || current.contracts != original.contracts
            || current.entry_block != original.entry_block
            || current.blocks != original.blocks
            || current.local_storage_slots != physical.local_storage_slots
            || current.outgoing_arguments != physical.outgoing_arguments
        {
            return Err(Error::SourceMismatch);
        }
        if current.blocks.len() != physical.blocks.len() {
            return Err(Error::BlockRosterMismatch);
        }
        for (block, physical) in current.blocks.iter().zip(&physical.blocks) {
            if block.id != physical.block
                || block.instructions.len() + 1 != physical.instructions.len()
            {
                return Err(Error::BlockRosterMismatch);
            }
            if block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
                .zip(&physical.instructions)
                .any(|(operation, physical)| operation.id != physical.instruction)
            {
                return Err(Error::InstructionRosterMismatch);
            }
        }
    }
    if program.identity != program.recomputed_identity() {
        return Err(Error::IdentityMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, PostAllocationOptimizationManifestIdentity};
    use register_homes::{AllocationLegalityIdentity, RegisterHomeIdentity};
    use register_model::RegisterConstraintCatalogIdentity;
    use selected_instructions::{
        LiveRangeIdentity, MachineEffectCatalogIdentity, MachineSemanticKind,
        PreAllocationMachineEffectIdentity, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
        SelectedFunction, SelectedFunctionContracts, SelectedInstruction, SelectedInstructionId,
        SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, WorkScheduleIdentity};
    use target::NativeTarget;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        CurrentSelectedProgram, PostAllocationMachinePlan, PostAllocationMachineProgram,
        post_allocation_machine_identity, selected_instruction_plan_identity,
    };
    use super::{
        PhysicalProgramReplay, PhysicalProgramValidationError as Error, validate_current_program,
    };
    use crate::{
        MachineAlternativeChoiceRule, PostAllocationMachineBlock, PostAllocationMachineFunction,
        PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    };

    fn fixture() -> (SelectedInstructionPlan, PostAllocationMachineProgram) {
        let machine_id = MachineId::new(1).unwrap();
        let target = NativeTarget::linux_x64();
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let physical = environment.physical().clone();
        let catalog =
            isa_x86_64::x86_64_machine_effect_catalog(target, environment.constraints()).unwrap();
        let declaration = catalog
            .declaration_by_semantic(MachineSemanticKind::ReturnUnit)
            .unwrap();
        let constraint = environment.constraint(declaration.constraint).unwrap();
        let operation = SelectedInstruction {
            id: SelectedInstructionId(7),
            kind: SelectedInstructionKind::ReturnUnit,
            constraint: declaration.constraint,
            operands: Vec::new(),
            implicit_uses: constraint.implicit_uses.clone(),
            implicit_defs: constraint.implicit_defs.clone(),
            clobbers: constraint.clobbers.clone(),
            provenance: Default::default(),
        };
        let alternative = declaration.alternatives[0].clone();
        let physical_operation = PostAllocationMachineInstruction {
            instruction: operation.id,
            operands: Vec::new(),
            address: None,
            implicit_unit_uses: alternative.encoded.implicit_unit_uses.clone(),
            implicit_unit_defs: alternative.encoded.implicit_unit_defs.clone(),
            implicit_unit_clobbers: alternative.encoded.implicit_unit_clobbers.clone(),
            unit_uses: alternative.encoded.implicit_unit_uses.clone(),
            unit_defs: alternative.encoded.implicit_unit_defs.clone(),
            unit_clobbers: alternative.encoded.implicit_unit_clobbers.clone(),
            alternative,
        };
        let selected = SelectedInstructionPlan {
            contracts: Default::default(),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            entry: machine_id,
            functions: vec![SelectedFunction {
                contracts: Arc::new(SelectedFunctionContracts {
                    machine: machine_id,
                    attachment: None,
                    provenance: Default::default(),
                    structural: None,
                    local_storage_slots: Vec::new(),
                    outgoing_arguments: Vec::new(),
                    calls: Vec::new(),
                    normalized_foreign_calls: Vec::new(),
                    memory_accesses: Vec::new(),
                    boundary_settlements: Vec::new(),
                    virtual_registers: Vec::new(),
                }),
                entry_block: SelectedBlockId(0),
                blocks: vec![SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: operation,
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }]
                .into(),
            }]
            .into(),
        };
        let mut machine = PostAllocationMachinePlan {
            identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
            selected: selected_instruction_plan_identity(&selected),
            effects: PreAllocationMachineEffectIdentity::from_bytes([2; 32]),
            ranges: LiveRangeIdentity::from_bytes([3; 32]),
            legality: AllocationLegalityIdentity::from_bytes([4; 32]),
            homes: RegisterHomeIdentity::from_bytes([5; 32]),
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
                [6; 32],
            ),
            target: selected.target,
            register_environment: environment.identity(),
            physical_register_model: physical.identity(),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([8; 32]),
            machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([9; 32]),
            choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
            functions: vec![PostAllocationMachineFunction {
                machine: machine_id,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                blocks: vec![PostAllocationMachineBlock {
                    block: SelectedBlockId(0),
                    instructions: vec![physical_operation],
                }],
            }],
        };
        machine.identity = post_allocation_machine_identity(&machine);
        let convention = register_environment::selected_abi_preservation(&environment).unwrap();
        let preservation_plan = register_homes::AllocatedCalleeSavedRequirementPlan {
            selected: machine.selected,
            homes: machine.homes,
            post_allocation_manifest: machine.post_allocation_manifest,
            register_environment: machine.register_environment,
            physical_register_model: physical.identity(),
            target,
            abi: convention.kind,
            callee_saved_units: convention.convention.callee_saved.clone(),
            policy: register_homes::AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
            budget: optimization_core::OptimizationWorkBudget::new(100, 100, 100, 100, 100).unwrap(),
            usage: Default::default(),
            functions: vec![register_homes::FunctionAllocatedCalleeSavedRequirements {
                machine: machine_id,
                modified_units: Vec::new(),
            }],
        };
        let preservation = register_homes::AllocatedCalleeSavedRequirements {
            identity: register_homes::allocated_callee_saved_requirement_identity(
                &preservation_plan,
            ),
            plan: Arc::new(preservation_plan),
        };
        let mut program = PostAllocationMachineProgram {
            contracts: Arc::clone(&selected.contracts),
            psi: selected.psi,
            work_schedule: selected.work_schedule,
            entry: selected.entry,
            environment,
            preservation,
            machine: Arc::new(machine),
            identity: [0; 32],
            functions: selected.functions.clone(),
        };
        program.identity = program.recomputed_identity();
        (selected, program)
    }

    fn source(selected: &SelectedInstructionPlan) -> CurrentSelectedProgram<'_> {
        CurrentSelectedProgram {
            plan: selected,
            identity: selected_instruction_plan_identity(selected),
            optimization_unit: OptimizationUnitIdentity::from_bytes([10; 32]),
            work_schedule: selected.work_schedule,
        }
    }

    fn replay<'a>(
        selected: &'a SelectedInstructionPlan,
        admitted: &'a PostAllocationMachineProgram,
    ) -> PhysicalProgramReplay<'a> {
        PhysicalProgramReplay {
            selected: source(selected),
            machine: &admitted.machine,
            environment: &admitted.environment,
            preservation: &admitted.preservation,
        }
    }

    #[test]
    fn matching_current_columns_share_the_original_contract_chunk() {
        let (selected, program) = fixture();
        assert!(Arc::ptr_eq(&selected.contracts, &program.contracts));
        assert!(
            program
                .functions
                .shares_function_storage(&selected.functions, 0)
        );
        assert!(Arc::ptr_eq(
            &selected.functions[0].contracts,
            &program.functions[0].contracts
        ));
        assert_eq!(
            validate_current_program(&program, replay(&selected, &program)),
            Ok(())
        );
    }

    #[test]
    fn a_relabelled_contract_chunk_is_refused_even_after_rehashing() {
        let (selected, mut program) = fixture();
        let admitted = program.clone();
        Arc::make_mut(&mut program.functions.get_mut(0).unwrap().contracts)
            .provenance
            .operations
            .push(OperationId::new(27).unwrap());
        program.identity = program.recomputed_identity();
        assert_eq!(
            validate_current_program(&program, replay(&selected, &admitted)),
            Err(Error::SourceMismatch)
        );
    }

    #[test]
    fn changed_machine_bytes_cannot_keep_a_claimed_matching_identity() {
        let (selected, mut program) = fixture();
        let admitted = program.clone();
        Arc::make_mut(&mut program.machine).functions[0].machine = MachineId::new(2).unwrap();
        program.identity = program.recomputed_identity();
        assert_eq!(
            validate_current_program(&program, replay(&selected, &admitted)),
            Err(Error::MachineMismatch)
        );
    }

    #[test]
    fn a_rehashed_physical_instruction_id_must_match_the_current_operation() {
        let (selected, mut program) = fixture();
        let machine = Arc::make_mut(&mut program.machine);
        machine.functions[0].blocks[0].instructions[0].instruction = SelectedInstructionId(8);
        machine.identity = post_allocation_machine_identity(machine);
        program.identity = program.recomputed_identity();
        // A canonical machine body still has to join its operation column.
        // The separate substitution control checks replacement of the owner.
        let admitted = program.clone();
        assert_eq!(
            validate_current_program(&program, replay(&selected, &admitted)),
            Err(Error::InstructionRosterMismatch)
        );
    }

    #[test]
    fn a_rehashed_current_machine_cannot_replace_the_original_sealed_machine() {
        let (selected, mut program) = fixture();
        let admitted = program.clone();
        let machine = Arc::make_mut(&mut program.machine);
        machine.functions[0].blocks[0].instructions[0]
            .alternative
            .key
            .variant += 1;
        machine.identity = post_allocation_machine_identity(machine);
        program.identity = program.recomputed_identity();
        assert_eq!(
            validate_current_program(&program, replay(&selected, &admitted)),
            Err(Error::MachineMismatch)
        );
    }
    #[test]
    fn a_rehashed_preservation_body_cannot_replace_the_admitted_requirements() {
        let (selected, mut program) = fixture();
        let admitted = program.clone();
        Arc::make_mut(&mut program.preservation.plan)
            .usage
            .candidates += 1;
        program.preservation.identity = register_homes::allocated_callee_saved_requirement_identity(
            program.preservation.plan(),
        );
        program.identity = program.recomputed_identity();
        assert_eq!(
            validate_current_program(&program, replay(&selected, &admitted)),
            Err(Error::PreservationMismatch)
        );
    }

    #[test]
    fn rehashed_program_contracts_cannot_replace_the_admitted_layouts() {
        let (selected, mut program) = fixture();
        let admitted = program.clone();
        let layout = runtime_abi::NativeBorrowedStorageLayout::new(
            semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            8,
            4,
            [(
                Vec::new(),
                Vec::new(),
                4,
                semantic_vocabulary::ScalarType::Boolean,
                semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        13,
                    )
                    .unwrap(),
                ),
            )],
            [],
            [],
        );
        Arc::make_mut(&mut program.contracts).borrowed_storage_layouts =
            runtime_abi::NativeBorrowedStorageLayouts::seal(vec![layout]).unwrap();
        program.identity = program.recomputed_identity();
        assert_eq!(
            validate_current_program(&program, replay(&selected, &admitted)),
            Err(Error::SourceMismatch)
        );
    }

    #[test]
    fn rehashed_abi_and_profile_cannot_replace_the_admitted_contracts() {
        let (selected, original) = fixture();
        let abi = target_vocabulary::function_abi::LegalizedFunctionAbi {
            machine: original.entry,
            call_plan: calling_conventions::evaluate_call_plan(
                calling_conventions::CallingPolicy::MicrosoftX64,
                &calling_conventions::CallSignature {
                    parameters: Vec::new(),
                    result: None,
                },
            )
            .unwrap(),
            scalar_parameters: Vec::new(),
            entry_scalar_codecs: Vec::new(),
            structural_parameters: Vec::new(),
            scalar: None,
            mixed_structural_scalar: None,
        };
        for (profile, abis) in [
            (Some(target::TargetProfile::WindowsX64), Arc::from([])),
            (None, Arc::from([abi])),
        ] {
            let mut changed = original.clone();
            changed.contracts = Arc::new(
                selected_instructions::SelectedProgramContracts::new(
                    Default::default(),
                    profile,
                    abis,
                )
                .unwrap(),
            );
            changed.identity = changed.recomputed_identity();
            assert_eq!(
                validate_current_program(&changed, replay(&selected, &original)),
                Err(Error::SourceMismatch)
            );
        }
    }
}
