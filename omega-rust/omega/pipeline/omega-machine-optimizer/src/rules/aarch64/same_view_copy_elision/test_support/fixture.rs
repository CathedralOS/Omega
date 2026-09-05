use omega_isa_aarch64::aarch64_physical_register_model;
use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterOperandAccess, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock,
    SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_selected_instructions_to_register_homes::{
    AllocationLegalityIdentity, BlockLiveness, FunctionLiveness, InstructionLiveness,
    LiveRangeIdentity, LivenessIdentity, LivenessPlan, LivenessPosition, RegisterHomeIdentity,
};
use omega_target::NativeTarget;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, PreAllocationMachineEffectIdentity,
};

use super::super::SameViewCopyInputs;

pub(crate) use super::{
    compare_i64_right_operand_fixture, two_pair_compare_i64_right_operand_fixture,
};

pub(crate) struct Fixture {
    pub selected: SelectedInstructionPlan,
    pub selected_identity: SelectedInstructionPlanIdentity,
    pub liveness: LivenessPlan,
    pub liveness_identity: LivenessIdentity,
    pub source: PostAllocationMachinePlan,
    pub source_identity: PostAllocationMachineIdentity,
    pub physical: ValidatedPhysicalRegisterModel,
}

impl Fixture {
    pub(crate) fn inputs(&self) -> SameViewCopyInputs<'_> {
        SameViewCopyInputs {
            selected: &self.selected,
            selected_identity: self.selected_identity,
            liveness: &self.liveness,
            liveness_identity: self.liveness_identity,
            source: &self.source,
            source_identity: self.source_identity,
            physical: &self.physical,
        }
    }
}

pub(crate) fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(2, 1, 1, 1, 2).unwrap()
}

fn selected_operand(
    operand: u16,
    register: u32,
    access: RegisterOperandAccess,
    class: omega_register_model::RegisterClassId,
    fixed_view: Option<omega_register_model::RegisterViewId>,
) -> SelectedOperand {
    SelectedOperand {
        operand,
        virtual_register: VirtualRegisterId(register),
        access,
        class,
        fixed_view,
        tied_to: None,
        early_clobber: false,
    }
}

