use isa_aarch64::aarch64_physical_register_model;
use optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
};
use optimization_unit::{FuelSettlement, PsiProvenance};
use register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterOperandAccess, TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    validate_physical_register_model,
};
use selected_instructions::{
    BlockLiveness, FunctionLiveness, InstructionLiveness, LivenessPlan, LivenessPosition,
    liveness_identity,
};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock,
    SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedSuccessor, SelectedTerminator,
    VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, ValueId};
use target::NativeTarget;
use target_operations::TerminalPsiProvenance;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::CbnzFusionInputs;
use physical_instructions::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan,
};
use selected_instructions::PreAllocationMachineEffectIdentity;

pub(super) struct Fixture {
    pub selected: SelectedInstructionPlan,
    pub selected_identity: SelectedInstructionPlanIdentity,
    pub liveness: LivenessPlan,
    pub source: PostAllocationMachinePlan,
    pub physical: ValidatedPhysicalRegisterModel,
}

impl Fixture {
    pub(super) fn inputs(&self) -> CbnzFusionInputs<'_> {
        CbnzFusionInputs {
            selected: &self.selected,
            selected_identity: self.selected_identity,
            liveness: &self.liveness,
            liveness_identity: liveness_identity(&self.liveness),
            source: &self.source,
            source_identity: self.source.identity,
            physical: &self.physical,
        }
    }
}

pub(super) fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(2, 1, 1, 1, 2).unwrap()
}

