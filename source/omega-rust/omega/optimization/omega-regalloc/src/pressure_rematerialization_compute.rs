use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedFunction, TerminalSelectedInstruction, TerminalSelectedInstructionId,
    TerminalSelectedInstructionKind, TerminalSelectedInstructionPlan,
    TerminalSelectedInstructionProvenance, TerminalSelectedOperand, TerminalSelectedTerminator,
    TerminalVirtualRegister, TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use omega_terminal_target_operations_to_selected_instructions::terminal_selected_instruction_plan_identity;

use crate::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_pressure_rematerialization<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
    recovery: &ValidatedTerminalRecoveryClassifications,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalPressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<TerminalPressureRematerializationPlan, TerminalPressureRematerializationError> {
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
    if policy != TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 {
        return Err(TerminalPressureRematerializationError::UnsupportedPolicy);
    }
    let materialize = materialize_row(constraints, selected_keys)?;
    let (functions, transformed) = build_functions(
        selected.selected_plan(),
        ranges.plan(),
        recovery.plan(),
        materialize,
    )?;
    let applied = functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    if applied == 0 {
        return Err(TerminalPressureRematerializationError::NoAction);
    }
    let usage = required_usage(selected, applied)?;
    ensure_budget(usage, budget)?;
    Ok(TerminalPressureRematerializationPlan {
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
        transformed_selected: terminal_selected_instruction_plan_identity(&transformed),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
    recovery: &ValidatedTerminalRecoveryClassifications,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), TerminalPressureRematerializationError> {
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
        return Err(TerminalPressureRematerializationError::RootMismatch);
    }
    Ok(())
}

fn materialize_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, TerminalPressureRematerializationError> {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == keys.materialize_i64)
        .ok_or(TerminalPressureRematerializationError::MaterializeConstraintMismatch)?;
    let [result] = row.operands.as_slice() else {
        return Err(TerminalPressureRematerializationError::MaterializeConstraintMismatch);
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
        return Err(TerminalPressureRematerializationError::MaterializeConstraintMismatch);
    }
    Ok(row)
}

pub(crate) fn build_functions(
    selected: &TerminalSelectedInstructionPlan,
    ranges: &TerminalLiveRangePlan,
    recovery: &TerminalRecoveryClassificationPlan,
    row: &RegisterInstructionConstraint,
) -> Result<
    (
        Vec<TerminalFunctionPressureRematerialization>,
        TerminalSelectedInstructionPlan,
    ),
    TerminalPressureRematerializationError,
> {
    let mut transformed = selected.clone();
    let mut functions = Vec::with_capacity(transformed.functions.len());
    for index in 0..transformed.functions.len() {
        let source = &selected.functions[index];
        let range_function = ranges
            .functions
            .get(index)
            .ok_or(TerminalPressureRematerializationError::FunctionMismatch { function: index })?;
        let recovery_function = recovery
            .functions
            .get(index)
            .ok_or(TerminalPressureRematerializationError::FunctionMismatch { function: index })?;
        if source.machine != range_function.machine || source.machine != recovery_function.machine {
            return Err(TerminalPressureRematerializationError::FunctionMismatch {
                function: index,
            });
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
            )?),
        };
        if let Some(action) = action {
            apply_action(index, &mut transformed.functions[index], action, row)?;
        }
        functions.push(TerminalFunctionPressureRematerialization {
            machine: source.machine,
            action,
        });
    }
    Ok((functions, transformed))
}

