use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::RegisterOperandAccess;
use omega_selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedTerminator, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use psi_core::{IntegerSign, ScalarType};

use crate::{
    FunctionRecoveryClassification, NoAdmittedRecoveryReason, PressureRecoveryClassification,
    RecoveryClassification, RecoveryClassificationError, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices, VirtualFixedConstraintSite,
};

pub(crate) fn compute_terminal_recovery_classifications<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    policy: RecoveryClassificationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<RecoveryClassificationPlan, RecoveryClassificationError> {
    validate_roots(selected, ranges, legality, spill_choices)?;
    if policy != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 {
        return Err(RecoveryClassificationError::UnsupportedPolicy);
    }
    let usage = required_usage(selected, ranges, spill_choices)?;
    if !usage.within(budget) {
        return Err(RecoveryClassificationError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let functions = selected
        .selected_plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .zip(&legality.plan().functions)
        .zip(&spill_choices.plan().functions)
        .enumerate()
        .map(|(function, (((selected, ranges), legality), choices))| {
            classify_function(function, selected, ranges, legality, choices)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecoveryClassificationPlan {
        selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
        allocator_availability: legality.receipt().allocator_availability(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn validate_roots(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
) -> Result<(), RecoveryClassificationError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment()
            != legality.receipt().register_environment()
        || selected.selected_plan().functions.len() != ranges.plan().functions.len()
        || selected.selected_plan().functions.len() != legality.plan().functions.len()
        || selected.selected_plan().functions.len() != spill_choices.plan().functions.len()
    {
        return Err(RecoveryClassificationError::RootMismatch);
    }
    Ok(())
}

fn classify_function(
    function: usize,
    selected: &omega_selected_instructions::SelectedFunction,
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
    let Some(choice) = &choices.choice else {
        return Ok(FunctionRecoveryClassification {
            machine: selected.machine,
            classification: None,
        });
    };
    let victim = selected
        .virtual_registers
        .iter()
        .find(|register| register.id == choice.selected_victim)
        .ok_or(RecoveryClassificationError::VictimMismatch {
            function,
            register: choice.selected_victim.0,
        })?;
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == victim.id && range.class == victim.class)
        .ok_or(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        })?;
    if !legality
        .virtual_registers
        .iter()
        .any(|row| row.virtual_register == victim.id && row.class == victim.class)
    {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let role = victim_role(function, choice)?;
    let classification = classify_victim(function, selected, ranges, choice, victim, range)?;
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

fn victim_role(
    function: usize,
    choice: &crate::SpillChoice,
) -> Result<RecoveryVictimRole, RecoveryClassificationError> {
    if choice.selected_victim == choice.incoming {
        let contender = choice
            .contenders
            .iter()
            .find(|row| row.virtual_register == choice.selected_victim)
            .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
        if contender.reclaimed_view.is_some() {
            return Err(RecoveryClassificationError::ChoiceMismatch { function });
        }
        return Ok(RecoveryVictimRole::Incoming);
    }
    let resident = choice
        .active_residents
        .iter()
        .find(|row| row.virtual_register == choice.selected_victim)
        .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
    let reclaimed_view = choice
        .contenders
        .iter()
        .find(|row| row.virtual_register == choice.selected_victim)
        .and_then(|row| row.reclaimed_view)
        .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
    Ok(RecoveryVictimRole::ActiveResident {
        current_view: resident.view,
        reclaimed_view,
    })
}

fn classify_victim(
    function: usize,
    selected: &omega_selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    choice: &crate::SpillChoice,
    victim: &omega_selected_instructions::VirtualRegister,
    range: &crate::VirtualLiveRange,
) -> Result<RecoveryClassification, RecoveryClassificationError> {
    if !is_fixed_unsigned_u64(victim.scalar_type) {
        return no_recovery(NoAdmittedRecoveryReason::UnsupportedScalarType);
    }
    let (defining_id, source_value) = match victim.origin {
        VirtualRegisterOrigin::EntryParameter { .. } => {
            return no_recovery(NoAdmittedRecoveryReason::EntryParameter);
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
    if range.fragments.len() != 1
        || range.fragments[0].block != choice.block
        || !range.edge_connectors.is_empty()
    {
        return no_recovery(NoAdmittedRecoveryReason::UnsupportedRangeShape);
    }
    if range.fixed_constraints.iter().any(|fixed| {
        matches!(
            fixed.site,
            VirtualFixedConstraintSite::Operand { point, .. } if point >= choice.point
        )
    }) {
        return no_recovery(NoAdmittedRecoveryReason::FutureFixedUse);
    }
    let defining = unique_definition(function, selected, victim.id, defining_id)?;
    match defining.kind {
        SelectedInstructionKind::ExactAddI64 { .. }
        | SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64 { .. } => {
            return no_recovery(NoAdmittedRecoveryReason::ProofBearingDefinition);
        }
        SelectedInstructionKind::MaterializeI64 { .. } => {}
        _ => return no_recovery(NoAdmittedRecoveryReason::NonMaterializeDefinition),
    }
    let SelectedInstructionKind::MaterializeI64 { value } = defining.kind else {
        unreachable!("materialize kind established above")
    };
    if defining.operands.len() != 1
        || defining.operands[0].virtual_register != victim.id
        || defining.operands[0].access != RegisterOperandAccess::Def
        || defining.provenance.values.as_slice() != [source_value]
        || defining.provenance.operations.len() != 1
        || !defining.provenance.edges.is_empty()
        || !defining.provenance.obligations.is_empty()
        || defining.provenance.fuel.is_empty()
        || !defining.provenance.fuel.iter().all(|fuel| {
            fuel.site
                == omega_optimization_unit::PsiProvenance::Operation(
                    defining.provenance.operations[0],
                )
        })
        || !matches!(victim.scalar_type, ScalarType::Integer(integer) if integer.admits(value))
    {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let future_uses = future_uses(function, selected, ranges, choice, victim.id, range)?;
    if future_uses.is_empty() {
        return no_recovery(NoAdmittedRecoveryReason::NoFutureUse);
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

fn unique_definition(
    function: usize,
    selected: &omega_selected_instructions::SelectedFunction,
    victim: VirtualRegisterId,
    expected: omega_selected_instructions::SelectedInstructionId,
) -> Result<&SelectedInstruction, RecoveryClassificationError> {
    let mut definitions = Vec::new();
    for block in &selected.blocks {
        for instruction in block_instructions(block) {
            if instruction.operands.iter().any(|operand| {
                operand.virtual_register == victim
                    && matches!(
                        operand.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    )
            }) {
                definitions.push(instruction);
            }
        }
    }
    if definitions.len() != 1 || definitions[0].id != expected {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.0,
        });
    }
    Ok(definitions[0])
}

fn future_uses(
    function: usize,
    selected: &omega_selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    choice: &crate::SpillChoice,
    victim: VirtualRegisterId,
    range: &crate::VirtualLiveRange,
) -> Result<Vec<RecoveryFutureUse>, RecoveryClassificationError> {
    let mut uses = Vec::new();
    for occurrence in &range.occurrences {
        if occurrence.point < choice.point || occurrence.access != RegisterOperandAccess::Use {
            continue;
        }
        let block = ranges
            .block_domains
            .iter()
            .find(|domain| domain.start <= occurrence.point && occurrence.point < domain.end)
            .ok_or(RecoveryClassificationError::VictimMismatch {
                function,
                register: victim.0,
            })?;
        if block.block != choice.block
            || !selected.blocks.iter().any(|candidate| {
                candidate.id == block.block
                    && block_instructions(candidate)
                        .into_iter()
                        .any(|instruction| {
                            instruction.id == occurrence.instruction
                                && instruction.operands.iter().any(|operand| {
                                    operand.operand == occurrence.operand
                                        && operand.virtual_register == victim
                                        && operand.access == RegisterOperandAccess::Use
                                        && operand.fixed_view.is_none()
                                })
                        })
            })
        {
            return Err(RecoveryClassificationError::VictimMismatch {
                function,
                register: victim.0,
            });
        }
        uses.push(RecoveryFutureUse {
            block: block.block,
            point: occurrence.point,
            instruction: occurrence.instruction,
            operand: occurrence.operand,
        });
    }
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn block_instructions(
    block: &omega_selected_instructions::SelectedBlock,
) -> Vec<&SelectedInstruction> {
    let terminator = match &block.terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    };
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator))
        .collect()
}

fn is_fixed_unsigned_u64(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Integer(integer)
            if !integer.is_address()
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    )
}

fn no_recovery(
    reason: NoAdmittedRecoveryReason,
) -> Result<RecoveryClassification, RecoveryClassificationError> {
    Ok(RecoveryClassification::NoAdmittedRecovery { reason })
}

fn required_usage(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    spill_choices: &ValidatedSpillChoices,
) -> Result<OptimizationWorkUsage, RecoveryClassificationError> {
    let mut usage = OptimizationWorkUsage {
        rule_evaluations: 0,
        candidates: 0,
        validation_steps: 0,
        commits: 0,
        iterations: 1,
    };
    for ((selected, ranges), choices) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .zip(&spill_choices.plan().functions)
    {
        checked_add(&mut usage.rule_evaluations, 1)?;
        checked_add(
            &mut usage.validation_steps,
            selected.virtual_registers.len() as u64,
        )?;
        let instruction_count = selected
            .blocks
            .iter()
            .map(|block| block.instructions.len() as u64 + 1)
            .sum::<u64>();
        checked_add(&mut usage.validation_steps, instruction_count)?;
        checked_add(
            &mut usage.validation_steps,
            ranges.virtual_registers.len() as u64,
        )?;
        if choices.choice.is_some() {
            checked_add(&mut usage.candidates, 1)?;
            checked_add(&mut usage.commits, 1)?;
        }
    }
    Ok(usage)
}

fn checked_add(target: &mut u64, amount: u64) -> Result<(), RecoveryClassificationError> {
    *target = target
        .checked_add(amount)
        .ok_or(RecoveryClassificationError::WorkOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
    use omega_register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
        RegisterViewId,
    };
    use omega_selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
        SelectedOperand, SelectedTerminator, VirtualRegister, VirtualRegisterId,
        VirtualRegisterOrigin,
    };
    use psi_core::{
        BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
        ScalarType, ValueId,
    };

    use super::*;
    use crate::{
        BlockPointDomain, FunctionAllocationLegality, FunctionLiveRanges, FunctionSpillChoices,
        LiveRangeFragment, LiveRangePoint, LivenessPosition, PressureContender, SpillChoice,
        VirtualLiveRange, VirtualOccurrence, VirtualPointLegality,
        VirtualRegisterAllocationLegality,
    };

    fn operand(register: u32, operand: u16, access: RegisterOperandAccess) -> SelectedOperand {
        SelectedOperand {
            operand,
            virtual_register: VirtualRegisterId(register),
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }

    fn fixture() -> (
        SelectedFunction,
        FunctionLiveRanges,
        FunctionAllocationLegality,
        FunctionSpillChoices,
    ) {
        let machine = MachineId::new(1).unwrap();
        let source_block = BlockId::new(1).unwrap();
        let key = RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 1,
        };
        let definitions = (0..3_u32)
            .map(|register| {
                let operation = OperationId::new(u64::from(register) + 1).unwrap();
                let source_value = ValueId::new(u64::from(register) + 1).unwrap();
                SelectedInstruction {
                    id: SelectedInstructionId(register),
                    kind: SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(u128::from(register) + 7),
                    },
                    constraint: key,
                    operands: vec![operand(register, 0, RegisterOperandAccess::Def)],
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
                            units: 1,
                        }],
                    },
                }
            })
            .collect::<Vec<_>>();
        let returned = SelectedInstruction {
            id: SelectedInstructionId(3),
            kind: SelectedInstructionKind::ReturnI64,
            constraint: key,
            operands: (0..3_u32)
                .map(|register| operand(register, register as u16, RegisterOperandAccess::Use))
                .collect(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance {
                values: (1..=3).map(|id| ValueId::new(id).unwrap()).collect(),
                ..Default::default()
            },
        };
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let selected = SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            entry_block: SelectedBlockId(0),
            virtual_registers: (0..3_u32)
                .map(|register| VirtualRegister {
                    id: VirtualRegisterId(register),
                    scalar_type,
                    class: RegisterClassId(0),
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(register),
                        source_value: ValueId::new(u64::from(register) + 1).unwrap(),
                    },
                    definition_site: ValueDefinitionSite::Node {
                        block: source_block,
                        node: register,
                    },
                    entry_fixed_view: None,
                })
                .collect(),
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                source_block,
                instructions: definitions,
                terminator: SelectedTerminator::Return {
                    instruction: returned,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        };
        let ranges = FunctionLiveRanges {
            machine,
            block_domains: vec![BlockPointDomain {
                block: SelectedBlockId(0),
                source_block,
                start: LiveRangePoint(0),
                end: LiveRangePoint(8),
            }],
            virtual_registers: (0..3_u32)
                .map(|register| VirtualLiveRange {
                    virtual_register: VirtualRegisterId(register),
                    class: RegisterClassId(0),
                    occurrences: vec![
                        VirtualOccurrence {
                            position: LivenessPosition(register),
                            point: LiveRangePoint(register * 2 + 1),
                            instruction: SelectedInstructionId(register),
                            operand: 0,
                            access: RegisterOperandAccess::Def,
                        },
                        VirtualOccurrence {
                            position: LivenessPosition(3),
                            point: LiveRangePoint(6),
                            instruction: SelectedInstructionId(3),
                            operand: register as u16,
                            access: RegisterOperandAccess::Use,
                        },
                    ],
                    fixed_constraints: Vec::new(),
                    fragments: vec![LiveRangeFragment {
                        block: SelectedBlockId(0),
                        start: LiveRangePoint(register * 2 + 1),
                        end: LiveRangePoint(7),
                    }],
                    edge_connectors: Vec::new(),
                })
                .collect(),
            tied_pairs: Vec::new(),
            early_clobbers: Vec::new(),
            architectural_units: Vec::new(),
            interference: [(0, 1), (0, 2), (1, 2)]
                .into_iter()
                .map(|(lower, higher)| crate::VirtualInterference {
                    lower: VirtualRegisterId(lower),
                    higher: VirtualRegisterId(higher),
                })
                .collect(),
        };
        let legality = FunctionAllocationLegality {
            machine,
            virtual_registers: (0..3_u32)
                .map(|register| VirtualRegisterAllocationLegality {
                    virtual_register: VirtualRegisterId(register),
                    class: RegisterClassId(0),
                    points: (register * 2 + 1..=6)
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
        let choices = FunctionSpillChoices {
            machine,
            choice: Some(SpillChoice {
                block: SelectedBlockId(0),
                point: LiveRangePoint(5),
                incoming: VirtualRegisterId(2),
                incoming_class: RegisterClassId(0),
                incoming_common_candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                active_residents: vec![
                    crate::PressureResident {
                        virtual_register: VirtualRegisterId(0),
                        class: RegisterClassId(0),
                        start: LiveRangePoint(1),
                        exclusive_end: LiveRangePoint(7),
                        view: RegisterViewId(0),
                    },
                    crate::PressureResident {
                        virtual_register: VirtualRegisterId(1),
                        class: RegisterClassId(0),
                        start: LiveRangePoint(3),
                        exclusive_end: LiveRangePoint(7),
                        view: RegisterViewId(1),
                    },
                ],
                contenders: vec![
                    PressureContender {
                        virtual_register: VirtualRegisterId(0),
                        exclusive_end: LiveRangePoint(7),
                        reclaimed_view: Some(RegisterViewId(0)),
                    },
                    PressureContender {
                        virtual_register: VirtualRegisterId(1),
                        exclusive_end: LiveRangePoint(7),
                        reclaimed_view: Some(RegisterViewId(1)),
                    },
                    PressureContender {
                        virtual_register: VirtualRegisterId(2),
                        exclusive_end: LiveRangePoint(7),
                        reclaimed_view: None,
                    },
                ],
                selected_victim: VirtualRegisterId(2),
            }),
        };
        (selected, ranges, legality, choices)
    }

    #[test]
    fn incoming_literal_is_classified_identically_by_compute_and_replay() {
        let (selected, ranges, legality, choices) = fixture();
        let computed = classify_function(0, &selected, &ranges, &legality, &choices).unwrap();
        let replayed = crate::recovery_classification_validate::replay_function_for_test(
            0, &selected, &ranges, &legality, &choices,
        )
        .unwrap();
        assert_eq!(computed, replayed);
        let row = computed.classification.unwrap();
        assert_eq!(row.role, RecoveryVictimRole::Incoming);
        assert!(matches!(
            row.classification,
            RecoveryClassification::ImmediateU64RematerializationCandidate {
                value: IntegerValue::Unsigned(9),
                ref future_uses,
                ..
            } if future_uses.len() == 1
        ));
    }

    #[test]
    fn honest_unsupported_and_corrupt_provenance_are_distinct() {
        let (mut selected, ranges, legality, choices) = fixture();
        selected.virtual_registers[2].scalar_type = ScalarType::Boolean;
        let result = classify_function(0, &selected, &ranges, &legality, &choices).unwrap();
        assert!(matches!(
            result.classification.unwrap().classification,
            RecoveryClassification::NoAdmittedRecovery {
                reason: NoAdmittedRecoveryReason::UnsupportedScalarType
            }
        ));

        let (mut selected, ranges, legality, choices) = fixture();
        selected.blocks[0].instructions[2].provenance.values[0] = ValueId::new(99).unwrap();
        let expected = Err(RecoveryClassificationError::VictimMismatch {
            function: 0,
            register: 2,
        });
        assert_eq!(
            classify_function(0, &selected, &ranges, &legality, &choices),
            expected
        );
        assert_eq!(
            crate::recovery_classification_validate::replay_function_for_test(
                0, &selected, &ranges, &legality, &choices,
            ),
            expected
        );
    }
}