fn physical_operand(
    operand: u16,
    register: u32,
    access: RegisterOperandAccess,
    view: &omega_register_model::RegisterView,
) -> PhysicalOperandFootprint {
    PhysicalOperandFootprint {
        operand,
        virtual_register: VirtualRegisterId(register),
        class: view.class,
        view: view.id,
        access,
        storage_units: view.units.clone(),
        read_units: matches!(access, RegisterOperandAccess::Use)
            .then(|| view.units.clone())
            .unwrap_or_default(),
        write_units: matches!(access, RegisterOperandAccess::Def)
            .then(|| view.write_units.clone())
            .unwrap_or_default(),
        write_semantics: matches!(access, RegisterOperandAccess::Def)
            .then_some(view.write_semantics),
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

pub(crate) fn fixture() -> Fixture {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x0 = physical.model().view_named("x0").unwrap();
    let x30 = physical.model().view_named("x30").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let machine = MachineId::new(1).unwrap();
    let block = SelectedBlockId(0);
    let source_block = BlockId::new(1).unwrap();
    let value = ValueId::new(1).unwrap();
    let copy_id = SelectedInstructionId(1);
    let return_id = SelectedInstructionId(2);
    let copy = SelectedInstruction {
        id: copy_id,
        kind: SelectedInstructionKind::CopyI64,
        constraint: super::constraint(),
        operands: vec![
            selected_operand(0, 1, RegisterOperandAccess::Use, x0.class, None),
            selected_operand(1, 2, RegisterOperandAccess::Def, x0.class, None),
        ],
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
        provenance: SelectedInstructionProvenance {
            values: vec![value],
            ..Default::default()
        },
    };
    let returned = SelectedInstruction {
        id: return_id,
        kind: SelectedInstructionKind::ReturnI64,
        constraint: super::constraint(),
        operands: vec![selected_operand(
            0,
            2,
            RegisterOperandAccess::Use,
            x0.class,
            Some(x0.id),
        )],
        implicit_uses: x30.units.clone(),
        implicit_defs: pc.units.clone(),
        clobbers: vec![],
        provenance: SelectedInstructionProvenance {
            values: vec![value],
            edges: vec![EdgeId::new(1).unwrap()],
            ..Default::default()
        },
    };
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
            virtual_registers: vec![],
            blocks: vec![SelectedBlock {
                id: block,
                source_block,
                instructions: vec![copy],
                terminator: SelectedTerminator::Return {
                    instruction: returned,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }],
        structural_unit_functions: vec![],
        projected_structural_call_returns: vec![],
    };
    let selected_identity = SelectedInstructionPlanIdentity::from_bytes([2; 32]);
    let through = super::sorted_units(x0.units.iter().chain(&x30.units).copied());
    let liveness = LivenessPlan {
        selected: selected_identity,
        optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"same-view-copy"),
        fuel_schedule: selected.fuel_schedule,
        target: selected.target,
        functions: vec![FunctionLiveness {
            machine,
            entry_definitions: vec![],
            operand_positions: vec![],
            blocks: vec![BlockLiveness {
                block,
                source_block,
                virtual_live_in: vec![],
                virtual_live_out: vec![],
                unit_live_in: through.clone(),
                unit_live_out: vec![],
                instructions: vec![
                    InstructionLiveness {
                        position: LivenessPosition(0),
                        instruction: copy_id,
                        virtual_uses: vec![VirtualRegisterId(1)],
                        virtual_defs: vec![VirtualRegisterId(2)],
                        virtual_live_in: vec![VirtualRegisterId(1)],
                        virtual_live_out: vec![VirtualRegisterId(2)],
                        unit_uses: x0.units.clone(),
                        unit_defs: x0.write_units.clone(),
                        unit_clobbers: vec![],
                        unit_live_in: through.clone(),
                        unit_live_out: through.clone(),
                    },
                    InstructionLiveness {
                        position: LivenessPosition(1),
                        instruction: return_id,
                        virtual_uses: vec![VirtualRegisterId(2)],
                        virtual_defs: vec![],
                        virtual_live_in: vec![VirtualRegisterId(2)],
                        virtual_live_out: vec![],
                        unit_uses: through.clone(),
                        unit_defs: pc.write_units.clone(),
                        unit_clobbers: vec![],
                        unit_live_in: through.clone(),
                        unit_live_out: vec![],
                    },
                ],
                successors: vec![],
            }],
        }],
        structural_unit_functions: vec![],
    };
    let copy_machine = PostAllocationMachineInstruction {
        instruction: copy_id,
        alternative: alternative(
            MachineAlternativeFamily::CopyI64,
            MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]),
        ),
        operands: vec![
            physical_operand(0, 1, RegisterOperandAccess::Use, x0),
            physical_operand(1, 2, RegisterOperandAccess::Def, x0),
        ],
        implicit_unit_uses: vec![],
        implicit_unit_defs: vec![],
        implicit_unit_clobbers: vec![],
        unit_uses: x0.units.clone(),
        unit_defs: x0.write_units.clone(),
        unit_clobbers: vec![],
    };
    let return_machine = PostAllocationMachineInstruction {
        instruction: return_id,
        alternative: alternative(
            MachineAlternativeFamily::ReturnI64,
            MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: x30.units.clone(),
                implicit_unit_defs: pc.units.clone(),
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id },
            },
        ),
        operands: vec![physical_operand(0, 2, RegisterOperandAccess::Use, x0)],
        implicit_unit_uses: x30.units.clone(),
        implicit_unit_defs: pc.units.clone(),
        implicit_unit_clobbers: vec![],
        unit_uses: through,
        unit_defs: pc.write_units.clone(),
        unit_clobbers: vec![],
    };
    let source_identity = PostAllocationMachineIdentity::from_bytes([4; 32]);
    let source = PostAllocationMachinePlan {
        identity: source_identity,
        selected: selected_identity,
        effects: PreAllocationMachineEffectIdentity::from_bytes([5; 32]),
        ranges: LiveRangeIdentity::from_bytes([6; 32]),
        legality: AllocationLegalityIdentity::from_bytes([7; 32]),
        homes: RegisterHomeIdentity::from_bytes([8; 32]),
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
                instructions: vec![copy_machine, return_machine],
            }],
        }],
        structural_unit_functions: vec![],
    };
    Fixture {
        selected,
        selected_identity,
        liveness,
        liveness_identity: LivenessIdentity::from_bytes([3; 32]),
        source,
        source_identity,
        physical,
    }
}

