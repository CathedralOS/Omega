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
    TerminalSelectedInstruction,
};

use crate::{
    TerminalBlockMachineEffects, TerminalInstructionMachineEffects,
    TerminalMachineAlternativeChoiceRule, TerminalPhysicalOperandFootprint,
    TerminalPostAllocationMachineBlock, TerminalPostAllocationMachineError,
    TerminalPostAllocationMachineFunction, TerminalPostAllocationMachineIdentity,
    TerminalPostAllocationMachineInstruction, TerminalPostAllocationMachinePlan,
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
    };
    plan.identity = terminal_post_allocation_machine_identity(&plan);
    Ok(plan)
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
            applicable.push(*alternative);
        }
    }
    match applicable.as_slice() {
        [alternative] => Ok(*alternative),
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
    fn x86_lea_add_rejects_two_r12_inputs_but_accepts_a_swappable_index() {
        let physical = physical();
        let rax = view(&physical, "rax");
        let r12 = view(&physical, "r12");
        let add = TerminalMachineAlternative {
            key: TerminalMachineAlternativeKey {
                family: TerminalMachineAlternativeFamily::ExactAddI64,
                variant: 0,
            },
            applicability:
                TerminalMachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
                    left: 0,
                    right: 1,
                    excluded_view: r12,
                },
            size: TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(5),
            },
            latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
        };
        let operands = [operand(0, r12), operand(1, r12), operand(2, rax)];
        assert_eq!(
            choose_alternative(8, &operands, &[add], &physical),
            Err(TerminalPostAllocationMachineError::NoApplicableAlternative { instruction: 8 })
        );
        let operands = [operand(0, r12), operand(1, rax), operand(2, rax)];
        assert_eq!(
            choose_alternative(8, &operands, &[add], &physical)
                .unwrap()
                .key,
            add.key
        );
    }
}
