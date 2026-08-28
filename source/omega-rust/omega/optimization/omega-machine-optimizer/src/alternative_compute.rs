use std::collections::BTreeSet;

use omega_regalloc::{
    ValidatedPostAllocationOptimizationManifest, ValidatedTerminalAllocationLegality,
    ValidatedTerminalLiveRanges, ValidatedTerminalRegisterHomes, ValidatedTerminalSelectedAnalysis,
};
use omega_register_model::{
    RegisterOperandAccess, TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability, TerminalSelectedBlock,
    TerminalSelectedInstruction, TerminalSelectedStructuralUnitFunction,
};

use crate::{
    TerminalBlockMachineEffects, TerminalInstructionMachineEffects,
    TerminalMachineAlternativeChoiceRule, TerminalPhysicalOperandFootprint,
    TerminalPostAllocationMachineBlock, TerminalPostAllocationMachineError,
    TerminalPostAllocationMachineFunction, TerminalPostAllocationMachineIdentity,
    TerminalPostAllocationMachineInstruction, TerminalPostAllocationMachinePlan,
    TerminalPostAllocationStructuralUnitFunction, TerminalStructuralUnitFunctionMachineEffects,
    ValidatedTerminalPreAllocationMachineEffects, terminal_post_allocation_machine_identity,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_post_allocation_machine_plan<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    effects: &ValidatedTerminalPreAllocationMachineEffects,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalPostAllocationMachinePlan, TerminalPostAllocationMachineError> {
    validate_roots(
        selected,
        effects,
        ranges,
        legality,
        homes,
        manifest,
        register_environment,
        physical,
        constraints,
    )?;
    let selected_plan = selected.selected_plan();
    let functions = selected_plan
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            let effect_function = effects
                .plan()
                .functions
                .get(function_index)
                .filter(|effects| effects.machine == function.machine)
                .ok_or(TerminalPostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                })?;
            let home_function = homes
                .plan()
                .functions
                .iter()
                .find(|homes| homes.machine == function.machine)
                .ok_or(TerminalPostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                })?;
            if effect_function.blocks.len() != function.blocks.len() {
                return Err(TerminalPostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                });
            }
            let blocks = function
                .blocks
                .iter()
                .enumerate()
                .map(|(block_index, block)| {
                    build_block(
                        function_index,
                        block_index,
                        block,
                        &effect_function.blocks[block_index],
                        home_function,
                        physical,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TerminalPostAllocationMachineFunction {
                machine: function.machine,
                blocks,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let structural_unit_functions = selected_plan
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(structural_index, function)| {
            let effect_function = unique_structural_effect(effects, function.machine)?;
            let home_function = unique_structural_home(homes, function.machine)?;
            build_structural_function(
                selected_plan.functions.len() + structural_index,
                function,
                effect_function,
                home_function,
                physical,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut plan = TerminalPostAllocationMachinePlan {
        identity: TerminalPostAllocationMachineIdentity::from_bytes([0; 32]),
        selected: selected.selected_identity(),
        effects: effects.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        target: selected_plan.target,
        register_environment,
        physical_register_model: physical.identity(),
        register_constraints: effects.plan().register_constraints,
        machine_effect_catalog: effects.plan().machine_effect_catalog,
        choice_rule: TerminalMachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions,
        structural_unit_functions,
    };
    plan.identity = terminal_post_allocation_machine_identity(&plan);
    Ok(plan)
}

fn build_structural_function(
    function_index: usize,
    selected: &TerminalSelectedStructuralUnitFunction,
    effects: &TerminalStructuralUnitFunctionMachineEffects,
    homes: &omega_regalloc::TerminalFunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalPostAllocationStructuralUnitFunction, TerminalPostAllocationMachineError> {
    if effects.machine != selected.machine
        || effects.block != selected.entry_block
        || effects.return_effect != selected.terminator.effect
        || effects.return_ownership != selected.terminator.ownership
        || !structural_call_matches(selected, effects)
    {
        return Err(
            TerminalPostAllocationMachineError::StructuralFunctionMismatch {
                machine: selected.machine,
            },
        );
    }
    if homes.machine != selected.machine || !homes.assignments.is_empty() {
        return Err(
            TerminalPostAllocationMachineError::StructuralAllocationMismatch {
                machine: selected.machine,
            },
        );
    }
    let return_instruction = build_instruction(
        function_index,
        &selected.terminator.instruction,
        &effects.return_instruction,
        homes,
        physical,
    )?;
    Ok(TerminalPostAllocationStructuralUnitFunction {
        machine: selected.machine,
        block: selected.entry_block,
        call: effects.call.clone(),
        return_instruction,
        return_provenance: selected.terminator.instruction.provenance.clone(),
        return_effect: selected.terminator.effect,
        return_ownership: selected.terminator.ownership.clone(),
    })
}

fn structural_call_matches(
    selected: &TerminalSelectedStructuralUnitFunction,
    effects: &TerminalStructuralUnitFunctionMachineEffects,
) -> bool {
    match (&selected.call, &effects.call) {
        (None, None) => true,
        (Some(selected), Some(effects)) => {
            effects.instruction == selected.id
                && effects.operation == selected.operation
                && effects.callee == selected.callee
                && effects.constraint == selected.constraint
                && effects.unit_uses == selected.implicit_uses
                && effects.unit_defs == selected.implicit_defs
                && effects.unit_clobbers == selected.clobbers
                && effects.layout == selected.layout
                && effects.effect == selected.effect
                && effects.ownership == selected.ownership
                && effects.claim_transfers == selected.claim_transfers
                && effects.provenance == selected.provenance
                && effects.declaration.constraint == selected.constraint
        }
        _ => false,
    }
}

fn unique_structural_effect(
    effects: &ValidatedTerminalPreAllocationMachineEffects,
    machine: psi_core::MachineId,
) -> Result<&TerminalStructuralUnitFunctionMachineEffects, TerminalPostAllocationMachineError> {
    let matches = effects
        .plan()
        .structural_unit_functions
        .iter()
        .filter(|function| function.machine == machine)
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(TerminalPostAllocationMachineError::StructuralFunctionMismatch { machine });
    };
    Ok(*function)
}

fn unique_structural_home(
    homes: &ValidatedTerminalRegisterHomes,
    machine: psi_core::MachineId,
) -> Result<&omega_regalloc::TerminalFunctionRegisterHomes, TerminalPostAllocationMachineError> {
    let matches = homes
        .plan()
        .structural_unit_functions
        .iter()
        .filter(|function| function.machine == machine)
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(TerminalPostAllocationMachineError::StructuralAllocationMismatch { machine });
    };
    Ok(*function)
}

fn build_block(
    function_index: usize,
    block_index: usize,
    selected: &TerminalSelectedBlock,
    effects: &TerminalBlockMachineEffects,
    homes: &omega_regalloc::TerminalFunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalPostAllocationMachineBlock, TerminalPostAllocationMachineError> {
    if effects.block != selected.id {
        return Err(TerminalPostAllocationMachineError::BlockMismatch {
            function: function_index,
            block: block_index,
        });
    }
    let selected_instructions = selected_instructions(selected).collect::<Vec<_>>();
    if effects.instructions.len() != selected_instructions.len() {
        return Err(TerminalPostAllocationMachineError::BlockMismatch {
            function: function_index,
            block: block_index,
        });
    }
    let instructions = selected_instructions
        .into_iter()
        .zip(&effects.instructions)
        .map(|(selected, effects)| {
            build_instruction(function_index, selected, effects, homes, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalPostAllocationMachineBlock {
        block: selected.id,
        instructions,
    })
}

fn build_instruction(
    function_index: usize,
    selected: &TerminalSelectedInstruction,
    effects: &TerminalInstructionMachineEffects,
    homes: &omega_regalloc::TerminalFunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalPostAllocationMachineInstruction, TerminalPostAllocationMachineError> {
    if effects.instruction != selected.id || effects.kind != selected.kind {
        return Err(TerminalPostAllocationMachineError::InstructionMismatch {
            function: function_index,
            instruction: selected.id.0,
        });
    }
    let operands = selected
        .operands
        .iter()
        .map(|operand| {
            let home = homes
                .assignments
                .iter()
                .find(|home| home.virtual_register == operand.virtual_register)
                .ok_or(TerminalPostAllocationMachineError::MissingHome {
                    function: function_index,
                    register: operand.virtual_register.0,
                })?;
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == home.view)
                .ok_or(TerminalPostAllocationMachineError::UnknownView {
                    function: function_index,
                    register: operand.virtual_register.0,
                    view: home.view.0,
                })?;
            if home.class != operand.class || view.class != operand.class {
                return Err(TerminalPostAllocationMachineError::HomeClassMismatch {
                    function: function_index,
                    register: operand.virtual_register.0,
                });
            }
            if operand.fixed_view.is_some_and(|fixed| fixed != home.view) {
                return Err(TerminalPostAllocationMachineError::HomeClassMismatch {
                    function: function_index,
                    register: operand.virtual_register.0,
                });
            }
            let reads = reads(operand.access);
            let writes = writes(operand.access);
            Ok(TerminalPhysicalOperandFootprint {
                operand: operand.operand,
                virtual_register: operand.virtual_register,
                class: operand.class,
                view: home.view,
                access: operand.access,
                storage_units: view.units.clone(),
                read_units: if reads {
                    view.units.clone()
                } else {
                    Vec::new()
                },
                write_units: if writes {
                    view.write_units.clone()
                } else {
                    Vec::new()
                },
                write_semantics: writes.then_some(view.write_semantics),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let alternative =
        choose_alternative(selected.id.0, &operands, &effects.alternatives, physical)?;
    let mut unit_uses = effects.unit_uses.iter().copied().collect::<BTreeSet<_>>();
    let mut unit_defs = effects.unit_defs.iter().copied().collect::<BTreeSet<_>>();
    for operand in &operands {
        unit_uses.extend(&operand.read_units);
        unit_defs.extend(&operand.write_units);
    }
    Ok(TerminalPostAllocationMachineInstruction {
        instruction: selected.id,
        alternative,
        operands,
        implicit_unit_uses: effects.unit_uses.clone(),
        implicit_unit_defs: effects.unit_defs.clone(),
        implicit_unit_clobbers: effects.unit_clobbers.clone(),
        unit_uses: unit_uses.into_iter().collect(),
        unit_defs: unit_defs.into_iter().collect(),
        unit_clobbers: effects.unit_clobbers.clone(),
    })
}

fn choose_alternative(
    instruction: u32,
    operands: &[TerminalPhysicalOperandFootprint],
    alternatives: &[TerminalMachineAlternative],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalMachineAlternative, TerminalPostAllocationMachineError> {
    let mut applicable = Vec::new();
    for alternative in alternatives {
        if applicability(instruction, operands, alternative.applicability, physical)? {
            applicable.push(alternative.clone());
        }
    }
    match applicable.as_slice() {
        [alternative] => Ok(alternative.clone()),
        [] => Err(TerminalPostAllocationMachineError::NoApplicableAlternative { instruction }),
        _ => {
            Err(TerminalPostAllocationMachineError::AmbiguousApplicableAlternatives { instruction })
        }
    }
}

fn applicability(
    instruction: u32,
    operands: &[TerminalPhysicalOperandFootprint],
    applicability: TerminalMachineAlternativeApplicability,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, TerminalPostAllocationMachineError> {
    let view = |operand| {
        operands
            .iter()
            .find(|candidate| candidate.operand == operand)
            .map(|operand| operand.view)
            .ok_or(
                TerminalPostAllocationMachineError::MissingApplicabilityOperand {
                    instruction,
                    operand,
                },
            )
    };
    let aliases = |left, right| physical.model().aliases(left, right);
    Ok(match applicability {
        TerminalMachineAlternativeApplicability::Always => true,
        TerminalMachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            aliases(view(result)?, view(operand)?)
        }
        TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let result = view(result)?;
            aliases(result, view(aliased_operand)?) && !aliases(result, view(distinct_operand)?)
        }
        TerminalMachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            aliases(result, view(left)?) && aliases(result, view(right)?)
        }
        TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            !aliases(result, view(left)?) && !aliases(result, view(right)?)
        }
        TerminalMachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left,
            right,
            excluded_view,
        } => !aliases(view(left)?, excluded_view) || !aliases(view(right)?, excluded_view),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedTerminalPreAllocationMachineEffects,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<(), TerminalPostAllocationMachineError> {
    validate_structural_allocation(selected, effects, ranges, legality, homes)?;
    if effects.receipt().selected() != selected.selected_identity()
        || ranges.receipt().selected() != selected.selected_identity()
    {
        return Err(TerminalPostAllocationMachineError::SelectedRootMismatch);
    }
    if effects.plan().optimization_unit != selected.optimization_unit_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
    {
        return Err(TerminalPostAllocationMachineError::OptimizationUnitMismatch);
    }
    if effects.plan().fuel_schedule != selected.fuel_schedule_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
    {
        return Err(TerminalPostAllocationMachineError::FuelScheduleMismatch);
    }
    if legality.receipt().ranges() != ranges.receipt().identity() {
        return Err(TerminalPostAllocationMachineError::RangeRootMismatch);
    }
    if homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
    {
        return Err(TerminalPostAllocationMachineError::HomeRootMismatch);
    }
    if effects.plan().register_environment != register_environment
        || legality.receipt().register_environment() != register_environment
        || homes.receipt().register_environment() != register_environment
    {
        return Err(TerminalPostAllocationMachineError::RegisterEnvironmentMismatch);
    }
    let record = manifest.record();
    if record.target != selected.selected_plan().target
        || record.selected != selected.selected_identity()
        || record.ranges != ranges.receipt().identity()
        || record.legality != legality.receipt().identity()
        || record.homes != homes.receipt().identity()
        || record.register_environment != register_environment
    {
        return Err(TerminalPostAllocationMachineError::PostAllocationManifestMismatch);
    }
    if effects.plan().target != selected.selected_plan().target {
        return Err(TerminalPostAllocationMachineError::TargetMismatch);
    }
    if physical.model().architecture != selected.selected_plan().target.architecture {
        return Err(TerminalPostAllocationMachineError::PhysicalRegisterModelMismatch);
    }
    if constraints.physical_identity() != physical.identity()
        || constraints.identity() != effects.plan().register_constraints
    {
        return Err(TerminalPostAllocationMachineError::RegisterConstraintCatalogMismatch);
    }
    Ok(())
}

fn validate_structural_allocation<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedTerminalPreAllocationMachineEffects,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<(), TerminalPostAllocationMachineError> {
    let source = &selected.selected_plan().structural_unit_functions;
    if effects.plan().structural_unit_functions.len() != source.len()
        || ranges.plan().structural_unit_functions.len() != source.len()
        || legality.plan().structural_unit_functions.len() != source.len()
        || homes.plan().structural_unit_functions.len() != source.len()
    {
        let machine = source
            .first()
            .map(|function| function.machine)
            .unwrap_or(selected.selected_plan().entry);
        return Err(TerminalPostAllocationMachineError::StructuralAllocationMismatch { machine });
    }
    for function in source {
        unique_structural_effect(effects, function.machine)?;
        let range_matches = ranges
            .plan()
            .structural_unit_functions
            .iter()
            .filter(|candidate| candidate.machine == function.machine)
            .collect::<Vec<_>>();
        let legality_matches = legality
            .plan()
            .structural_unit_functions
            .iter()
            .filter(|candidate| candidate.machine == function.machine)
            .collect::<Vec<_>>();
        let home = unique_structural_home(homes, function.machine)?;
        let ([range], [legality]) = (range_matches.as_slice(), legality_matches.as_slice()) else {
            return Err(
                TerminalPostAllocationMachineError::StructuralAllocationMismatch {
                    machine: function.machine,
                },
            );
        };
        if range.block_domains.len() != 1
            || range.block_domains[0].block != function.entry_block
            || !range.virtual_registers.is_empty()
            || !range.tied_pairs.is_empty()
            || !range.early_clobbers.is_empty()
            || !range.interference.is_empty()
            || !legality.virtual_registers.is_empty()
            || !home.assignments.is_empty()
        {
            return Err(
                TerminalPostAllocationMachineError::StructuralAllocationMismatch {
                    machine: function.machine,
                },
            );
        }
    }
    Ok(())
}

fn selected_instructions(
    block: &TerminalSelectedBlock,
) -> impl Iterator<Item = &TerminalSelectedInstruction> {
    let terminator = match &block.terminator {
        omega_terminal_selected_instructions::TerminalSelectedTerminator::ConditionalBranch {
            instruction,
            ..
        }
        | omega_terminal_selected_instructions::TerminalSelectedTerminator::Return {
            instruction,
            ..
        } => instruction,
    };
    block.instructions.iter().chain(std::iter::once(terminator))
}

const fn reads(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

const fn writes(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}

#[cfg(test)]
mod tests {
    use omega_register_model::{RegisterClassId, RegisterViewId, validate_physical_register_model};
    use omega_terminal_selected_instructions::{
        TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey,
        TerminalMachineLatencyKnowledge, TerminalMachineSizeKnowledge, TerminalVirtualRegisterId,
    };

    use super::*;

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(
            omega_terminal_isa_x86_64::x86_64_physical_register_model(),
        )
        .unwrap()
    }

    fn view(physical: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
        physical.model().view_named(name).unwrap().id
    }

    fn operand(number: u16, view: RegisterViewId) -> TerminalPhysicalOperandFootprint {
        TerminalPhysicalOperandFootprint {
            operand: number,
            virtual_register: TerminalVirtualRegisterId(u32::from(number)),
            class: RegisterClassId(0),
            view,
            access: if number == 2 {
                RegisterOperandAccess::Def
            } else {
                RegisterOperandAccess::Use
            },
            storage_units: Vec::new(),
            read_units: Vec::new(),
            write_units: Vec::new(),
            write_semantics: None,
        }
    }

    fn alternative(
        variant: u32,
        applicability: TerminalMachineAlternativeApplicability,
    ) -> TerminalMachineAlternative {
        TerminalMachineAlternative {
            key: TerminalMachineAlternativeKey {
                family: TerminalMachineAlternativeFamily::ExactSubtractI64,
                variant,
            },
            applicability,
            size: TerminalMachineSizeKnowledge::ExactBytes(3),
            latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
            encoded:
                omega_terminal_selected_instructions::TerminalMachineEncodedEffects::fallthrough_v1(
                    vec![0, 1],
                    vec![2],
                ),
        }
    }

    fn subtract_alternatives() -> Vec<TerminalMachineAlternative> {
        vec![
            alternative(
                0,
                TerminalMachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
            ),
            alternative(
                1,
                TerminalMachineAlternativeApplicability::
                    ResultAliasesOperandAndDistinctFromOperand {
                        result: 2,
                        aliased_operand: 0,
                        distinct_operand: 1,
                    },
            ),
            alternative(
                2,
                TerminalMachineAlternativeApplicability::
                    ResultAliasesOperandAndDistinctFromOperand {
                        result: 2,
                        aliased_operand: 1,
                        distinct_operand: 0,
                    },
            ),
            alternative(
                3,
                TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
            ),
        ]
    }

    #[test]
    fn x86_subtract_home_partition_selects_each_unique_variant() {
        let physical = physical();
        let rax = view(&physical, "rax");
        let rbx = view(&physical, "rbx");
        let rcx = view(&physical, "rcx");
        let alternatives = subtract_alternatives();
        for (views, expected) in [
            ([rax, rax, rax], 0),
            ([rax, rbx, rax], 1),
            ([rbx, rax, rax], 2),
            ([rax, rbx, rcx], 3),
        ] {
            let operands = views
                .into_iter()
                .enumerate()
                .map(|(number, view)| operand(u16::try_from(number).unwrap(), view))
                .collect::<Vec<_>>();
            assert_eq!(
                choose_alternative(7, &operands, &alternatives, &physical)
                    .unwrap()
                    .key
                    .variant,
                expected
            );
        }
    }

    #[test]
    fn x86_lea_add_accepts_r12_as_a_rex_extended_sib_index() {
        let physical = physical();
        let rax = view(&physical, "rax");
        let r12 = view(&physical, "r12");
        let add = TerminalMachineAlternative {
            key: TerminalMachineAlternativeKey {
                family: TerminalMachineAlternativeFamily::ExactAddI64,
                variant: 0,
            },
            applicability: TerminalMachineAlternativeApplicability::Always,
            size: TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(5),
            },
            latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
            encoded:
                omega_terminal_selected_instructions::TerminalMachineEncodedEffects::fallthrough_v1(
                    vec![0, 1],
                    vec![2],
                ),
        };
        let operands = [operand(0, r12), operand(1, r12), operand(2, rax)];
        assert_eq!(
            choose_alternative(8, &operands, std::slice::from_ref(&add), &physical)
                .unwrap()
                .key,
            add.key
        );
    }
}