pub(crate) fn two_pair_fixture() -> Fixture {
    let mut fixture = fixture();
    let second_machine = MachineId::new(2).unwrap();

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

pub(crate) fn compare_fixture() -> Fixture {
    let mut fixture = fixture();
    let x0 = fixture.physical.model().view_named("x0").unwrap().clone();
    let x30 = fixture.physical.model().view_named("x30").unwrap().clone();
    let pc = fixture.physical.model().view_named("pc").unwrap().clone();
    let nzcv = fixture.physical.model().view_named("nzcv").unwrap().clone();
    let value = ValueId::new(1).unwrap();
    let return_id = SelectedInstructionId(3);

    let block = &mut fixture.selected.functions[0].blocks[0];
    let SelectedTerminator::Return {
        instruction: mut compare,
        psi_return_edge,
    } = block.terminator.clone()
    else {
        unreachable!()
    };
    compare.kind = SelectedInstructionKind::CompareI64Zero;
    compare.operands[0].fixed_view = None;
    compare.implicit_uses.clear();
    compare.implicit_defs = nzcv.units.clone();
    compare.provenance.edges.clear();
    block.instructions.push(compare);
    block.terminator = SelectedTerminator::Return {
        instruction: SelectedInstruction {
            id: return_id,
            kind: SelectedInstructionKind::ReturnUnit,
            constraint: super::constraint(),
            operands: vec![],
            implicit_uses: x30.units.clone(),
            implicit_defs: pc.units.clone(),
            clobbers: vec![],
            provenance: SelectedInstructionProvenance {
                edges: vec![psi_return_edge],
                ..Default::default()
            },
        },
        psi_return_edge,
    };

    let through = super::sorted_units(x0.units.iter().chain(&x30.units).copied());
    let live_block = &mut fixture.liveness.functions[0].blocks[0];
    let compare_live = &mut live_block.instructions[1];
    compare_live.virtual_uses = vec![VirtualRegisterId(2)];
    compare_live.virtual_live_in = vec![VirtualRegisterId(2)];
    compare_live.virtual_live_out.clear();
    compare_live.unit_uses = x0.units.clone();
    compare_live.unit_defs = nzcv.write_units.clone();
    compare_live.unit_live_in = through;
    compare_live.unit_live_out = x30.units.clone();
    live_block.instructions.push(InstructionLiveness {
        position: LivenessPosition(2),
        instruction: return_id,
        virtual_uses: vec![],
        virtual_defs: vec![],
        virtual_live_in: vec![],
        virtual_live_out: vec![],
        unit_uses: x30.units.clone(),
        unit_defs: pc.write_units.clone(),
        unit_clobbers: vec![],
        unit_live_in: x30.units.clone(),
        unit_live_out: vec![],
    });

    let machine_block = &mut fixture.source.functions[0].blocks[0];
    let compare_machine = &mut machine_block.instructions[1];
    compare_machine.alternative = alternative(
        MachineAlternativeFamily::CompareI64Zero,
        MachineEncodedEffects {
            external_operand_reads: vec![0],
            external_operand_writes: vec![],
            implicit_unit_uses: vec![],
            implicit_unit_defs: nzcv.units.clone(),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: MachineEncodedControlEffect::FallThroughV1,
        },
    );
    compare_machine.operands = vec![physical_operand(0, 2, RegisterOperandAccess::Use, &x0)];
    compare_machine.implicit_unit_uses.clear();
    compare_machine.implicit_unit_defs = nzcv.units.clone();
    compare_machine.unit_uses = x0.units.clone();
    compare_machine.unit_defs = nzcv.write_units.clone();
    machine_block
        .instructions
        .push(PostAllocationMachineInstruction {
            instruction: return_id,
            alternative: alternative(
                MachineAlternativeFamily::ReturnUnit,
                MachineEncodedEffects {
                    external_operand_reads: vec![],
                    external_operand_writes: vec![],
                    implicit_unit_uses: x30.units.clone(),
                    implicit_unit_defs: pc.units.clone(),
                    implicit_unit_clobbers: vec![],
                    memory: MachineEncodedMemoryEffect::NoneV1,
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: x30.id,
                    },
                },
            ),
            operands: vec![],
            implicit_unit_uses: x30.units.clone(),
            implicit_unit_defs: pc.units.clone(),
            implicit_unit_clobbers: vec![],
            unit_uses: x30.units.clone(),
            unit_defs: pc.write_units.clone(),
            unit_clobbers: vec![],
        });
    fixture.selected.functions[0].blocks[0].instructions[1]
        .provenance
        .values = vec![value];
    fixture
}

