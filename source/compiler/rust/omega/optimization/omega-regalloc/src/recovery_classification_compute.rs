use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::RegisterOperandAccess;
use omega_terminal_selected_instructions::{
    TerminalSelectedInstruction, TerminalSelectedInstructionKind, TerminalSelectedTerminator,
    TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use psi_core::{IntegerSign, ScalarType};

use crate::{
    TerminalFunctionRecoveryClassification, TerminalNoAdmittedRecoveryReason,
    TerminalPressureRecoveryClassification, TerminalRecoveryClassification,
    TerminalRecoveryClassificationError, TerminalRecoveryClassificationPlan,
    TerminalRecoveryClassificationPolicy, TerminalRecoveryFutureUse, TerminalRecoveryVictimRole,
    TerminalVirtualFixedConstraintSite, ValidatedTerminalAllocationLegality,
    ValidatedTerminalLiveRanges, ValidatedTerminalSelectedAnalysis, ValidatedTerminalSpillChoices,
};

pub(crate) fn compute_terminal_recovery_classifications<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
    policy: TerminalRecoveryClassificationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<TerminalRecoveryClassificationPlan, TerminalRecoveryClassificationError> {
    validate_roots(selected, ranges, legality, spill_choices)?;
    if policy != TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 {
        return Err(TerminalRecoveryClassificationError::UnsupportedPolicy);
    }
    let usage = required_usage(selected, ranges, spill_choices)?;
    if !usage.within(budget) {
        return Err(TerminalRecoveryClassificationError::BudgetExceeded {
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
    Ok(TerminalRecoveryClassificationPlan {
        selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn validate_roots(
    selected: &impl ValidatedTerminalSelectedAnalysis,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    spill_choices: &ValidatedTerminalSpillChoices,
) -> Result<(), TerminalRecoveryClassificationError> {
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
        return Err(TerminalRecoveryClassificationError::RootMismatch);
    }
    Ok(())
}

fn classify_function(
    function: usize,
    selected: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    ranges: &crate::TerminalFunctionLiveRanges,
    legality: &crate::TerminalFunctionAllocationLegality,
    choices: &crate::TerminalFunctionSpillChoices,
) -> Result<TerminalFunctionRecoveryClassification, TerminalRecoveryClassificationError> {
    if selected.machine != ranges.machine
        || selected.machine != legality.machine
        || selected.machine != choices.machine
        || selected.virtual_registers.len() != ranges.virtual_registers.len()
        || selected.virtual_registers.len() != legality.virtual_registers.len()
    {
        return Err(TerminalRecoveryClassificationError::FunctionMismatch { function });
    }
    let Some(choice) = &choices.choice else {
        return Ok(TerminalFunctionRecoveryClassification {
            machine: selected.machine,
            classification: None,
        });
    };
    let victim = selected
        .virtual_registers
        .iter()
        .find(|register| register.id == choice.selected_victim)
        .ok_or(TerminalRecoveryClassificationError::VictimMismatch {
            function,
            register: choice.selected_victim.0,
        })?;
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == victim.id && range.class == victim.class)
        .ok_or(TerminalRecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        })?;
    if !legality
        .virtual_registers
        .iter()
        .any(|row| row.virtual_register == victim.id && row.class == victim.class)
    {
        return Err(TerminalRecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let role = victim_role(function, choice)?;
    let classification = classify_victim(function, selected, ranges, choice, victim, range)?;
    Ok(TerminalFunctionRecoveryClassification {
        machine: selected.machine,
        classification: Some(TerminalPressureRecoveryClassification {
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
    choice: &crate::TerminalSpillChoice,
) -> Result<TerminalRecoveryVictimRole, TerminalRecoveryClassificationError> {
    if choice.selected_victim == choice.incoming {
        let contender = choice
            .contenders
            .iter()
            .find(|row| row.virtual_register == choice.selected_victim)
            .ok_or(TerminalRecoveryClassificationError::ChoiceMismatch { function })?;
        if contender.reclaimed_view.is_some() {
            return Err(TerminalRecoveryClassificationError::ChoiceMismatch { function });
        }
        return Ok(TerminalRecoveryVictimRole::Incoming);
    }
    let resident = choice
        .active_residents
        .iter()
        .find(|row| row.virtual_register == choice.selected_victim)
        .ok_or(TerminalRecoveryClassificationError::ChoiceMismatch { function })?;
    let reclaimed_view = choice
        .contenders
        .iter()
        .find(|row| row.virtual_register == choice.selected_victim)
        .and_then(|row| row.reclaimed_view)
        .ok_or(TerminalRecoveryClassificationError::ChoiceMismatch { function })?;
    Ok(TerminalRecoveryVictimRole::ActiveResident {
        current_view: resident.view,
        reclaimed_view,
    })
}

fn classify_victim(
    function: usize,
    selected: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    ranges: &crate::TerminalFunctionLiveRanges,
    choice: &crate::TerminalSpillChoice,
    victim: &omega_terminal_selected_instructions::TerminalVirtualRegister,
    range: &crate::TerminalVirtualLiveRange,
) -> Result<TerminalRecoveryClassification, TerminalRecoveryClassificationError> {
    if !is_fixed_unsigned_u64(victim.scalar_type) {
        return no_recovery(TerminalNoAdmittedRecoveryReason::UnsupportedScalarType);
    }
    let TerminalVirtualRegisterOrigin::InstructionResult {
        instruction: defining_id,
        source_value,
    } = victim.origin
    else {
        return no_recovery(TerminalNoAdmittedRecoveryReason::EntryParameter);
    };
    if range.fragments.len() != 1
        || range.fragments[0].block != choice.block
        || !range.edge_connectors.is_empty()
    {
        return no_recovery(TerminalNoAdmittedRecoveryReason::UnsupportedRangeShape);
    }
    if range.fixed_constraints.iter().any(|fixed| {
        matches!(
            fixed.site,
            TerminalVirtualFixedConstraintSite::Operand { point, .. } if point >= choice.point
        )
    }) {
        return no_recovery(TerminalNoAdmittedRecoveryReason::FutureFixedUse);
    }
    let defining = unique_definition(function, selected, victim.id, defining_id)?;
    match defining.kind {
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            return no_recovery(TerminalNoAdmittedRecoveryReason::ProofBearingDefinition);
        }
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => {}
        _ => return no_recovery(TerminalNoAdmittedRecoveryReason::NonMaterializeDefinition),
    }
    let TerminalSelectedInstructionKind::MaterializeI64 { value } = defining.kind else {
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
        return Err(TerminalRecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let future_uses = future_uses(function, selected, ranges, choice, victim.id, range)?;
    if future_uses.is_empty() {
        return no_recovery(TerminalNoAdmittedRecoveryReason::NoFutureUse);
    }
    Ok(
        TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
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
    selected: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    victim: TerminalVirtualRegisterId,
    expected: omega_terminal_selected_instructions::TerminalSelectedInstructionId,
) -> Result<&TerminalSelectedInstruction, TerminalRecoveryClassificationError> {
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
        return Err(TerminalRecoveryClassificationError::VictimMismatch {
            function,
            register: victim.0,
        });
    }
    Ok(definitions[0])
}

fn future_uses(
    function: usize,
    selected: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    ranges: &crate::TerminalFunctionLiveRanges,
    choice: &crate::TerminalSpillChoice,
    victim: TerminalVirtualRegisterId,
    range: &crate::TerminalVirtualLiveRange,
) -> Result<Vec<TerminalRecoveryFutureUse>, TerminalRecoveryClassificationError> {
    let mut uses = Vec::new();
    for occurrence in &range.occurrences {
        if occurrence.point < choice.point || occurrence.access != RegisterOperandAccess::Use {
            continue;
        }
        let block = ranges
            .block_domains
            .iter()
            .find(|domain| domain.start <= occurrence.point && occurrence.point < domain.end)
            .ok_or(TerminalRecoveryClassificationError::VictimMismatch {
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
            return Err(TerminalRecoveryClassificationError::VictimMismatch {
                function,
                register: victim.0,
            });
        }
        uses.push(TerminalRecoveryFutureUse {
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
    block: &omega_terminal_selected_instructions::TerminalSelectedBlock,
) -> Vec<&TerminalSelectedInstruction> {
    let terminator = match &block.terminator {
        TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
        | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
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
    reason: TerminalNoAdmittedRecoveryReason,
) -> Result<TerminalRecoveryClassification, TerminalRecoveryClassificationError> {
    Ok(TerminalRecoveryClassification::NoAdmittedRecovery { reason })
}

fn required_usage(
    selected: &impl ValidatedTerminalSelectedAnalysis,
    ranges: &ValidatedTerminalLiveRanges,
    spill_choices: &ValidatedTerminalSpillChoices,
) -> Result<OptimizationWorkUsage, TerminalRecoveryClassificationError> {
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

fn checked_add(target: &mut u64, amount: u64) -> Result<(), TerminalRecoveryClassificationError> {
    *target = target
        .checked_add(amount)
        .ok_or(TerminalRecoveryClassificationError::WorkOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
    use omega_register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
        RegisterViewId,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlock, TerminalSelectedBlockId, TerminalSelectedFunction,
        TerminalSelectedInstruction, TerminalSelectedInstructionId,
        TerminalSelectedInstructionKind, TerminalSelectedInstructionProvenance,
        TerminalSelectedOperand, TerminalSelectedTerminator, TerminalVirtualRegister,
        TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
    };
    use psi_core::{
        BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
        ScalarType, ValueId,
    };

    use super::*;
    use crate::{
        TerminalBlockPointDomain, TerminalFunctionAllocationLegality, TerminalFunctionLiveRanges,
        TerminalFunctionSpillChoices, TerminalLiveRangeFragment, TerminalLiveRangePoint,
        TerminalLivenessPosition, TerminalPressureContender, TerminalSpillChoice,
        TerminalVirtualLiveRange, TerminalVirtualOccurrence, TerminalVirtualPointLegality,
        TerminalVirtualRegisterAllocationLegality,
    };

    fn operand(
        register: u32,
        operand: u16,
        access: RegisterOperandAccess,
    ) -> TerminalSelectedOperand {
        TerminalSelectedOperand {
            operand,
            virtual_register: TerminalVirtualRegisterId(register),
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }

    fn fixture() -> (
        TerminalSelectedFunction,
        TerminalFunctionLiveRanges,
        TerminalFunctionAllocationLegality,
        TerminalFunctionSpillChoices,
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
                TerminalSelectedInstruction {
                    id: TerminalSelectedInstructionId(register),
                    kind: TerminalSelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(u128::from(register) + 7),
                    },
                    constraint: key,
                    operands: vec![operand(register, 0, RegisterOperandAccess::Def)],
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
                            units: 1,
                        }],
                    },
                }
            })
            .collect::<Vec<_>>();
        let returned = TerminalSelectedInstruction {
            id: TerminalSelectedInstructionId(3),
            kind: TerminalSelectedInstructionKind::ReturnI64,
            constraint: key,
            operands: (0..3_u32)
                .map(|register| operand(register, register as u16, RegisterOperandAccess::Use))
                .collect(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: TerminalSelectedInstructionProvenance {
                values: (1..=3).map(|id| ValueId::new(id).unwrap()).collect(),
                ..Default::default()
            },
        };
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let selected = TerminalSelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            entry_block: TerminalSelectedBlockId(0),
            virtual_registers: (0..3_u32)
                .map(|register| TerminalVirtualRegister {
                    id: TerminalVirtualRegisterId(register),
                    scalar_type,
                    class: RegisterClassId(0),
                    origin: TerminalVirtualRegisterOrigin::InstructionResult {
                        instruction: TerminalSelectedInstructionId(register),
                        source_value: ValueId::new(u64::from(register) + 1).unwrap(),
                    },
                    definition_site: ValueDefinitionSite::Node {
                        block: source_block,
                        node: register,
                    },
                    entry_fixed_view: None,
                })
                .collect(),
            blocks: vec![TerminalSelectedBlock {
                id: TerminalSelectedBlockId(0),
                source_block,
                instructions: definitions,
                terminator: TerminalSelectedTerminator::Return {
                    instruction: returned,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        };
        let ranges = TerminalFunctionLiveRanges {
            machine,
            block_domains: vec![TerminalBlockPointDomain {
                block: TerminalSelectedBlockId(0),
                source_block,
                start: TerminalLiveRangePoint(0),
                end: TerminalLiveRangePoint(8),
            }],
            virtual_registers: (0..3_u32)
                .map(|register| TerminalVirtualLiveRange {
                    virtual_register: TerminalVirtualRegisterId(register),
                    class: RegisterClassId(0),
                    occurrences: vec![
                        TerminalVirtualOccurrence {
                            position: TerminalLivenessPosition(register),
                            point: TerminalLiveRangePoint(register * 2 + 1),
                            instruction: TerminalSelectedInstructionId(register),
                            operand: 0,
                            access: RegisterOperandAccess::Def,
                        },
                        TerminalVirtualOccurrence {
                            position: TerminalLivenessPosition(3),
                            point: TerminalLiveRangePoint(6),
                            instruction: TerminalSelectedInstructionId(3),
                            operand: register as u16,
                            access: RegisterOperandAccess::Use,
                        },
                    ],
                    fixed_constraints: Vec::new(),
                    fragments: vec![TerminalLiveRangeFragment {
                        block: TerminalSelectedBlockId(0),
                        start: TerminalLiveRangePoint(register * 2 + 1),
                        end: TerminalLiveRangePoint(7),
                    }],
                    edge_connectors: Vec::new(),
                })
                .collect(),
            architectural_units: Vec::new(),
            interference: [(0, 1), (0, 2), (1, 2)]
                .into_iter()
                .map(|(lower, higher)| crate::TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(lower),
                    higher: TerminalVirtualRegisterId(higher),
                })
                .collect(),
        };
        let legality = TerminalFunctionAllocationLegality {
            machine,
            virtual_registers: (0..3_u32)
                .map(|register| TerminalVirtualRegisterAllocationLegality {
                    virtual_register: TerminalVirtualRegisterId(register),
                    class: RegisterClassId(0),
                    points: (register * 2 + 1..=6)
                        .map(|point| TerminalVirtualPointLegality {
                            block: TerminalSelectedBlockId(0),
                            point: TerminalLiveRangePoint(point),
                            candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                        })
                        .collect(),
                    entry_transitions: Vec::new(),
                })
                .collect(),
        };
        let choices = TerminalFunctionSpillChoices {
            machine,
            choice: Some(TerminalSpillChoice {
                block: TerminalSelectedBlockId(0),
                point: TerminalLiveRangePoint(5),
                incoming: TerminalVirtualRegisterId(2),
                incoming_class: RegisterClassId(0),
                incoming_common_candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                active_residents: vec![
                    crate::TerminalPressureResident {
                        virtual_register: TerminalVirtualRegisterId(0),
                        class: RegisterClassId(0),
                        start: TerminalLiveRangePoint(1),
                        exclusive_end: TerminalLiveRangePoint(7),
                        view: RegisterViewId(0),
                    },
                    crate::TerminalPressureResident {
                        virtual_register: TerminalVirtualRegisterId(1),
                        class: RegisterClassId(0),
                        start: TerminalLiveRangePoint(3),
                        exclusive_end: TerminalLiveRangePoint(7),
                        view: RegisterViewId(1),
                    },
                ],
                contenders: vec![
                    TerminalPressureContender {
                        virtual_register: TerminalVirtualRegisterId(0),
                        exclusive_end: TerminalLiveRangePoint(7),
                        reclaimed_view: Some(RegisterViewId(0)),
                    },
                    TerminalPressureContender {
                        virtual_register: TerminalVirtualRegisterId(1),
                        exclusive_end: TerminalLiveRangePoint(7),
                        reclaimed_view: Some(RegisterViewId(1)),
                    },
                    TerminalPressureContender {
                        virtual_register: TerminalVirtualRegisterId(2),
                        exclusive_end: TerminalLiveRangePoint(7),
                        reclaimed_view: None,
                    },
                ],
                selected_victim: TerminalVirtualRegisterId(2),
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
        assert_eq!(row.role, TerminalRecoveryVictimRole::Incoming);
        assert!(matches!(
            row.classification,
            TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
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
            TerminalRecoveryClassification::NoAdmittedRecovery {
                reason: TerminalNoAdmittedRecoveryReason::UnsupportedScalarType
            }
        ));

        let (mut selected, ranges, legality, choices) = fixture();
        selected.blocks[0].instructions[2].provenance.values[0] = ValueId::new(99).unwrap();
        let expected = Err(TerminalRecoveryClassificationError::VictimMismatch {
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