fn action_from_candidate(
    function_index: usize,
    function: &TerminalSelectedFunction,
    ranges: &TerminalFunctionLiveRanges,
    candidate: &TerminalPressureRecoveryClassification,
    row: &RegisterInstructionConstraint,
) -> Result<TerminalPressureRematerializationAction, TerminalPressureRematerializationError> {
    let TerminalRecoveryVictimRole::ActiveResident {
        current_view,
        reclaimed_view,
    } = candidate.role
    else {
        return Err(
            TerminalPressureRematerializationError::UnsupportedVictimRole {
                function: function_index,
            },
        );
    };
    let TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
        defining_instruction,
        source_value,
        value,
        provenance,
        future_uses,
    } = &candidate.classification
    else {
        return Err(
            TerminalPressureRematerializationError::ClassificationNotAdmitted {
                function: function_index,
            },
        );
    };
    let [future] = future_uses.as_slice() else {
        return Err(TerminalPressureRematerializationError::FutureUseMismatch {
            function: function_index,
        });
    };
    if future.block != candidate.block || future.point <= candidate.point {
        return Err(TerminalPressureRematerializationError::FutureUseMismatch {
            function: function_index,
        });
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|register| register.id == candidate.victim)
        .ok_or(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        )?;
    if victim.scalar_type != candidate.scalar_type
        || victim.class != candidate.class
        || victim.origin != candidate.origin
        || victim.definition_site != candidate.definition_site
        || victim.entry_fixed_view.is_some()
        || row.operands[0].class != victim.class
    {
        return Err(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        );
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.block)
        .ok_or(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        )?;
    let original = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == *defining_instruction)
        .ok_or(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        )?;
    if original.kind != (TerminalSelectedInstructionKind::MaterializeI64 { value: *value })
        || original.constraint != row.key
        || original.provenance != *provenance
        || original.operands.as_slice() != [selected_operand(&row.operands[0], candidate.victim)]
    {
        return Err(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        );
    }
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == candidate.victim)
        .ok_or(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        )?;
    if !range.occurrences.iter().any(|occurrence| {
        occurrence.instruction == *defining_instruction
            && occurrence.access == RegisterOperandAccess::Def
            && occurrence.point < candidate.point
    }) {
        return Err(
            TerminalPressureRematerializationError::MaterializeMismatch {
                function: function_index,
            },
        );
    }
    let future_instruction = find_instruction(block, future.instruction).ok_or(
        TerminalPressureRematerializationError::FutureUseMismatch {
            function: function_index,
        },
    )?;
    let operand = future_instruction
        .operands
        .iter()
        .find(|operand| operand.operand == future.operand)
        .ok_or(TerminalPressureRematerializationError::FutureUseMismatch {
            function: function_index,
        })?;
    if operand.virtual_register != candidate.victim
        || operand.access != RegisterOperandAccess::Use
        || operand.fixed_view.is_some()
        || operand.class != candidate.class
    {
        return Err(TerminalPressureRematerializationError::FutureUseMismatch {
            function: function_index,
        });
    }
    let fresh_instruction =
        TerminalSelectedInstructionId(instruction_count(function_index, function)?);
    let fresh_register =
        TerminalVirtualRegisterId(u32::try_from(function.virtual_registers.len()).map_err(
            |_| TerminalPressureRematerializationError::IdentifierOverflow {
                function: function_index,
            },
        )?);
    Ok(TerminalPressureRematerializationAction {
        block: candidate.block,
        pressure_point: candidate.point,
        victim: candidate.victim,
        current_view,
        reclaimed_view,
        original_materialize: *defining_instruction,
        source_value: *source_value,
        value: *value,
        future_point: future.point,
        future_instruction: future.instruction,
        future_operand: future.operand,
        fresh_materialize: fresh_instruction,
        result_virtual_register: fresh_register,
        materialize_constraint: row.key,
    })
}

fn apply_action(
    function_index: usize,
    function: &mut TerminalSelectedFunction,
    action: TerminalPressureRematerializationAction,
    row: &RegisterInstructionConstraint,
) -> Result<(), TerminalPressureRematerializationError> {
    let source = function
        .virtual_registers
        .iter()
        .find(|register| register.id == action.victim)
        .cloned()
        .ok_or(TerminalPressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
    function.virtual_registers.push(TerminalVirtualRegister {
        id: action.result_virtual_register,
        scalar_type: source.scalar_type,
        class: source.class,
        origin: TerminalVirtualRegisterOrigin::InstructionResult {
            instruction: action.fresh_materialize,
            source_value: action.source_value,
        },
        definition_site: source.definition_site,
        entry_fixed_view: None,
    });
    let new_instruction = TerminalSelectedInstruction {
        id: action.fresh_materialize,
        kind: TerminalSelectedInstructionKind::MaterializeI64 {
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
        provenance: TerminalSelectedInstructionProvenance {
            values: vec![action.source_value],
            ..Default::default()
        },
    };
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == action.block)
        .ok_or(TerminalPressureRematerializationError::DecisionMismatch {
            function: function_index,
        })?;
    if let Some(index) = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == action.future_instruction)
    {
        rewrite_operand(function_index, &mut block.instructions[index], action)?;
        block.instructions.insert(index, new_instruction);
    } else {
        let terminator = match &mut block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        };
        if terminator.id != action.future_instruction {
            return Err(TerminalPressureRematerializationError::DecisionMismatch {
                function: function_index,
            });
        }
        rewrite_operand(function_index, terminator, action)?;
        block.instructions.push(new_instruction);
    }
    Ok(())
}