pub(crate) fn two_pair_compare_fixture() -> Fixture {
    let mut fixture = compare_fixture();
    let second_machine = MachineId::new(2).unwrap();

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

pub(crate) fn compare_i64_left_operand_fixture() -> Fixture {
    let mut fixture = compare_fixture();
    let x0 = fixture.physical.model().view_named("x0").unwrap().clone();
    let x1 = fixture.physical.model().view_named("x1").unwrap().clone();
    let x30 = fixture.physical.model().view_named("x30").unwrap().clone();
    let nzcv = fixture.physical.model().view_named("nzcv").unwrap().clone();

    let compare = &mut fixture.selected.functions[0].blocks[0].instructions[1];
    compare.kind = SelectedInstructionKind::CompareI64;
    compare.operands.push(selected_operand(
        1,
        3,
        RegisterOperandAccess::Use,
        x1.class,
        None,
    ));
    compare.provenance.values = vec![
        ValueId::new(1).unwrap(),
        ValueId::new(2).unwrap(),
        ValueId::new(3).unwrap(),
    ];
    fixture.selected.functions[0].virtual_registers = vec![
        VirtualRegister {
            id: VirtualRegisterId(1),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: x0.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: ValueDefinitionSite::FunctionParameter(0),
            entry_fixed_view: Some(x0.id),
        },
        VirtualRegister {
            id: VirtualRegisterId(2),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: x0.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(1),
                source_value: ValueId::new(1).unwrap(),
            },
            definition_site: ValueDefinitionSite::Node {
                block: fixture.selected.functions[0].blocks[0].source_block,
                node: 0,
            },
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: VirtualRegisterId(3),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: x1.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(2).unwrap(),
                parameter_index: 1,
            },
            definition_site: ValueDefinitionSite::FunctionParameter(1),
            entry_fixed_view: Some(x1.id),
        },
    ];

    let through = super::sorted_units(x0.units.iter().chain(&x1.units).chain(&x30.units).copied());
    let live_block = &mut fixture.liveness.functions[0].blocks[0];
    live_block.instructions[0].virtual_live_in = vec![VirtualRegisterId(1), VirtualRegisterId(3)];
    live_block.instructions[0].virtual_live_out = vec![VirtualRegisterId(2), VirtualRegisterId(3)];
    live_block.instructions[0].unit_live_in = through.clone();
    live_block.instructions[0].unit_live_out = through.clone();
    live_block.instructions[1].virtual_uses = vec![VirtualRegisterId(2), VirtualRegisterId(3)];
    live_block.instructions[1].virtual_live_in = vec![VirtualRegisterId(2), VirtualRegisterId(3)];
    live_block.instructions[1].unit_uses =
        super::sorted_units(x0.units.iter().chain(&x1.units).copied());
    live_block.instructions[1].unit_live_in = through;
    live_block.instructions[1].unit_defs = nzcv.write_units.clone();

    let machine_compare = &mut fixture.source.functions[0].blocks[0].instructions[1];
    machine_compare.alternative = alternative(
        MachineAlternativeFamily::CompareI64,
        MachineEncodedEffects {
            external_operand_reads: vec![0, 1],
            external_operand_writes: vec![],
            implicit_unit_uses: vec![],
            implicit_unit_defs: nzcv.units.clone(),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: MachineEncodedControlEffect::FallThroughV1,
        },
    );
    machine_compare
        .operands
        .push(physical_operand(1, 3, RegisterOperandAccess::Use, &x1));
    machine_compare.unit_uses = super::sorted_units(x0.units.iter().chain(&x1.units).copied());
    machine_compare.unit_defs = nzcv.write_units;
    fixture
}

pub(crate) fn two_pair_compare_i64_left_operand_fixture() -> Fixture {
    let mut fixture = compare_i64_left_operand_fixture();
    let second_machine = MachineId::new(2).unwrap();

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
