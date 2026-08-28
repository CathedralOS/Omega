use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_pressure_rematerialization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<PressureRematerializationPlan, PressureRematerializationError> {
    validate_roots(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    let materialize = materialize_row(constraints, selected_keys)?;
    let (functions, transformed) = build_functions(
        selected.selected_plan(),
        ranges.plan(),
        recovery.plan(),
        materialize,
        policy,
    )?;
    let applied = functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    if applied == 0 {
        return Err(PressureRematerializationError::NoAction);
    }
    let rewritten_uses = functions
        .iter()
        .filter_map(|function| function.action.as_ref())
        .try_fold(0usize, |total, action| {
            total.checked_add(action.rewrites.len())
        })
        .ok_or(PressureRematerializationError::WorkOverflow)?;
    let usage = required_usage(selected.selected_plan(), applied, rewritten_uses)?;
    ensure_budget(usage, budget)?;
    Ok(PressureRematerializationPlan {
        source_selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        recovery_classifications: recovery.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
        transformed_selected: selected_instruction_plan_identity(&transformed),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), PressureRematerializationError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || legality.receipt().allocator_availability() != availability.receipt().identity()
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical.identity()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != selected.selected_plan().target
        || target_register_environment_identity(
            selected.selected_plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment() != register_environment
        || spill_choices.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().selected() != selected.selected_identity()
        || recovery.receipt().ranges() != ranges.receipt().identity()
        || recovery.receipt().legality() != legality.receipt().identity()
        || recovery.receipt().spill_choices() != spill_choices.receipt().identity()
        || recovery.receipt().register_environment() != register_environment
        || recovery.receipt().allocator_availability() != availability.receipt().identity()
        || recovery.receipt().optimization_unit() != selected.optimization_unit_identity()
        || recovery.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || selected.selected_plan().functions.len() != recovery.plan().functions.len()
        || selected.selected_plan().functions.len() != ranges.plan().functions.len()
    {
        return Err(PressureRematerializationError::RootMismatch);
    }
    Ok(())
}

fn materialize_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, PressureRematerializationError> {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == keys.materialize_i64)
        .ok_or(PressureRematerializationError::MaterializeConstraintMismatch)?;
    let [result] = row.operands.as_slice() else {
        return Err(PressureRematerializationError::MaterializeConstraintMismatch);
    };
    if result.operand != 0
        || result.access != RegisterOperandAccess::Def
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(PressureRematerializationError::MaterializeConstraintMismatch);
    }
    Ok(row)
}

pub(crate) fn build_functions(
    selected: &SelectedInstructionPlan,
    ranges: &LiveRangePlan,
    recovery: &RecoveryClassificationPlan,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<
    (
        Vec<FunctionPressureRematerialization>,
        SelectedInstructionPlan,
    ),
    PressureRematerializationError,
> {
    let mut transformed = selected.clone();
    let mut functions = Vec::with_capacity(transformed.functions.len());
    for index in 0..transformed.functions.len() {
        let source = &selected.functions[index];
        let range_function = ranges
            .functions
            .get(index)
            .ok_or(PressureRematerializationError::FunctionMismatch { function: index })?;
        let recovery_function = recovery
            .functions
            .get(index)
            .ok_or(PressureRematerializationError::FunctionMismatch { function: index })?;
        if source.machine != range_function.machine || source.machine != recovery_function.machine {
            return Err(PressureRematerializationError::FunctionMismatch { function: index });
        }
        validate_dense(index, source)?;
        let action = match &recovery_function.classification {
            None => None,
            Some(candidate) => Some(action_from_candidate(
                index,
                source,
                range_function,
                candidate,
                row,
                policy,
            )?),
        };
        if let Some(action) = &action {
            apply_action(index, &mut transformed.functions[index], action, row)?;
        }
        functions.push(FunctionPressureRematerialization {
            machine: source.machine,
            action,
        });
    }
    Ok((functions, transformed))
}

fn action_from_candidate(
    function_index: usize,
    function: &SelectedFunction,
    ranges: &FunctionLiveRanges,
    candidate: &PressureRecoveryClassification,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<PressureRematerializationAction, PressureRematerializationError> {
    let RecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(PressureRematerializationError::UnsupportedVictimRole {
            function: function_index,
        });
    };
    let RecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(PressureRematerializationError::ClassificationNotAdmitted {
            function: function_index,
        });
    };
    let valid_arity = match policy {
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => future_uses.len() == 1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => future_uses.len() >= 2,
    };
    if !valid_arity
        || future_uses.windows(2).any(|pair| pair[0] >= pair[1])
        || future_uses
            .iter()
            .any(|future| future.block != candidate.block || future.point <= candidate.point)
    {
        return Err(PressureRematerializationError::FutureUseMismatch {
            function: function_index,
        });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if victim.scalar_type != candidate.scalar_type
        || victim.class != candidate.class
        || victim.origin != candidate.origin
        || victim.definition_site != candidate.definition_site
        || victim.entry_fixed_view.is_some()
        || row.operands[0].class != victim.class
    {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if original.kind != (SelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice() != [selected_operand(&row.operands[0], candidate.victim)]
    {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == candidate.victim)
        .ok_or(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        })?;
    if !range.occurrences.iter().any(|occurrence| {
        occurrence.instruction == *defining_instruction
            && occurrence.access == RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(PressureRematerializationError::MaterializeMismatch {
            function: function_index,
        });
    }
    for future in future_uses {
        let future_instruction = find_instruction(block, future.instruction).ok_or(
            PressureRematerializationError::FutureUseMismatch {
                function: function_index,
            },
        )?;
        let matching = future_instruction
            .operands
            .iter()
            .filter(|operand| operand.operand == future.operand)
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0].virtual_register != candidate.victim
            || matching[0].access != RegisterOperandAccess::Use
            || matching[0].fixed_view.is_some()
            || matching[0].class != candidate.class
        {
            return Err(PressureRematerializationError::FutureUseMismatch {
                function: function_index,
            });
        }
    }
    let fresh_instruction = SelectedInstructionId(instruction_count(function_index, function)?);
    let fresh_register = VirtualRegisterId(
        u32::try_from(function.virtual_registers.len()).map_err(|_| {
            PressureRematerializationError::IdentifierOverflow {
                function: function_index,
            }
        })?,
    );
    Ok(PressureRematerializationAction {
        block: candidate.block,
        pressure_point: candidate.point,
        victim: candidate.victim,
        current_view,
        reclaimed_view,
        original_materialize: *defining_instruction,
        source_value: *source_value,
        value: *value,
        rewrites: future_uses
            .iter()
            .map(|future| PressureRematerializationRewrite {
                point: future.point,
                instruction: future.instruction,
                operand: future.operand,
            })
            .collect(),
        fresh_materialize: fresh_instruction,
        result_virtual_register: fresh_register,
        materialize_constraint: row.key,
    })
}

fn apply_action(
    function_index: usize,
    function: &mut SelectedFunction,
    action: &PressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), PressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(PressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
    function.virtual_registers.push(VirtualRegister {
        id: action.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: action.fresh_materialize,
            source_value: action.source_value,
        },
        definition_site: source.definition_site,
        entry_fixed_view: None,
    });
    let new_instruction = SelectedInstruction {
        id: action.fresh_materialize,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: action.value,
        },
        constraint: action.materialize_constraint,
        operands: vec![selected_operand(
            &row.operands[0],
            action.result_virtual_register,
        )],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: vec![action.source_value],
            ..Default::default()
        },
    };
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(PressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
    for rewrite in &action.rewrites {
        rewrite_operand(
            function_index,
            block,
            action.victim,
            action.result_virtual_register,
            *rewrite,
        )?;
    }
    let first =
        action
            .rewrites
            .first()
            .ok_or(PressureRematerializationError::DecisionMismatch {
                function: function_index,
            })?;
    if let Some(index) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == first.instruction)
    {
        block.instructions.insert(index, new_instruction);
    } else {
        let terminator_id = match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction.id,
        };
        if terminator_id != first.instruction {
            return Err(PressureRematerializationError::DecisionMismatch {
                function: function_index,
            });
        }
        block.instructions.push(new_instruction);
    }
    Ok(())
}