fn rewrite_operand(
    function: usize,
    instruction: &mut TerminalSelectedInstruction,
    action: TerminalPressureRematerializationAction,
) -> Result<(), TerminalPressureRematerializationError> {
    let operand = instruction
        .operands
        .iter_mut()
        .find(|operand| {
            operand.operand == action.future_operand
                && operand.virtual_register == action.victim
                && operand.access == RegisterOperandAccess::Use
                && operand.fixed_view.is_none()
        })
        .ok_or(TerminalPressureRematerializationError::DecisionMismatch { function })?;
    operand.virtual_register = action.result_virtual_register;
    Ok(())
}

fn selected_operand(
    constraint: &omega_register_model::RegisterOperandConstraint,
    register: TerminalVirtualRegisterId,
) -> TerminalSelectedOperand {
    TerminalSelectedOperand {
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
    block: &omega_terminal_selected_instructions::TerminalSelectedBlock,
    id: TerminalSelectedInstructionId,
) -> Option<&TerminalSelectedInstruction> {
    block
        .instructions
        .iter()
        .find(|instruction| instruction.id == id)
        .or_else(|| match &block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. }
                if instruction.id == id =>
            {
                Some(instruction)
            }
            _ => None,
        })
}

fn validate_dense(
    function_index: usize,
    function: &TerminalSelectedFunction,
) -> Result<(), TerminalPressureRematerializationError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(TerminalPressureRematerializationError::FunctionMismatch {
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
                    TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
                    | TerminalSelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(TerminalPressureRematerializationError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn instruction_count(
    function_index: usize,
    function: &TerminalSelectedFunction,
) -> Result<u32, TerminalPressureRematerializationError> {
    let count = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len().checked_add(1)?)
        })
        .ok_or(TerminalPressureRematerializationError::IdentifierOverflow {
            function: function_index,
        })?;
    u32::try_from(count).map_err(
        |_| TerminalPressureRematerializationError::IdentifierOverflow {
            function: function_index,
        },
    )
}

