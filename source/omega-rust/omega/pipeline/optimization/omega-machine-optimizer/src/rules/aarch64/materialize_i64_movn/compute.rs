use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstruction,
    SelectedInstructionKind, SelectedInstructionPlan,
};
use omega_target::Architecture;
use psi_core::IntegerValue;

use crate::{
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationAction,
    Aarch64MovnMaterializationAttempt, Aarch64MovnMaterializationAttemptOutcome,
    Aarch64MovnMaterializationBlock, Aarch64MovnMaterializationError,
    Aarch64MovnMaterializationFunction, Aarch64MovnMaterializationIdentity,
    Aarch64MovnMaterializationInstruction, Aarch64MovnMaterializationPlan,
    Aarch64MovnMaterializationPolicy, Aarch64MovnMaterializationWorkAxis, Aarch64MovnPatch,
    Aarch64MovnRecipe, PhysicalOperandFootprint, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, QualifiedPhysicalWrite, ValidatedPostAllocationMachinePlan,
    aarch64_movn_materialization_identity,
};

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationError> {
    compute_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        budget,
    )
}

pub(crate) fn compute_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationError> {
    validate_roots(
        selected,
        selected_identity,
        source,
        source_identity,
        physical,
    )?;
    let mut functions = baseline_roster(source);
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();

    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            Aarch64MovnMaterializationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        let mut candidate = None;
        'scan: for (function_index, selected_function) in selected.functions.iter().enumerate() {
            let machine_function = source.functions.get(function_index).ok_or(
                Aarch64MovnMaterializationError::FunctionRosterMismatch(function_index),
            )?;
            if machine_function.machine != selected_function.machine {
                return Err(Aarch64MovnMaterializationError::FunctionRosterMismatch(
                    function_index,
                ));
            }
            for (block_index, block) in selected_function.blocks.iter().enumerate() {
                let machine_block = machine_function.blocks.get(block_index).ok_or(
                    Aarch64MovnMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    },
                )?;
                if machine_block.block != block.id
                    || machine_block.instructions.len() != block.instructions.len() + 1
                {
                    return Err(Aarch64MovnMaterializationError::BlockRosterMismatch {
                        function: function_index,
                        block: block_index,
                    });
                }
                for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                    let SelectedInstructionKind::MaterializeI64 { value } = instruction.kind else {
                        continue;
                    };
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        Aarch64MovnMaterializationWorkAxis::RuleEvaluations,
                    )?;
                    let machine = machine_block.instructions.get(instruction_index).ok_or(
                        Aarch64MovnMaterializationError::InstructionRosterMismatch(instruction.id),
                    )?;
                    let literal_bits = integer_bits(value, instruction.id)?;
                    validate_materialization(instruction, machine, physical)?;
                    let destination = qualified_write(instruction, machine)?;
                    let baseline_word_count = zero_seed_word_count(literal_bits);
                    let recipe = movn_recipe(literal_bits);
                    let already_selected =
                        actions
                            .iter()
                            .any(|action: &Aarch64MovnMaterializationAction| {
                                action.machine == selected_function.machine
                                    && action.block == block.id
                                    && action.instruction == instruction.id
                            });
                    let candidate_words = recipe
                        .word_count()
                        .ok_or(Aarch64MovnMaterializationError::CountOverflow)?;
                    let outcome = if already_selected {
                        Aarch64MovnMaterializationAttemptOutcome::AlreadySelected
                    } else if candidate_words >= baseline_word_count {
                        Aarch64MovnMaterializationAttemptOutcome::BaselineNotLonger
                    } else {
                        Aarch64MovnMaterializationAttemptOutcome::SelectedForRewrite
                    };
                    attempts.push(Aarch64MovnMaterializationAttempt {
                        iteration,
                        input,
                        machine: selected_function.machine,
                        block: block.id,
                        instruction: instruction.id,
                        literal_bits,
                        destination: destination.clone(),
                        baseline_word_count,
                        recipe: recipe.clone(),
                        outcome,
                    });
                    if outcome == Aarch64MovnMaterializationAttemptOutcome::SelectedForRewrite {
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            Aarch64MovnMaterializationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            Aarch64MovnMaterializationWorkAxis::ValidationSteps,
                        )?;
                        candidate = Some((
                            function_index,
                            block_index,
                            selected_function.machine,
                            block.id,
                            instruction.id,
                            literal_bits,
                            destination,
                            baseline_word_count,
                            recipe,
                        ));
                        break 'scan;
                    }
                }
            }
        }

        let Some((
            function_index,
            block_index,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            baseline_word_count,
            recipe,
        )) = candidate
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            Aarch64MovnMaterializationWorkAxis::Commits,
        )?;
        let row = functions[function_index].blocks[block_index]
            .instructions
            .iter_mut()
            .find(|row| row.instruction == instruction)
            .ok_or(Aarch64MovnMaterializationError::InstructionRosterMismatch(
                instruction,
            ))?;
        row.disposition = Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
            literal_bits,
            destination: destination.clone(),
            baseline_word_count,
            recipe: recipe.clone(),
        };
        let output = super::identity::revision_identity(
            source_identity,
            selected_identity,
            source.target,
            physical.identity(),
            &functions,
        );
        actions.push(Aarch64MovnMaterializationAction {
            iteration,
            input,
            output,
            machine,
            block,
            instruction,
            literal_bits,
            destination,
            baseline_word_count,
            recipe,
        });
    }

    let output_revision = super::identity::revision_identity(
        source_identity,
        selected_identity,
        source.target,
        physical.identity(),
        &functions,
    );
    let mut plan = Aarch64MovnMaterializationPlan {
        identity: Aarch64MovnMaterializationIdentity::from_bytes([0; 32]),
        source: source_identity,
        selected: selected_identity,
        target: source.target,
        physical_register_model: physical.identity(),
        policy:
            Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    plan.identity = aarch64_movn_materialization_identity(&plan);
    Ok(plan)
}

