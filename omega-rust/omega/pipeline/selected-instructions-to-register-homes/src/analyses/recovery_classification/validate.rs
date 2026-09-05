use optimization_core::OptimizationWorkUsage;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedTerminator, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::{
    FunctionRecoveryClassification, NoAdmittedRecoveryReason, PressureRecoveryClassification,
    RecoveryClassification, RecoveryClassificationError, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryClassificationValidationReceipt, RecoveryFutureUse,
    RecoveryVictimRole, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
    VirtualFixedConstraintSite, recovery_classification_identity,
};

pub fn validate_recovery_classifications<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    plan: RecoveryClassificationPlan,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    if plan.selected != selected.selected_identity()
        || plan.spill_choices != spill_choices.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.register_environment != legality.receipt().register_environment()
        || plan.allocator_availability != legality.receipt().allocator_availability()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment()
            != legality.receipt().register_environment()
        || spill_choices.receipt().allocator_availability()
            != legality.receipt().allocator_availability()
        || plan.functions.len() != selected.selected_plan().functions.len()
        || plan.functions.len() != ranges.plan().functions.len()
        || plan.functions.len() != legality.plan().functions.len()
        || plan.functions.len() != spill_choices.plan().functions.len()
    {
        return Err(RecoveryClassificationError::RootMismatch);
    }
    if plan.policy != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 {
        return Err(RecoveryClassificationError::UnsupportedPolicy);
    }
    for (function, plan_function) in plan.functions.iter().enumerate() {
        let expected = replay_function(
            function,
            &selected.selected_plan().functions[function],
            &ranges.plan().functions[function],
            &legality.plan().functions[function],
            &spill_choices.plan().functions[function],
        )?;
        if plan_function != &expected {
            return Err(RecoveryClassificationError::ClassificationMismatch { function });
        }
    }
    let expected_usage = replay_usage(selected, ranges, spill_choices)?;
    if plan.usage != expected_usage {
        return Err(RecoveryClassificationError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(RecoveryClassificationError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let classification_count = plan
        .functions
        .iter()
        .filter(|function| function.classification.is_some())
        .count();
    let immediate_candidate_count = plan
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function
                    .classification
                    .as_ref()
                    .map(|row| &row.classification),
                Some(RecoveryClassification::ImmediateU64RematerializationCandidate { .. })
            )
        })
        .count();
    let receipt = RecoveryClassificationValidationReceipt {
        identity: recovery_classification_identity(&plan),
        selected: plan.selected,
        spill_choices: plan.spill_choices,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        classification_count,
        immediate_candidate_count,
    };
    Ok(ValidatedRecoveryClassifications { plan, receipt })
}