fn rewrite_operand(
    function: usize,
    block: &mut omega_selected_instructions::SelectedBlock,
    victim: VirtualRegisterId,
    result: VirtualRegisterId,
    rewrite: PressureRematerializationRewrite,
) -> Result<(), PressureRematerializationError> {
    let instruction = block
        .instructions
        .iter_mut()
        .find(|instruction| instruction.id == rewrite.instruction)
        .or_else(|| {
            let terminator = match &mut block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::Return { instruction, .. } => instruction,
            };
            (terminator.id == rewrite.instruction).then_some(terminator)
        })
        .ok_or(PressureRematerializationError::DecisionMismatch { function })?;
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| {
            operand.operand == rewrite.operand
                && operand.virtual_register == victim
                && operand.access == RegisterOperandAccess::Use
                && operand.fixed_view.is_none()
        })
        .ok_or(PressureRematerializationError::DecisionMismatch { function })?;
    operand.virtual_register = result;
    Ok(())
}

fn selected_operand(
    constraint: &omega_register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

fn find_instruction(
    block: &omega_selected_instructions::SelectedBlock,
    id: SelectedInstructionId,
) -> Option<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == id)
        .or_else(|| match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
                if instruction.id == id =>
            {
                Some(instruction)
            }
            _ => None,
        })
}