fn required_usage(
    selected: &impl ValidatedTerminalSelectedAnalysis,
    applied: usize,
) -> Result<OptimizationWorkUsage, TerminalPressureRematerializationError> {
    let rule_evaluations = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| TerminalPressureRematerializationError::WorkOverflow)?;
    let validation_steps = selected
        .selected_plan()
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
        .ok_or(TerminalPressureRematerializationError::WorkOverflow)?;
    let applied =
        u64::try_from(applied).map_err(|_| TerminalPressureRematerializationError::WorkOverflow)?;
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
) -> Result<(), TerminalPressureRematerializationError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(TerminalPressureRematerializationError::BudgetExceeded {
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
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlock, TerminalSelectedBlockId, TerminalSelectedFunction,
        TerminalSelectedInstruction, TerminalSelectedInstructionId,
        TerminalSelectedInstructionKind, TerminalSelectedInstructionPlan,
        TerminalSelectedInstructionPlanIdentity, TerminalSelectedInstructionProvenance,
        TerminalSelectedOperand, TerminalSelectedTerminator, TerminalVirtualRegister,
        TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;

    fn operand(register: u32, access: RegisterOperandAccess) -> TerminalSelectedOperand {
        TerminalSelectedOperand {
            operand: 0,
            virtual_register: TerminalVirtualRegisterId(register),
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }

    pub(crate) fn fixture() -> (
        TerminalSelectedInstructionPlan,
        TerminalLiveRangePlan,
        TerminalRecoveryClassificationPlan,
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
            definitions.push(TerminalSelectedInstruction {
                id: TerminalSelectedInstructionId(register),
                kind: TerminalSelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(register) + 40),
                },
                constraint: key,
                operands: vec![operand(register, RegisterOperandAccess::Def)],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: TerminalSelectedInstructionProvenance {
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
            registers.push(TerminalVirtualRegister {
                id: TerminalVirtualRegisterId(register),
                scalar_type: scalar,
                class: RegisterClassId(0),
                origin: TerminalVirtualRegisterOrigin::InstructionResult {
                    instruction: TerminalSelectedInstructionId(register),
                    source_value,
                },
                definition_site: ValueDefinitionSite::Node {
                    block: source_block,
                    node: register,
                },
                entry_fixed_view: None,
            });
        }
        let returned = TerminalSelectedInstruction {
            id: TerminalSelectedInstructionId(3),
            kind: TerminalSelectedInstructionKind::ReturnI64,
            constraint: key,
            operands: vec![operand(0, RegisterOperandAccess::Use)],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: TerminalSelectedInstructionProvenance {
                values: vec![ValueId::new(1).unwrap()],
                ..Default::default()
            },
        };
        let selected = TerminalSelectedInstructionPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: omega_target::NativeTarget::linux_x64(),
            entry: machine,
            functions: vec![TerminalSelectedFunction {
                machine,
                attachment: None,
                provenance: Default::default(),
                entry_block: TerminalSelectedBlockId(0),
                virtual_registers: registers,
                blocks: vec![TerminalSelectedBlock {
                    id: TerminalSelectedBlockId(0),
                    source_block,
                    instructions: definitions,
                    terminator: TerminalSelectedTerminator::Return {
                        instruction: returned,
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }],
            }],
        };
        let ranges = TerminalLiveRangePlan {
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([2; 32]),
            liveness: TerminalLivenessIdentity::from_bytes([3; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: selected.target,
            functions: vec![TerminalFunctionLiveRanges {
                machine,
                block_domains: vec![TerminalBlockPointDomain {
                    block: TerminalSelectedBlockId(0),
                    source_block,
                    start: TerminalLiveRangePoint(0),
                    end: TerminalLiveRangePoint(8),
                }],
                virtual_registers: vec![TerminalVirtualLiveRange {
                    virtual_register: TerminalVirtualRegisterId(0),
                    class: RegisterClassId(0),
                    occurrences: vec![
                        TerminalVirtualOccurrence {
                            position: TerminalLivenessPosition(0),
                            point: TerminalLiveRangePoint(1),
                            instruction: TerminalSelectedInstructionId(0),
                            operand: 0,
                            access: RegisterOperandAccess::Def,
                        },
                        TerminalVirtualOccurrence {
                            position: TerminalLivenessPosition(3),
                            point: TerminalLiveRangePoint(6),
                            instruction: TerminalSelectedInstructionId(3),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                    ],
                    fixed_constraints: Vec::new(),
                    fragments: vec![TerminalLiveRangeFragment {
                        block: TerminalSelectedBlockId(0),
                        start: TerminalLiveRangePoint(1),
                        end: TerminalLiveRangePoint(7),
                    }],
                    edge_connectors: Vec::new(),
                }],
                tied_pairs: Vec::new(),
                early_clobbers: Vec::new(),
                architectural_units: Vec::new(),
                interference: Vec::new(),
            }],
        };
        let original = &selected.functions[0].blocks[0].instructions[0];
        let recovery = TerminalRecoveryClassificationPlan {
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([2; 32]),
            spill_choices: TerminalSpillChoiceIdentity::from_bytes([5; 32]),
            ranges: TerminalLiveRangeIdentity::from_bytes([6; 32]),
            legality: TerminalAllocationLegalityIdentity::from_bytes([7; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
            allocator_availability: TerminalAllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([4; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            policy: TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 1,
                commits: 1,
                iterations: 1,
            },
            functions: vec![TerminalFunctionRecoveryClassification {
                machine,
                classification: Some(TerminalPressureRecoveryClassification {
                    block: TerminalSelectedBlockId(0),
                    point: TerminalLiveRangePoint(5),
                    victim: TerminalVirtualRegisterId(0),
                    role: TerminalRecoveryVictimRole::ActiveResident {
                        current_view: RegisterViewId(0),
                        reclaimed_view: RegisterViewId(0),
                    },
                    scalar_type: scalar,
                    class: RegisterClassId(0),
                    origin: selected.functions[0].virtual_registers[0].origin,
                    definition_site: selected.functions[0].virtual_registers[0].definition_site,
                    classification:
                        TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: original.id,
                            source_value: ValueId::new(1).unwrap(),
                            value: IntegerValue::Unsigned(40),
                            provenance: original.provenance.clone(),
                            future_uses: vec![TerminalRecoveryFutureUse {
                                block: TerminalSelectedBlockId(0),
                                point: TerminalLiveRangePoint(6),
                                instruction: TerminalSelectedInstructionId(3),
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

    #[test]
    fn active_resident_is_split_before_sole_future_use_and_reanalyzes() {
        let (selected, ranges, recovery, row) = fixture();
        let original = selected.functions[0].blocks[0].instructions[0].clone();
        let (functions, transformed) =
            build_functions(&selected, &ranges, &recovery, &row).unwrap();
        let action = functions[0].action.unwrap();
        assert_eq!(action.fresh_materialize, TerminalSelectedInstructionId(4));
        assert_eq!(action.result_virtual_register, TerminalVirtualRegisterId(3));
        let function = &transformed.functions[0];
        let transformed_machine = function.machine;
        assert_eq!(function.blocks[0].instructions[0], original);
        let inserted = function.blocks[0].instructions.last().unwrap();
        assert_eq!(inserted.id, TerminalSelectedInstructionId(4));
        assert_eq!(
            inserted.kind,
            TerminalSelectedInstructionKind::MaterializeI64 {
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
            TerminalSelectedTerminator::Return { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        assert_eq!(
            returned.operands[0].virtual_register,
            TerminalVirtualRegisterId(3)
        );
        assert_eq!(
            function.virtual_registers[3].origin,
            TerminalVirtualRegisterOrigin::InstructionResult {
                instruction: TerminalSelectedInstructionId(4),
                source_value: ValueId::new(1).unwrap()
            }
        );

        let transformed_identity = terminal_selected_instruction_plan_identity(&transformed);
        let optimization_unit = OptimizationUnitIdentity::from_bytes([4; 32]);
        let plan = TerminalPressureRematerializationPlan {
            source_selected: TerminalSelectedInstructionPlanIdentity::from_bytes([2; 32]),
            spill_choices: TerminalSpillChoiceIdentity::from_bytes([5; 32]),
            recovery_classifications: TerminalRecoveryClassificationIdentity::from_bytes([10; 32]),
            ranges: TerminalLiveRangeIdentity::from_bytes([6; 32]),
            legality: TerminalAllocationLegalityIdentity::from_bytes([7; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([8; 32]),
            allocator_availability: TerminalAllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit, fuel_schedule: transformed.fuel_schedule,
            policy: TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
            usage: OptimizationWorkUsage { rule_evaluations: 1, candidates: 1, validation_steps: 8, commits: 1, iterations: 1 },
            functions, transformed_selected: transformed_identity,
        };
        let receipt = TerminalPressureRematerializationValidationReceipt {
            identity: terminal_pressure_rematerialization_identity(&plan),
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
        };
        let validated = ValidatedTerminalPressureRematerialization {
            plan,
            transformed,
            receipt,
        };
        let liveness = analyze_terminal_liveness(&validated).unwrap();
        let post_ranges = analyze_terminal_live_ranges(&validated, &liveness).unwrap();
        assert_eq!(post_ranges.receipt().virtual_register_count(), 4);
        assert!(
            !post_ranges.plan().functions[0]
                .interference
                .iter()
                .any(|edge| edge.lower == TerminalVirtualRegisterId(0)
                    && edge.higher == TerminalVirtualRegisterId(2))
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
        let legality = TerminalFunctionAllocationLegality {
            machine: transformed_machine,
            virtual_registers: post_ranges.plan().functions[0]
                .virtual_registers
                .iter()
                .map(|range| TerminalVirtualRegisterAllocationLegality {
                    virtual_register: range.virtual_register,
                    class: range.class,
                    points: range
                        .fragments
                        .iter()
                        .flat_map(|fragment| fragment.start.0..fragment.end.0)
                        .map(|point| TerminalVirtualPointLegality {
                            block: TerminalSelectedBlockId(0),
                            point: TerminalLiveRangePoint(point),
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
}