fn replay_function(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    legality: &crate::FunctionAllocationLegality,
    choices: &crate::FunctionSpillChoices,
) -> Result<FunctionRecoveryClassification, RecoveryClassificationError> {
    if selected.machine != ranges.machine
        || selected.machine != legality.machine
        || selected.machine != choices.machine
        || selected.virtual_registers.len() != ranges.virtual_registers.len()
        || selected.virtual_registers.len() != legality.virtual_registers.len()
    {
        return Err(RecoveryClassificationError::FunctionMismatch { function });
    }
    let Some(choice) = choices.choice.as_ref() else {
        return Ok(FunctionRecoveryClassification {
            machine: selected.machine,
            classification: None,
        });
    };
    let mut victim = None;
    for register in &selected.virtual_registers {
        if register.id == choice.selected_victim && victim.replace(register).is_some() {
            return Err(RecoveryClassificationError::VictimMismatch {
                function,
                register: choice.selected_victim.0,
            });
        }
    }
    let victim = victim.ok_or(RecoveryClassificationError::VictimMismatch {
        function,
        register: choice.selected_victim.0,
    })?;
    let range_rows = ranges
        .virtual_registers
        .iter()
        .filter(|range| range.virtual_register == victim.id && range.class == victim.class)
        .collect::<Vec<_>>();
    let legality_rows = legality
        .virtual_registers
        .iter()
        .filter(|row| row.virtual_register == victim.id && row.class == victim.class)
        .count();
    if range_rows.len() != 1 || legality_rows != 1 {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let role = replay_role(function, choice)?;
    let classification =
        replay_classification(function, selected, ranges, choice, victim, range_rows[0])?;
    Ok(FunctionRecoveryClassification {
        machine: selected.machine,
        classification: Some(PressureRecoveryClassification {
            block: choice.block,
            point: choice.point,
            victim: victim.id,
            role,
            scalar_type: victim.scalar_type,
            class: victim.class,
            origin: victim.origin,
            definition_site: victim.definition_site,
            classification,
        }),
    })
}

#[cfg(test)]
pub(crate) fn replay_function_for_test(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    legality: &crate::FunctionAllocationLegality,
    choices: &crate::FunctionSpillChoices,
) -> Result<FunctionRecoveryClassification, RecoveryClassificationError> {
    replay_function(function, selected, ranges, legality, choices)
}

fn replay_role(
    function: usize,
    choice: &crate::SpillChoice,
) -> Result<RecoveryVictimRole, RecoveryClassificationError> {
    let selected_rows = choice
        .contenders
        .iter()
        .filter(|contender| contender.virtual_register == choice.selected_victim)
        .collect::<Vec<_>>();
    if selected_rows.len() != 1 {
        return Err(RecoveryClassificationError::ChoiceMismatch { function });
    }
    if choice.incoming == choice.selected_victim {
        if selected_rows[0].reclaimed_view.is_some() {
            return Err(RecoveryClassificationError::ChoiceMismatch { function });
        }
        return Ok(RecoveryVictimRole::Incoming);
    }
    let residents = choice
        .active_residents
        .iter()
        .filter(|resident| resident.virtual_register == choice.selected_victim)
        .collect::<Vec<_>>();
    let Some(reclaimed_view) = selected_rows[0].reclaimed_view else {
        return Err(RecoveryClassificationError::ChoiceMismatch { function });
    };
    if residents.len() != 1 {
        return Err(RecoveryClassificationError::ChoiceMismatch { function });
    }
    Ok(RecoveryVictimRole::ActiveResident {
        current_view: residents[0].view,
        reclaimed_view,
    })
}

fn replay_classification(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    choice: &crate::SpillChoice,
    victim: &selected_instructions::VirtualRegister,
    range: &crate::VirtualLiveRange,
) -> Result<RecoveryClassification, RecoveryClassificationError> {
    let ScalarType::Integer(integer) = victim.scalar_type else {
        return replay_no(NoAdmittedRecoveryReason::UnsupportedScalarType);
    };
    if integer.is_address() || integer.sign() != IntegerSign::Unsigned || integer.bits() != 64 {
        return replay_no(NoAdmittedRecoveryReason::UnsupportedScalarType);
    }
    let (expected_instruction, source_value) = match victim.origin {
        VirtualRegisterOrigin::EntryParameter { .. } => {
            return replay_no(NoAdmittedRecoveryReason::EntryParameter);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        }
        | VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            source_value,
            ..
        } => (instruction, source_value),
    };
    if range.fragments.as_slice()
        != [crate::LiveRangeFragment {
            block: choice.block,
            start: range
                .fragments
                .first()
                .map_or(choice.point, |row| row.start),
            end: range.fragments.first().map_or(choice.point, |row| row.end),
        }]
        || !range.edge_connectors.is_empty()
    {
        return replay_no(NoAdmittedRecoveryReason::UnsupportedRangeShape);
    }
    for fixed in &range.fixed_constraints {
        if let VirtualFixedConstraintSite::Operand { point, .. } = fixed.site
            && point >= choice.point
        {
            return replay_no(NoAdmittedRecoveryReason::FutureFixedUse);
        }
    }
    let mut defining = None;
    let mut definition_count = 0_usize;
    for block in &selected.blocks {
        for instruction in replay_block_instructions(block) {
            if instruction.operands.iter().any(|operand| {
                operand.virtual_register == victim.id
                    && matches!(
                        operand.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    )
            }) {
                definition_count += 1;
                if instruction.id == expected_instruction {
                    defining = Some(instruction);
                }
            }
        }
    }
    let defining = defining.ok_or(RecoveryClassificationError::VictimMismatch {
        function,
        register: victim.id.0,
    })?;
    if definition_count != 1 {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let value = match defining.kind {
        SelectedInstructionKind::ExactAddI64 { .. }
        | SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64 { .. } => {
            return replay_no(NoAdmittedRecoveryReason::ProofBearingDefinition);
        }
        SelectedInstructionKind::MaterializeI64 { value } => value,
        _ => return replay_no(NoAdmittedRecoveryReason::NonMaterializeDefinition),
    };
    if defining.operands.len() != 1
        || defining.operands[0].virtual_register != victim.id
        || defining.operands[0].access != RegisterOperandAccess::Def
        || defining.provenance.values.as_slice() != [source_value]
        || defining.provenance.operations.len() != 1
        || !defining.provenance.edges.is_empty()
        || !defining.provenance.obligations.is_empty()
        || defining.provenance.fuel.is_empty()
        || defining.provenance.fuel.iter().any(|fuel| {
            fuel.site
                != optimization_unit::PsiProvenance::Operation(defining.provenance.operations[0])
        })
        || !integer.admits(value)
    {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let mut future_uses = Vec::new();
    for occurrence in &range.occurrences {
        if occurrence.point < choice.point || occurrence.access != RegisterOperandAccess::Use {
            continue;
        }
        let domains = ranges
            .block_domains
            .iter()
            .filter(|domain| domain.start <= occurrence.point && occurrence.point < domain.end)
            .collect::<Vec<_>>();
        if domains.len() != 1 || domains[0].block != choice.block {
            return Err(RecoveryClassificationError::VictimMismatch {
                function,
                register: victim.id.0,
            });
        }
        let matching_operands = selected
            .blocks
            .iter()
            .filter(|block| block.id == domains[0].block)
            .flat_map(replay_block_instructions)
            .filter(|instruction| instruction.id == occurrence.instruction)
            .flat_map(|instruction| &instruction.operands)
            .filter(|operand| {
                operand.operand == occurrence.operand
                    && operand.virtual_register == victim.id
                    && operand.access == RegisterOperandAccess::Use
                    && operand.fixed_view.is_none()
            })
            .count();
        if matching_operands != 1 {
            return Err(RecoveryClassificationError::VictimMismatch {
                function,
                register: victim.id.0,
            });
        }
        future_uses.push(RecoveryFutureUse {
            block: domains[0].block,
            point: occurrence.point,
            instruction: occurrence.instruction,
            operand: occurrence.operand,
        });
    }
    future_uses.sort_unstable();
    future_uses.dedup();
    if future_uses.is_empty() {
        return replay_no(NoAdmittedRecoveryReason::NoFutureUse);
    }
    Ok(
        RecoveryClassification::ImmediateU64RematerializationCandidate {
            defining_instruction: defining.id,
            source_value,
            value,
            provenance: defining.provenance.clone(),
            future_uses,
        },
    )
}

fn replay_block_instructions(
    block: &selected_instructions::SelectedBlock,
) -> Vec<&SelectedInstruction> {
    let mut instructions = block.instructions.iter().collect::<Vec<_>>();
    instructions.push(match &block.terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    });
    instructions
}

fn replay_no(
    reason: NoAdmittedRecoveryReason,
) -> Result<RecoveryClassification, RecoveryClassificationError> {
    Ok(RecoveryClassification::NoAdmittedRecovery { reason })
}

fn replay_usage(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    spill_choices: &ValidatedSpillChoices,
) -> Result<OptimizationWorkUsage, RecoveryClassificationError> {
    let mut rules = 0_u64;
    let mut candidates = 0_u64;
    let mut steps = 0_u64;
    let mut commits = 0_u64;
    for index in 0..selected.selected_plan().functions.len() {
        replay_add(&mut rules, 1)?;
        let selected_function = &selected.selected_plan().functions[index];
        replay_add(
            &mut steps,
            u64::try_from(selected_function.virtual_registers.len())
                .map_err(|_| RecoveryClassificationError::WorkOverflow)?,
        )?;
        let mut instruction_count = 0_u64;
        for block in &selected_function.blocks {
            replay_add(
                &mut instruction_count,
                u64::try_from(block.instructions.len())
                    .map_err(|_| RecoveryClassificationError::WorkOverflow)?,
            )?;
            replay_add(&mut instruction_count, 1)?;
        }
        replay_add(&mut steps, instruction_count)?;
        replay_add(
            &mut steps,
            u64::try_from(ranges.plan().functions[index].virtual_registers.len())
                .map_err(|_| RecoveryClassificationError::WorkOverflow)?,
        )?;
        if spill_choices.plan().functions[index].choice.is_some() {
            replay_add(&mut candidates, 1)?;
            replay_add(&mut commits, 1)?;
        }
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: rules,
        candidates,
        validation_steps: steps,
        commits,
        iterations: 1,
    })
}

fn replay_add(target: &mut u64, amount: u64) -> Result<(), RecoveryClassificationError> {
    *target = target
        .checked_add(amount)
        .ok_or(RecoveryClassificationError::WorkOverflow)?;
    Ok(())
}