fn validate_dense(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), PressureRematerializationError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(PressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    let count = instruction_count(function_index, function)?;
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(PressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn instruction_count(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<u32, PressureRematerializationError> {
    let count = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len().checked_add(1)?)
        })
        .ok_or(PressureRematerializationError::IdentifierOverflow {
            function: function_index,
        })?;
    u32::try_from(count).map_err(|_| PressureRematerializationError::IdentifierOverflow {
        function: function_index,
    })
}

fn required_usage(
    selected: &SelectedInstructionPlan,
    applied: usize,
    rewritten_uses: usize,
) -> Result<OptimizationWorkUsage, PressureRematerializationError> {
    let rule_evaluations = u64::try_from(selected.functions.len())
        .map_err(|_| PressureRematerializationError::WorkOverflow)?;
    let validation_steps = selected
        .functions
        .iter()
        .try_fold(0u64, |total, function| {
            let instructions = function.blocks.iter().try_fold(0u64, |count, block| {
                count.checked_add(
                    u64::try_from(block.instructions.len())
                        .ok()?
                        .checked_add(1)?,
                )
            })?;
            total
                .checked_add(u64::try_from(function.virtual_registers.len()).ok()?)?
                .checked_add(instructions)
        })
        .ok_or(PressureRematerializationError::WorkOverflow)?
        .checked_add(
            u64::try_from(rewritten_uses)
                .map_err(|_| PressureRematerializationError::WorkOverflow)?,
        )
        .ok_or(PressureRematerializationError::WorkOverflow)?;
    let applied =
        u64::try_from(applied).map_err(|_| PressureRematerializationError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations,
        candidates: applied,
        validation_steps,
        commits: applied,
        iterations: 1,
    })
}