fn validate_roots(
    selected: &SelectedInstructionPlan,
    selected_identity: omega_selected_instructions::SelectedInstructionPlanIdentity,
    source: &PostAllocationMachinePlan,
    source_identity: crate::PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64MovnMaterializationError> {
    if selected.target.architecture != Architecture::Aarch64
        || source.target.architecture != Architecture::Aarch64
        || physical.model().architecture != Architecture::Aarch64
    {
        return Err(Aarch64MovnMaterializationError::UnsupportedTarget(
            source.target,
        ));
    }
    if source.identity != source_identity
        || source.selected != selected_identity
        || selected.target != source.target
        || source.physical_register_model != physical.identity()
        || selected.functions.len() != source.functions.len()
    {
        return Err(Aarch64MovnMaterializationError::RootMismatch);
    }
    Ok(())
}

fn baseline_roster(source: &PostAllocationMachinePlan) -> Vec<Aarch64MovnMaterializationFunction> {
    source
        .functions
        .iter()
        .map(|function| Aarch64MovnMaterializationFunction {
            machine: function.machine,
            blocks: function
                .blocks
                .iter()
                .map(|block| Aarch64MovnMaterializationBlock {
                    block: block.block,
                    instructions: block
                        .instructions
                        .iter()
                        .map(|instruction| Aarch64MovnMaterializationInstruction {
                            instruction: instruction.instruction,
                            disposition: Aarch64MovnInstructionDisposition::RetainedV1,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

fn integer_bits(
    value: IntegerValue,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<u64, Aarch64MovnMaterializationError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| Aarch64MovnMaterializationError::IntegerOutsideI64Bits(instruction)),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| Aarch64MovnMaterializationError::IntegerOutsideI64Bits(instruction)),
    }
}

fn validate_materialization(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), Aarch64MovnMaterializationError> {
    let encoded = &machine.alternative.encoded;
    if machine.instruction != selected.id
        || selected.operands.len() != 1
        || !selected.implicit_uses.is_empty()
        || !selected.implicit_defs.is_empty()
        || !selected.clobbers.is_empty()
        || machine.alternative.key.family != MachineAlternativeFamily::MaterializeI64
        || machine.alternative.key.variant != 0
        || !encoded.external_operand_reads.is_empty()
        || encoded.external_operand_writes != [0]
        || !encoded.implicit_unit_uses.is_empty()
        || !encoded.implicit_unit_defs.is_empty()
        || !encoded.implicit_unit_clobbers.is_empty()
        || encoded.memory != MachineEncodedMemoryEffect::NoneV1
        || encoded.stack != MachineEncodedStackEffect::UnchangedV1
        || encoded.trap != MachineEncodedTrapBehavior::NeverV1
        || encoded.control != MachineEncodedControlEffect::FallThroughV1
        || machine.operands.len() != 1
        || !machine.implicit_unit_uses.is_empty()
        || !machine.implicit_unit_defs.is_empty()
        || !machine.implicit_unit_clobbers.is_empty()
        || !machine.unit_uses.is_empty()
        || !machine.unit_clobbers.is_empty()
    {
        return Err(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    let selected_operand = &selected.operands[0];
    let operand = &machine.operands[0];
    if selected_operand.operand != 0
        || selected_operand.access != RegisterOperandAccess::Def
        || operand.operand != 0
        || operand.virtual_register != selected_operand.virtual_register
        || operand.class != selected_operand.class
        || operand.access != RegisterOperandAccess::Def
        || !operand.read_units.is_empty()
        || machine.unit_defs != operand.write_units
        || operand.write_semantics.is_none()
    {
        return Err(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id));
    }
    validate_x_view(operand, physical, selected.id)
}

fn validate_x_view(
    operand: &PhysicalOperandFootprint,
    physical: &ValidatedPhysicalRegisterModel,
    instruction: omega_selected_instructions::SelectedInstructionId,
) -> Result<(), Aarch64MovnMaterializationError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == operand.view)
        .ok_or(Aarch64MovnMaterializationError::InvalidPhysicalDestination(
            instruction,
        ))?;
    let valid_index = view
        .name
        .strip_prefix('x')
        .and_then(|name| name.parse::<u8>().ok())
        .is_some_and(|index| index <= 30);
    if !valid_index
        || view.bits != 64
        || !view.allocatable
        || view.class != operand.class
        || view.units != operand.storage_units
        || view.write_units != operand.write_units
        || Some(view.write_semantics) != operand.write_semantics
    {
        return Err(Aarch64MovnMaterializationError::InvalidPhysicalDestination(
            instruction,
        ));
    }
    Ok(())
}

fn qualified_write(
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
) -> Result<QualifiedPhysicalWrite, Aarch64MovnMaterializationError> {
    let operand = machine
        .operands
        .first()
        .ok_or(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id))?;
    Ok(QualifiedPhysicalWrite {
        instruction: selected.id,
        operand: operand.operand,
        virtual_register: operand.virtual_register,
        class: operand.class,
        view: operand.view,
        storage_units: operand.storage_units.clone(),
        write_units: operand.write_units.clone(),
        write_semantics: operand
            .write_semantics
            .ok_or(Aarch64MovnMaterializationError::InvalidMaterializationFootprint(selected.id))?,
    })
}

pub(crate) fn zero_seed_word_count(bits: u64) -> u8 {
    1 + (1..4)
        .filter(|halfword| ((bits >> (halfword * 16)) & 0xffff) != 0)
        .count() as u8
}

pub(crate) fn movn_recipe(bits: u64) -> Aarch64MovnRecipe {
    let chunks = [
        bits as u16,
        (bits >> 16) as u16,
        (bits >> 32) as u16,
        (bits >> 48) as u16,
    ];
    let seed_halfword = chunks
        .iter()
        .position(|chunk| *chunk != u16::MAX)
        .unwrap_or(0) as u8;
    let patches = chunks
        .iter()
        .enumerate()
        .filter(|(halfword, chunk)| *halfword != usize::from(seed_halfword) && **chunk != u16::MAX)
        .map(|(halfword, immediate)| Aarch64MovnPatch {
            halfword: halfword as u8,
            immediate: *immediate,
        })
        .collect();
    Aarch64MovnRecipe {
        seed_halfword,
        seed_immediate: !chunks[usize::from(seed_halfword)],
        patches,
    }
}

fn charge(
    usage: &mut u64,
    budget: u64,
    axis: Aarch64MovnMaterializationWorkAxis,
) -> Result<(), Aarch64MovnMaterializationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(Aarch64MovnMaterializationError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(Aarch64MovnMaterializationError::BudgetExceeded(axis));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{
        OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
    };
    use omega_register_model::{
        PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
        RegisterOperandAccess, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
        RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
        ValidatedPhysicalRegisterModel, validate_physical_register_model,
    };
    use omega_selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedEffects,
        MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock, SelectedBlockId,
        SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
        SelectedOperand, SelectedTerminator, VirtualRegisterId,
    };
    use omega_target::{Architecture, NativeTarget};
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{movn_recipe, zero_seed_word_count};
    use crate::{
        Aarch64MovnMaterializationAttemptOutcome, Aarch64MovnMaterializationError,
        Aarch64MovnMaterializationWorkAxis, Aarch64MovnPatch, MachineAlternativeChoiceRule,
        PhysicalOperandFootprint, PostAllocationMachineBlock, PostAllocationMachineFunction,
        PostAllocationMachineIdentity, PostAllocationMachineInstruction, PostAllocationMachinePlan,
        PreAllocationMachineEffectIdentity,
    };

    #[test]
    fn movn_recipe_uses_lowest_eligible_seed_and_ascending_patches() {
        let recipe = movn_recipe(0xffff_1234_ffff_abcd);
        assert_eq!(recipe.seed_halfword, 0);
        assert_eq!(recipe.seed_immediate, !0xabcd);
        assert_eq!(
            recipe.patches,
            vec![Aarch64MovnPatch {
                halfword: 2,
                immediate: 0x1234,
            }]
        );
        assert_eq!(recipe.word_count(), Some(2));
    }

    #[test]
    fn strict_word_count_policy_only_prefers_real_shrinks() {
        assert_eq!(zero_seed_word_count(0), 1);
        assert_eq!(movn_recipe(0).word_count(), Some(4));
        assert_eq!(zero_seed_word_count(u64::MAX), 4);
        assert_eq!(movn_recipe(u64::MAX).word_count(), Some(1));
        assert_eq!(zero_seed_word_count(0xffff_0000_0000_0001), 2);
        assert_eq!(movn_recipe(0xffff_0000_0000_0001).word_count(), Some(3));
    }

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(PhysicalRegisterModel {
            architecture: Architecture::Aarch64,
            units: vec![RegisterUnit {
                id: RegisterUnitId(0),
                name: "x0.storage".into(),
                bits: 64,
                kind: RegisterUnitKind::IntegerLane,
            }],
            views: vec![RegisterView {
                id: RegisterViewId(0),
                name: "x0".into(),
                class: RegisterClassId(0),
                units: vec![RegisterUnitId(0)],
                write_units: vec![RegisterUnitId(0)],
                bits: 64,
                write_semantics: RegisterWriteSemantics::ExactView,
                allocatable: true,
            }],
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "gpr64".into(),
                views: vec![RegisterViewId(0)],
            }],
            conventions: vec![PreservationConvention {
                name: "test".into(),
                argument_views: vec![RegisterViewId(0)],
                result_views: vec![RegisterViewId(0)],
                caller_saved: vec![RegisterUnitId(0)],
                callee_saved: vec![],
                fixed: vec![],
                stack_alignment: 16,
                red_zone_bytes: 0,
            }],
            reservations: vec![],
        })
        .unwrap()
    }

    fn constraint() -> RegisterConstraintKey {
        RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 0,
        }
    }

    fn selected_instruction(id: u32, value: u64) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(id),
            kind: SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(value.into()),
            },
            constraint: constraint(),
            operands: vec![SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(id),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: vec![],
            implicit_defs: vec![],
            clobbers: vec![],
            provenance: SelectedInstructionProvenance::default(),
        }
    }

    fn machine_instruction(id: u32) -> PostAllocationMachineInstruction {
        PostAllocationMachineInstruction {
            instruction: SelectedInstructionId(id),
            alternative: MachineAlternative {
                key: MachineAlternativeKey {
                    family: MachineAlternativeFamily::MaterializeI64,
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: 4,
                    maximum_bytes: Some(16),
                },
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
            },
            operands: vec![PhysicalOperandFootprint {
                operand: 0,
                virtual_register: VirtualRegisterId(id),
                class: RegisterClassId(0),
                view: RegisterViewId(0),
                access: RegisterOperandAccess::Def,
                storage_units: vec![RegisterUnitId(0)],
                read_units: vec![],
                write_units: vec![RegisterUnitId(0)],
                write_semantics: Some(RegisterWriteSemantics::ExactView),
            }],
            implicit_unit_uses: vec![],
            implicit_unit_defs: vec![],
            implicit_unit_clobbers: vec![],
            unit_uses: vec![],
            unit_defs: vec![RegisterUnitId(0)],
            unit_clobbers: vec![],
        }
    }

    fn fixture() -> (
        SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
        PostAllocationMachinePlan,
        PostAllocationMachineIdentity,
        ValidatedPhysicalRegisterModel,
    ) {
        let machine = MachineId::new(1).unwrap();
        let block = SelectedBlockId(0);
        let return_instruction = SelectedInstruction {
            id: SelectedInstructionId(4),
            kind: SelectedInstructionKind::ReturnUnit,
            constraint: constraint(),
            operands: vec![],
            implicit_uses: vec![],
            implicit_defs: vec![],
            clobbers: vec![],
            provenance: SelectedInstructionProvenance::default(),
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
                    source_block: BlockId::new(1).unwrap(),
                    instructions: vec![
                        selected_instruction(1, u64::MAX),
                        selected_instruction(2, 0xffff_1234_ffff_abcd),
                        selected_instruction(3, 7),
                    ],
                    terminator: SelectedTerminator::Return {
                        instruction: return_instruction,
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }],
            }],
            structural_unit_functions: vec![],
        };
        let selected_identity = SelectedInstructionPlanIdentity::from_bytes([2; 32]);
        let physical = physical();
        let source_identity = PostAllocationMachineIdentity::from_bytes([3; 32]);
        let return_machine = PostAllocationMachineInstruction {
            instruction: SelectedInstructionId(4),
            alternative: MachineAlternative {
                key: MachineAlternativeKey {
                    family: MachineAlternativeFamily::ReturnUnit,
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(4),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![]),
            },
            operands: vec![],
            implicit_unit_uses: vec![],
            implicit_unit_defs: vec![],
            implicit_unit_clobbers: vec![],
            unit_uses: vec![],
            unit_defs: vec![],
            unit_clobbers: vec![],
        };
        let source = PostAllocationMachinePlan {
            identity: source_identity,
            selected: selected_identity,
            effects: PreAllocationMachineEffectIdentity::from_bytes([4; 32]),
            ranges: omega_regalloc::LiveRangeIdentity::from_bytes([5; 32]),
            legality: omega_regalloc::AllocationLegalityIdentity::from_bytes([6; 32]),
            homes: omega_regalloc::RegisterHomeIdentity::from_bytes([7; 32]),
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
                [8; 32],
            ),
            target: NativeTarget::linux_arm64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([9; 32]),
            physical_register_model: physical.identity(),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([10; 32]),
            machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([11; 32]),
            choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
            functions: vec![PostAllocationMachineFunction {
                machine,
                blocks: vec![PostAllocationMachineBlock {
                    block,
                    instructions: vec![
                        machine_instruction(1),
                        machine_instruction(2),
                        machine_instruction(3),
                        return_machine,
                    ],
                }],
            }],
            structural_unit_functions: vec![],
        };
        (
            selected,
            selected_identity,
            source,
            source_identity,
            physical,
        )
    }

    #[test]
    fn compute_and_independent_replay_bind_every_action_and_retention() {
        let (selected, selected_identity, source, source_identity, physical) = fixture();
        let budget = OptimizationWorkBudget::new(20, 20, 20, 20, 20).unwrap();
        let computed = super::compute_from_parts(
            &selected,
            selected_identity,
            &source,
            source_identity,
            &physical,
            budget,
        )
        .unwrap();
        let replayed = crate::rules::aarch64::materialize_i64_movn::validate::replay_from_parts(
            &selected,
            selected_identity,
            &source,
            source_identity,
            &physical,
            budget,
        )
        .unwrap();
        assert_eq!(computed, replayed);
        assert_eq!(computed.actions.len(), 2);
        assert_eq!(computed.attempts.len(), 6);
        assert_eq!(computed.usage.iterations, 3);
        assert_eq!(computed.usage.rule_evaluations, 6);
        assert_eq!(computed.actions[0].literal_bits, u64::MAX);
        assert_eq!(computed.actions[0].baseline_word_count, 4);
        assert_eq!(computed.actions[0].recipe.word_count(), Some(1));
        assert_eq!(computed.actions[1].recipe.word_count(), Some(2));
        assert_eq!(
            computed.attempts.last().unwrap().outcome,
            Aarch64MovnMaterializationAttemptOutcome::BaselineNotLonger
        );

        let mut corrupted = computed.clone();
        corrupted.actions[0].literal_bits ^= 1;
        assert_ne!(corrupted, replayed);
    }

    #[test]
    fn compute_charges_the_exact_bounded_scan() {
        let (selected, selected_identity, source, source_identity, physical) = fixture();
        let budget = OptimizationWorkBudget::new(1, 20, 20, 20, 20).unwrap();
        assert_eq!(
            super::compute_from_parts(
                &selected,
                selected_identity,
                &source,
                source_identity,
                &physical,
                budget,
            ),
            Err(Aarch64MovnMaterializationError::BudgetExceeded(
                Aarch64MovnMaterializationWorkAxis::RuleEvaluations
            ))
        );
    }
}