pub(super) fn fixture() -> Fixture {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x0 = physical.model().view_named("x0").unwrap();
    let nzcv = physical.model().view_named("nzcv").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let machine = MachineId::new(1).unwrap();
    let block = SelectedBlockId(0);
    let source_block = BlockId::new(1).unwrap();
    let compare_id = SelectedInstructionId(0);
    let branch_id = SelectedInstructionId(1);
    let condition = ValueId::new(2).unwrap();
    let operation = OperationId::new(3).unwrap();
    let fuel = vec![FuelSettlement {
        site: PsiProvenance::Operation(operation),
        units: 2,
    }];
    let constraint = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 0,
    };
    let compare_operand = SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(0),
        access: RegisterOperandAccess::Use,
        class: x0.class,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    };
    let compare = SelectedInstruction {
        id: compare_id,
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint,
        operands: vec![compare_operand],
        implicit_uses: Vec::new(),
        implicit_defs: nzcv.units.clone(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            operations: vec![operation],
            values: vec![condition],
            fuel: fuel.clone(),
            ..Default::default()
        },
    };
    let branch = SelectedInstruction {
        id: branch_id,
        kind: SelectedInstructionKind::ConditionalBranchNonZero,
        constraint,
        operands: Vec::new(),
        implicit_uses: units(nzcv.units.iter().chain(&pc.units).copied()),
        implicit_defs: pc.units.clone(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: vec![condition],
            ..Default::default()
        },
    };
    let selected_identity = SelectedInstructionPlanIdentity::from_bytes([2; 32]);
    let selected = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: NativeTarget::linux_arm64(),
        entry: machine,
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            entry_block: block,
            virtual_registers: Vec::new(),
            blocks: vec![SelectedBlock {
                id: block,
                source_block,
                instructions: vec![compare],
                terminator: SelectedTerminator::ConditionalBranch {
                    instruction: branch,
                    when_nonzero: successor(4, block),
                    when_zero: successor(5, block),
                },
            }],
        }],
        structural_unit_functions: Vec::new(),
        projected_structural_call_returns: Vec::new(),
    };
    let nzcv_pc = units(nzcv.units.iter().chain(&pc.units).copied());
    let x0_pc = units(x0.units.iter().chain(&pc.units).copied());
    let liveness = LivenessPlan {
        selected: selected_identity,
        optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"cbnz-fuel"),
        fuel_schedule: selected.fuel_schedule,
        target: selected.target,
        functions: vec![FunctionLiveness {
            machine,
            entry_definitions: Vec::new(),
            operand_positions: Vec::new(),
            blocks: vec![BlockLiveness {
                block,
                source_block,
                virtual_live_in: vec![VirtualRegisterId(0)],
                virtual_live_out: Vec::new(),
                unit_live_in: x0_pc.clone(),
                unit_live_out: Vec::new(),
                instructions: vec![
                    InstructionLiveness {
                        position: LivenessPosition(0),
                        instruction: compare_id,
                        virtual_uses: vec![VirtualRegisterId(0)],
                        virtual_defs: Vec::new(),
                        virtual_live_in: vec![VirtualRegisterId(0)],
                        virtual_live_out: Vec::new(),
                        unit_uses: x0.units.clone(),
                        unit_defs: nzcv.write_units.clone(),
                        unit_clobbers: Vec::new(),
                        unit_live_in: x0_pc,
                        unit_live_out: nzcv_pc.clone(),
                    },
                    InstructionLiveness {
                        position: LivenessPosition(1),
                        instruction: branch_id,
                        virtual_uses: Vec::new(),
                        virtual_defs: Vec::new(),
                        virtual_live_in: Vec::new(),
                        virtual_live_out: Vec::new(),
                        unit_uses: nzcv_pc.clone(),
                        unit_defs: pc.write_units.clone(),
                        unit_clobbers: Vec::new(),
                        unit_live_in: nzcv_pc.clone(),
                        unit_live_out: Vec::new(),
                    },
                ],
                successors: Vec::new(),
            }],
        }],
        structural_unit_functions: Vec::new(),
    };
    let compare_machine = PostAllocationMachineInstruction {
        instruction: compare_id,
        alternative: alternative(
            MachineAlternativeFamily::CompareI64Zero,
            MachineEncodedEffects {
                external_operand_reads: vec![0],
                external_operand_writes: Vec::new(),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: nzcv.units.clone(),
                implicit_unit_clobbers: Vec::new(),
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::NeverV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        ),
        operands: vec![physical_operand(x0)],
        implicit_unit_uses: Vec::new(),
        implicit_unit_defs: nzcv.units.clone(),
        implicit_unit_clobbers: Vec::new(),
        unit_uses: x0.units.clone(),
        unit_defs: nzcv.write_units.clone(),
        unit_clobbers: Vec::new(),
    };
    let branch_machine = PostAllocationMachineInstruction {
        instruction: branch_id,
        alternative: alternative(
            MachineAlternativeFamily::ConditionalBranchNonZero,
            MachineEncodedEffects {
                external_operand_reads: Vec::new(),
                external_operand_writes: Vec::new(),
                implicit_unit_uses: nzcv_pc.clone(),
                implicit_unit_defs: pc.units.clone(),
                implicit_unit_clobbers: Vec::new(),
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            },
        ),
        operands: Vec::new(),
        implicit_unit_uses: nzcv_pc.clone(),
        implicit_unit_defs: pc.units.clone(),
        implicit_unit_clobbers: Vec::new(),
        unit_uses: nzcv_pc,
        unit_defs: pc.write_units.clone(),
        unit_clobbers: Vec::new(),
    };
    let source_identity = PostAllocationMachineIdentity::from_bytes([4; 32]);
    let source = PostAllocationMachinePlan {
        identity: source_identity,
        selected: selected_identity,
        effects: PreAllocationMachineEffectIdentity::from_bytes([5; 32]),
        ranges: selected_instructions::LiveRangeIdentity::from_bytes([6; 32]),
        legality: register_homes::AllocationLegalityIdentity::from_bytes([7; 32]),
        homes: selected_instructions_to_register_homes::RegisterHomeIdentity::from_bytes([8; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([9; 32]),
        target: selected.target,
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([10; 32]),
        physical_register_model: physical.identity(),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([11; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([12; 32]),
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions: vec![PostAllocationMachineFunction {
            machine,
            blocks: vec![PostAllocationMachineBlock {
                block,
                instructions: vec![compare_machine, branch_machine],
            }],
        }],
        structural_unit_functions: Vec::new(),
    };
    Fixture {
        selected,
        selected_identity,
        liveness,
        source,
        physical,
    }
}

pub(super) fn two_pair_fixture() -> Fixture {
    let mut fixture = fixture();
    let second_machine = MachineId::new(20).unwrap();

    let mut selected = fixture.selected.functions[0].clone();
    selected.machine = second_machine;
    fixture.selected.functions.push(selected);

    let mut liveness = fixture.liveness.functions[0].clone();
    liveness.machine = second_machine;
    fixture.liveness.functions.push(liveness);

    let mut source = fixture.source.functions[0].clone();
    source.machine = second_machine;
    fixture.source.functions.push(source);
    fixture
}

fn successor(edge: u64, block: SelectedBlockId) -> SelectedSuccessor {
    SelectedSuccessor {
        psi_edge: EdgeId::new(edge).unwrap(),
        block,
        source_target: BlockId::new(1).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    }
}

fn alternative(
    family: MachineAlternativeFamily,
    encoded: MachineEncodedEffects,
) -> MachineAlternative {
    MachineAlternative {
        key: MachineAlternativeKey { family, variant: 0 },
        applicability: MachineAlternativeApplicability::Always,
        size: MachineSizeKnowledge::ExactBytes(4),
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded,
    }
}

fn physical_operand(view: &register_model::RegisterView) -> PhysicalOperandFootprint {
    PhysicalOperandFootprint {
        operand: 0,
        virtual_register: VirtualRegisterId(0),
        class: view.class,
        view: view.id,
        access: RegisterOperandAccess::Use,
        storage_units: view.units.clone(),
        read_units: view.units.clone(),
        write_units: Vec::new(),
        write_semantics: None,
    }
}

fn units(
    values: impl IntoIterator<Item = register_model::RegisterUnitId>,
) -> Vec<register_model::RegisterUnitId> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values
}