fn ensure_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), PressureRematerializationError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(PressureRematerializationError::BudgetExceeded {
            required: usage,
            budget,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_optimization_core::{
        OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
    use omega_register_model::{
        PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterConstraintFamily,
        RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
        RegisterOperandAccess, RegisterOperandConstraint, RegisterUnit, RegisterUnitId,
        RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
        TargetRegisterEnvironmentIdentity, validate_physical_register_model,
    };
    use omega_selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity, SelectedInstructionProvenance, SelectedOperand,
        SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;

    fn operand(register: u32, access: RegisterOperandAccess) -> SelectedOperand {
        SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(register),
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }

    pub(crate) fn fixture() -> (
        SelectedInstructionPlan,
        LiveRangePlan,
        RecoveryClassificationPlan,
        RegisterInstructionConstraint,
    ) {
        let machine = MachineId::new(1).unwrap();
        let source_block = BlockId::new(1).unwrap();
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let key = RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 1,
        };
        let mut definitions = Vec::new();
        let mut registers = Vec::new();
        for register in 0..3_u32 {
            let source_value = ValueId::new(u64::from(register) + 1).unwrap();
            let operation = OperationId::new(u64::from(register) + 1).unwrap();
            definitions.push(SelectedInstruction {
                id: SelectedInstructionId(register),
                kind: SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(register) + 40),
                },
                constraint: key,
                operands: vec![operand(register, RegisterOperandAccess::Def)],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![operation],
                    values: vec![source_value],
                    edges: Vec::new(),
                    obligations: Vec::new(),
                    fuel: vec![FuelSettlement {
                        site: PsiProvenance::Operation(operation),
                        units: 2,
                    }],
                },
            });
            registers.push(VirtualRegister {
                id: VirtualRegisterId(register),
                scalar_type: scalar,
                class: RegisterClassId(0),
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(register),
                    source_value,
                },
                definition_site: ValueDefinitionSite::Node {
                    block: source_block,
                    node: register,
                },
                entry_fixed_view: None,
            });
        }
        let returned = SelectedInstruction {
            id: SelectedInstructionId(3),
            kind: SelectedInstructionKind::ReturnI64,
            constraint: key,
            operands: vec![operand(0, RegisterOperandAccess::Use)],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance {
                values: vec![ValueId::new(1).unwrap()],
                ..Default::default()
            },
        };
        let selected = SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: omega_target::NativeTarget::linux_x64(),
            entry: machine,
            functions: vec![SelectedFunction {
                machine,
                attachment: None,
                provenance: Default::default(),
                entry_block: SelectedBlockId(0),
                virtual_registers: registers,
                blocks: vec![SelectedBlock {
                    id: SelectedBlockId(0),
                    source_block,
                    instructions: definitions,
                    terminator: SelectedTerminator::Return {
                        instruction: returned,
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        let ranges = LiveRangePlan {
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            liveness: LivenessIdentity::from_bytes([3; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: selected.target,
            functions: vec![FunctionLiveRanges {
                machine,
                block_domains: vec![BlockPointDomain {
                    block: SelectedBlockId(0),
                    source_block,
                    start: LiveRangePoint(0),
                    end: LiveRangePoint(8),
                }],
                virtual_registers: vec![VirtualLiveRange {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(0),
                    occurrences: vec![
                        VirtualOccurrence {
                            position: LivenessPosition(0),
                            point: LiveRangePoint(1),
                            instruction: SelectedInstructionId(0),
                            operand: 0,
                            access: RegisterOperandAccess::Def,
                        },
                        VirtualOccurrence {
                            position: LivenessPosition(3),
                            point: LiveRangePoint(6),
                            instruction: SelectedInstructionId(3),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                    ],
                    fixed_constraints: Vec::new(),
                    fragments: vec![LiveRangeFragment {
                        block: SelectedBlockId(0),
                        start: LiveRangePoint(1),
                        end: LiveRangePoint(7),
                    }],
                    edge_connectors: Vec::new(),
                }],
                tied_pairs: Vec::new(),
                early_clobbers: Vec::new(),
                architectural_units: Vec::new(),
                interference: Vec::new(),
            }],
            structural_unit_functions: Vec::new(),
        };
        let original = &selected.functions[0].blocks[0].instructions[0];
        let recovery = RecoveryClassificationPlan {
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([5; 32]),
            ranges: LiveRangeIdentity::from_bytes([6; 32]),
            legality: AllocationLegalityIdentity::from_bytes([7; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            policy: RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 1,
                commits: 1,
                iterations: 1,
            },
            functions: vec![FunctionRecoveryClassification {
                machine,
                classification: Some(PressureRecoveryClassification {
                    block: SelectedBlockId(0),
                    point: LiveRangePoint(5),
                    victim: VirtualRegisterId(0),
                    role: RecoveryVictimRole::ActiveResident {
                        current_view: RegisterViewId(0),
                        reclaimed_view: RegisterViewId(0),
                    },
                    scalar_type: scalar,
                    class: RegisterClassId(0),
                    origin: selected.functions[0].virtual_registers[0].origin,
                    definition_site: selected.functions[0].virtual_registers[0].definition_site,
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: original.id,
                            source_value: ValueId::new(1).unwrap(),
                            value: IntegerValue::Unsigned(40),
                            provenance: original.provenance.clone(),
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(6),
                                instruction: SelectedInstructionId(3),
                                operand: 0,
                            }],
                        },
                }),
            }],
        };
        let row = RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: vec![RegisterOperandConstraint {
                operand: 0,
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        };
        (selected, ranges, recovery, row)
    }

    pub(crate) fn multiple_future_fixture() -> (
        SelectedInstructionPlan,
        LiveRangePlan,
        RecoveryClassificationPlan,
        RegisterInstructionConstraint,
    ) {
        let (mut selected, mut ranges, mut recovery, row) = fixture();
        let block = &mut selected.functions[0].blocks[0];
        let SelectedTerminator::Return { instruction, .. } = &mut block.terminator else {
            unreachable!()
        };
        instruction.id = SelectedInstructionId(4);
        block.instructions.push(SelectedInstruction {
            id: SelectedInstructionId(3),
            kind: SelectedInstructionKind::CompareI64Zero,
            constraint: row.key,
            operands: vec![
                operand(0, RegisterOperandAccess::Use),
                SelectedOperand {
                    operand: 1,
                    virtual_register: VirtualRegisterId(1),
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                },
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance {
                values: vec![ValueId::new(1).unwrap()],
                ..Default::default()
            },
        });
        let function_ranges = &mut ranges.functions[0];
        function_ranges.block_domains[0].end = LiveRangePoint(10);
        let victim = &mut function_ranges.virtual_registers[0];
        victim.occurrences[1] = VirtualOccurrence {
            position: LivenessPosition(3),
            point: LiveRangePoint(6),
            instruction: SelectedInstructionId(3),
            operand: 0,
            access: RegisterOperandAccess::Use,
        };
        victim.occurrences.push(VirtualOccurrence {
            position: LivenessPosition(4),
            point: LiveRangePoint(8),
            instruction: SelectedInstructionId(4),
            operand: 0,
            access: RegisterOperandAccess::Use,
        });
        victim.fragments[0].end = LiveRangePoint(9);
        let Some(PressureRecoveryClassification {
            classification:
                RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
            ..
        }) = recovery.functions[0].classification.as_mut()
        else {
            unreachable!()
        };
        future_uses.push(RecoveryFutureUse {
            block: SelectedBlockId(0),
            point: LiveRangePoint(8),
            instruction: SelectedInstructionId(4),
            operand: 0,
        });
        (selected, ranges, recovery, row)
    }

    fn same_instruction_multiple_future_fixture() -> (
        SelectedInstructionPlan,
        LiveRangePlan,
        RecoveryClassificationPlan,
        RegisterInstructionConstraint,
    ) {
        let (mut selected, mut ranges, mut recovery, row) = multiple_future_fixture();
        selected.functions[0].blocks[0].instructions[3].operands[1].virtual_register =
            VirtualRegisterId(0);
        let SelectedTerminator::Return { instruction, .. } =
            &mut selected.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.operands[0].virtual_register = VirtualRegisterId(1);
        let victim = &mut ranges.functions[0].virtual_registers[0];
        victim.occurrences[2] = VirtualOccurrence {
            position: LivenessPosition(3),
            point: LiveRangePoint(6),
            instruction: SelectedInstructionId(3),
            operand: 1,
            access: RegisterOperandAccess::Use,
        };
        victim.fragments[0].end = LiveRangePoint(7);
        let Some(PressureRecoveryClassification {
            classification:
                RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
            ..
        }) = recovery.functions[0].classification.as_mut()
        else {
            unreachable!()
        };
        future_uses[1] = RecoveryFutureUse {
            block: SelectedBlockId(0),
            point: LiveRangePoint(6),
            instruction: SelectedInstructionId(3),
            operand: 1,
        };
        (selected, ranges, recovery, row)
    }

    #[test]
    fn active_resident_is_split_before_sole_future_use_and_reanalyzes() {
        let (selected, ranges, recovery, row) = fixture();
        let original = selected.functions[0].blocks[0].instructions[0].clone();
        let (functions, transformed) = build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
        )
        .unwrap();
        let action = functions[0].action.as_ref().unwrap();
        assert_eq!(action.fresh_materialize, SelectedInstructionId(4));
        assert_eq!(action.result_virtual_register, VirtualRegisterId(3));
        let function = &transformed.functions[0];
        let transformed_machine = function.machine;
        assert_eq!(function.blocks[0].instructions[0], original);
        let inserted = function.blocks[0].instructions.last().unwrap();
        assert_eq!(inserted.id, SelectedInstructionId(4));
        assert_eq!(
            inserted.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(40)
            }
        );
        assert!(inserted.provenance.operations.is_empty());
        assert_eq!(inserted.provenance.values, vec![ValueId::new(1).unwrap()]);
        assert!(inserted.provenance.edges.is_empty());
        assert!(inserted.provenance.obligations.is_empty());
        assert!(inserted.provenance.fuel.is_empty());
        assert_eq!(original.provenance.fuel.len(), 1);
        let returned = match &function.blocks[0].terminator {
            SelectedTerminator::Return { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        assert_eq!(returned.operands[0].virtual_register, VirtualRegisterId(3));
        assert_eq!(
            function.virtual_registers[3].origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(4),
                source_value: ValueId::new(1).unwrap()
            }
        );

        let transformed_identity = selected_instruction_plan_identity(&transformed);
        let optimization_unit = OptimizationUnitIdentity::from_bytes([4; 32]);
        let plan = PressureRematerializationPlan {
            source_selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([5; 32]),
            recovery_classifications: RecoveryClassificationIdentity::from_bytes([10; 32]),
            ranges: LiveRangeIdentity::from_bytes([6; 32]),
            legality: AllocationLegalityIdentity::from_bytes([7; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit, fuel_schedule: transformed.fuel_schedule,
            policy: PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
            usage: OptimizationWorkUsage { rule_evaluations: 1, candidates: 1, validation_steps: 8, commits: 1, iterations: 1 },
            functions, transformed_selected: transformed_identity,
        };
        let receipt = PressureRematerializationValidationReceipt {
            identity: pressure_rematerialization_identity(&plan),
            source_selected: plan.source_selected,
            spill_choices: plan.spill_choices,
            recovery_classifications: plan.recovery_classifications,
            ranges: plan.ranges,
            legality: plan.legality,
            register_environment: plan.register_environment,
            allocator_availability: plan.allocator_availability,
            optimization_unit,
            fuel_schedule: plan.fuel_schedule,
            transformed_selected: transformed_identity,
            policy: plan.policy,
            usage: plan.usage,
            function_count: 1,
            applied_count: 1,
            rewritten_use_count: 1,
        };
        let validated = ValidatedPressureRematerialization {
            plan,
            transformed,
            receipt,
        };
        let liveness = analyze_liveness(&validated).unwrap();
        let post_ranges = analyze_live_ranges(&validated, &liveness).unwrap();
        assert_eq!(post_ranges.receipt().virtual_register_count(), 4);
        assert!(
            !post_ranges.plan().functions[0]
                .interference
                .iter()
                .any(|edge| edge.lower == VirtualRegisterId(0)
                    && edge.higher == VirtualRegisterId(2))
        );

        let physical = validate_physical_register_model(PhysicalRegisterModel {
            architecture: omega_target::Architecture::X86_64,
            units: (0..2)
                .map(|id| RegisterUnit {
                    id: RegisterUnitId(id),
                    name: format!("r{id}.storage"),
                    bits: 64,
                    kind: RegisterUnitKind::IntegerLane,
                })
                .collect(),
            views: (0..2)
                .map(|id| RegisterView {
                    id: RegisterViewId(id),
                    name: format!("r{id}"),
                    class: RegisterClassId(0),
                    units: vec![RegisterUnitId(id)],
                    write_units: vec![RegisterUnitId(id)],
                    bits: 64,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                })
                .collect(),
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
            conventions: Vec::new(),
            reservations: Vec::new(),
        })
        .unwrap();
        let legality = FunctionAllocationLegality {
            machine: transformed_machine,
            virtual_registers: post_ranges.plan().functions[0]
                .virtual_registers
                .iter()
                .map(|range| VirtualRegisterAllocationLegality {
                    virtual_register: range.virtual_register,
                    class: range.class,
                    points: range
                        .fragments
                        .iter()
                        .flat_map(|fragment| fragment.start.0..fragment.end.0)
                        .map(|point| VirtualPointLegality {
                            block: SelectedBlockId(0),
                            point: LiveRangePoint(point),
                            candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                        })
                        .collect(),
                    early_clobber_points: Vec::new(),
                    entry_transitions: Vec::new(),
                })
                .collect(),
        };
        let homes = crate::home_assignment_compute::compute_function(
            0,
            &legality,
            &post_ranges.plan().functions[0],
            &physical,
        )
        .unwrap();
        let replayed_homes = crate::home_assignment_validate::replay_function(
            0,
            &legality,
            &post_ranges.plan().functions[0],
            &physical,
        )
        .unwrap();
        assert_eq!(homes, replayed_homes);
        assert_eq!(homes.assignments.len(), 4);
    }

    #[test]
    fn active_resident_is_split_once_before_a_multiple_use_suffix_and_reanalyzes() {
        let (selected, ranges, recovery, row) = multiple_future_fixture();
        assert!(matches!(
            build_functions(
                &selected,
                &ranges,
                &recovery,
                &row,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            ),
            Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
        ));
        let (functions, transformed) = build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        )
        .unwrap();
        let action = functions[0].action.as_ref().unwrap();
        assert_eq!(action.rewrites.len(), 2);
        assert_eq!(action.rewrites[0].instruction, SelectedInstructionId(3));
        assert_eq!(action.rewrites[1].instruction, SelectedInstructionId(4));
        assert_eq!(action.fresh_materialize, SelectedInstructionId(5));
        assert_eq!(action.result_virtual_register, VirtualRegisterId(3));

        let source_original = &selected.functions[0].blocks[0].instructions[0];
        let transformed_function = &transformed.functions[0];
        let transformed_machine = transformed_function.machine;
        assert_eq!(
            transformed_function.blocks[0].instructions[0],
            *source_original
        );
        assert_eq!(source_original.provenance.fuel.len(), 1);
        let inserted = &transformed_function.blocks[0].instructions[3];
        assert_eq!(inserted.id, SelectedInstructionId(5));
        assert_eq!(inserted.provenance.values, vec![ValueId::new(1).unwrap()]);
        assert!(inserted.provenance.operations.is_empty());
        assert!(inserted.provenance.fuel.is_empty());
        assert_eq!(
            transformed_function.blocks[0].instructions[4].operands[0].virtual_register,
            VirtualRegisterId(3)
        );
        assert_eq!(
            transformed_function.blocks[0].instructions[4].operands[1].virtual_register,
            VirtualRegisterId(1)
        );
        let returned = match &transformed_function.blocks[0].terminator {
            SelectedTerminator::Return { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        assert_eq!(returned.operands[0].virtual_register, VirtualRegisterId(3));

        let transformed_identity = selected_instruction_plan_identity(&transformed);
        let plan = PressureRematerializationPlan {
            source_selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([5; 32]),
            recovery_classifications: RecoveryClassificationIdentity::from_bytes([10; 32]),
            ranges: LiveRangeIdentity::from_bytes([6; 32]),
            legality: AllocationLegalityIdentity::from_bytes([7; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
            fuel_schedule: transformed.fuel_schedule,
            policy: PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
            usage: OptimizationWorkUsage { rule_evaluations: 1, candidates: 1, validation_steps: 10, commits: 1, iterations: 1 },
            functions,
            transformed_selected: transformed_identity,
        };
        let receipt = PressureRematerializationValidationReceipt {
            identity: pressure_rematerialization_identity(&plan),
            source_selected: plan.source_selected,
            spill_choices: plan.spill_choices,
            recovery_classifications: plan.recovery_classifications,
            ranges: plan.ranges,
            legality: plan.legality,
            register_environment: plan.register_environment,
            allocator_availability: plan.allocator_availability,
            optimization_unit: plan.optimization_unit,
            fuel_schedule: plan.fuel_schedule,
            transformed_selected: transformed_identity,
            policy: plan.policy,
            usage: plan.usage,
            function_count: 1,
            applied_count: 1,
            rewritten_use_count: 2,
        };
        let validated = ValidatedPressureRematerialization {
            plan,
            transformed,
            receipt,
        };
        assert_eq!(validated.receipt().rewritten_use_count(), 2);
        let liveness = analyze_liveness(&validated).unwrap();
        let post_ranges = analyze_live_ranges(&validated, &liveness).unwrap();
        let victim = &post_ranges.plan().functions[0].virtual_registers[0];
        assert!(
            victim
                .fragments
                .iter()
                .all(|fragment| fragment.end <= LiveRangePoint(5))
        );
        let suffix = &post_ranges.plan().functions[0].virtual_registers[3];
        assert_eq!(suffix.occurrences.len(), 3);

        let physical = validate_physical_register_model(PhysicalRegisterModel {
            architecture: omega_target::Architecture::X86_64,
            units: (0..2)
                .map(|id| RegisterUnit {
                    id: RegisterUnitId(id),
                    name: format!("r{id}.storage"),
                    bits: 64,
                    kind: RegisterUnitKind::IntegerLane,
                })
                .collect(),
            views: (0..2)
                .map(|id| RegisterView {
                    id: RegisterViewId(id),
                    name: format!("r{id}"),
                    class: RegisterClassId(0),
                    units: vec![RegisterUnitId(id)],
                    write_units: vec![RegisterUnitId(id)],
                    bits: 64,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                })
                .collect(),
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
            conventions: Vec::new(),
            reservations: Vec::new(),
        })
        .unwrap();
        let legality = FunctionAllocationLegality {
            machine: transformed_machine,
            virtual_registers: post_ranges.plan().functions[0]
                .virtual_registers
                .iter()
                .map(|range| VirtualRegisterAllocationLegality {
                    virtual_register: range.virtual_register,
                    class: range.class,
                    points: range
                        .fragments
                        .iter()
                        .flat_map(|fragment| fragment.start.0..fragment.end.0)
                        .map(|point| VirtualPointLegality {
                            block: SelectedBlockId(0),
                            point: LiveRangePoint(point),
                            candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                        })
                        .collect(),
                    early_clobber_points: Vec::new(),
                    entry_transitions: Vec::new(),
                })
                .collect(),
        };
        let homes = crate::home_assignment_compute::compute_function(
            0,
            &legality,
            &post_ranges.plan().functions[0],
            &physical,
        )
        .unwrap();
        assert_eq!(
            homes,
            crate::home_assignment_validate::replay_function(
                0,
                &legality,
                &post_ranges.plan().functions[0],
                &physical,
            )
            .unwrap()
        );
        assert_eq!(homes.assignments.len(), 4);
    }

    #[test]
    fn multiple_use_policy_rejects_noncanonical_or_single_rewrite_evidence() {
        let (single_selected, single_ranges, single_recovery, row) = fixture();
        assert!(matches!(
            build_functions(
                &single_selected,
                &single_ranges,
                &single_recovery,
                &row,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            ),
            Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
        ));

        let (selected, ranges, mut recovery, row) = multiple_future_fixture();
        {
            let Some(PressureRecoveryClassification {
                classification:
                    RecoveryClassification::ImmediateU64RematerializationCandidate {
                        future_uses, ..
                    },
                ..
            }) = recovery.functions[0].classification.as_mut()
            else {
                unreachable!()
            };
            future_uses.swap(0, 1);
        }
        assert!(matches!(
            build_functions(
                &selected,
                &ranges,
                &recovery,
                &row,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            ),
            Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
        ));
        {
            let Some(PressureRecoveryClassification {
                classification:
                    RecoveryClassification::ImmediateU64RematerializationCandidate {
                        future_uses, ..
                    },
                ..
            }) = recovery.functions[0].classification.as_mut()
            else {
                unreachable!()
            };
            future_uses.swap(0, 1);
            future_uses[1] = future_uses[0];
        }
        assert!(matches!(
            build_functions(
                &selected,
                &ranges,
                &recovery,
                &row,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            ),
            Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
        ));

        let (selected, ranges, recovery, row) = same_instruction_multiple_future_fixture();
        let usage = required_usage(&selected, 1, 2).unwrap();
        assert_eq!(usage.validation_steps, 10);
        let insufficient = OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps - 1,
            usage.commits,
            usage.iterations,
        )
        .unwrap();
        assert_eq!(
            ensure_budget(usage, insufficient),
            Err(PressureRematerializationError::BudgetExceeded {
                required: usage,
                budget: insufficient,
            })
        );
        let (functions, transformed) = build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        )
        .unwrap();
        assert_eq!(functions[0].action.as_ref().unwrap().rewrites.len(), 2);
        assert_eq!(
            transformed.functions[0].blocks[0].instructions[4]
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            vec![VirtualRegisterId(3), VirtualRegisterId(3)]
        );

        let (mut selected, ranges, recovery, row) = multiple_future_fixture();
        selected.functions[0].blocks[0].instructions[3].operands[0].fixed_view =
            Some(RegisterViewId(0));
        assert!(matches!(
            build_functions(
                &selected,
                &ranges,
                &recovery,
                &row,
                PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            ),
            Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
        ));
    }
}
