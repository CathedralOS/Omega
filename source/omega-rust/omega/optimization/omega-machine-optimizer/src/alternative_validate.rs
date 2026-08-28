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
    TerminalMachineAlternativeApplicability, TerminalSelectedBlock, TerminalSelectedInstruction,
    TerminalSelectedStructuralUnitFunction,
};

use crate::{
    TerminalInstructionMachineEffects, TerminalMachineAlternativeChoiceRule,
    TerminalPhysicalOperandFootprint, TerminalPostAllocationMachineError,
    TerminalPostAllocationMachineInstruction, TerminalPostAllocationMachinePlan,
    TerminalPostAllocationStructuralUnitFunction, TerminalStructuralUnitFunctionMachineEffects,
    ValidatedTerminalPostAllocationMachinePlan, ValidatedTerminalPreAllocationMachineEffects,
    post_allocation_receipt, terminal_post_allocation_machine_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_post_allocation_machine_plan<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedTerminalPreAllocationMachineEffects,
    ranges: &ValidatedTerminalLiveRanges,
    legality: &ValidatedTerminalAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    plan: TerminalPostAllocationMachinePlan,
) -> Result<ValidatedTerminalPostAllocationMachinePlan, TerminalPostAllocationMachineError> {
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
        &plan,
    )?;
    if plan.functions.len() != selected.selected_plan().functions.len()
        || plan.functions.len() != effects.plan().functions.len()
        || plan.functions.len() != homes.plan().functions.len()
    {
        return Err(TerminalPostAllocationMachineError::FunctionMismatch { function: 0 });
    }
    for (function_index, ((selected_function, effect_function), actual_function)) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&effects.plan().functions)
        .zip(&plan.functions)
        .enumerate()
    {
        let home_function = homes
            .plan()
            .functions
            .iter()
            .find(|homes| homes.machine == selected_function.machine)
            .ok_or(TerminalPostAllocationMachineError::FunctionMismatch {
                function: function_index,
            })?;
        if effect_function.machine != selected_function.machine
            || actual_function.machine != selected_function.machine
            || effect_function.blocks.len() != selected_function.blocks.len()
            || actual_function.blocks.len() != selected_function.blocks.len()
        {
            return Err(TerminalPostAllocationMachineError::FunctionMismatch {
                function: function_index,
            });
        }
        for (block_index, ((selected_block, effect_block), actual_block)) in selected_function
            .blocks
            .iter()
            .zip(&effect_function.blocks)
            .zip(&actual_function.blocks)
            .enumerate()
        {
            if effect_block.block != selected_block.id || actual_block.block != selected_block.id {
                return Err(TerminalPostAllocationMachineError::BlockMismatch {
                    function: function_index,
                    block: block_index,
                });
            }
            let selected_instructions = selected_instructions(selected_block).collect::<Vec<_>>();
            if effect_block.instructions.len() != selected_instructions.len()
                || actual_block.instructions.len() != selected_instructions.len()
            {
                return Err(TerminalPostAllocationMachineError::BlockMismatch {
                    function: function_index,
                    block: block_index,
                });
            }
            for ((selected_instruction, effect_instruction), actual_instruction) in
                selected_instructions
                    .into_iter()
                    .zip(&effect_block.instructions)
                    .zip(&actual_block.instructions)
            {
                let expected = reconstruct_instruction(
                    function_index,
                    selected_instruction,
                    effect_instruction,
                    home_function,
                    physical,
                )?;
                if &expected != actual_instruction {
                    return Err(TerminalPostAllocationMachineError::InstructionMismatch {
                        function: function_index,
                        instruction: selected_instruction.id.0,
                    });
                }
            }
        }
    }
    if plan.structural_unit_functions.len()
        != selected.selected_plan().structural_unit_functions.len()
        || plan.structural_unit_functions.len() != effects.plan().structural_unit_functions.len()
    {
        return Err(
            TerminalPostAllocationMachineError::StructuralAllocationMismatch {
                machine: selected.selected_plan().entry,
            },
        );
    }
    let actual_machines = plan
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    if actual_machines.len() != plan.structural_unit_functions.len() {
        return Err(
            TerminalPostAllocationMachineError::StructuralAllocationMismatch {
                machine: selected.selected_plan().entry,
            },
        );
    }
    for (structural_index, selected_function) in selected
        .selected_plan()
        .structural_unit_functions
        .iter()
        .enumerate()
    {
        let effect_function = unique_structural_effect(effects, selected_function.machine)?;
        let home_function = unique_structural_home(homes, selected_function.machine)?;
        let expected = reconstruct_structural_function(
            selected.selected_plan().functions.len() + structural_index,
            selected_function,
            effect_function,
            home_function,
            physical,
        )?;
        let Some(actual) = plan
            .structural_unit_functions
            .get(structural_index)
            .filter(|function| function.machine == selected_function.machine)
        else {
            return Err(
                TerminalPostAllocationMachineError::StructuralFunctionMismatch {
                    machine: selected_function.machine,
                },
            );
        };
        if *actual != expected {
            return Err(
                TerminalPostAllocationMachineError::StructuralFunctionMismatch {
                    machine: selected_function.machine,
                },
            );
        }
    }
    if terminal_post_allocation_machine_identity(&plan) != plan.identity {
        return Err(TerminalPostAllocationMachineError::IdentityMismatch);
    }
    let receipt = post_allocation_receipt(&plan)?;
    Ok(ValidatedTerminalPostAllocationMachinePlan::new(
        plan, receipt,
    ))
}

fn reconstruct_structural_function(
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
    Ok(TerminalPostAllocationStructuralUnitFunction {
        machine: selected.machine,
        block: selected.entry_block,
        call: effects.call.clone(),
        return_instruction: reconstruct_instruction(
            function_index,
            &selected.terminator.instruction,
            &effects.return_instruction,
            homes,
            physical,
        )?,
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

fn reconstruct_instruction(
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
    let mut operands = Vec::with_capacity(selected.operands.len());
    for operand in &selected.operands {
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
        if home.class != operand.class
            || view.class != operand.class
            || operand.fixed_view.is_some_and(|fixed| fixed != home.view)
        {
            return Err(TerminalPostAllocationMachineError::HomeClassMismatch {
                function: function_index,
                register: operand.virtual_register.0,
            });
        }
        let reads = reads(operand.access);
        let writes = writes(operand.access);
        operands.push(TerminalPhysicalOperandFootprint {
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
        });
    }
    let mut chosen = None;
    for alternative in &effects.alternatives {
        if is_applicable(
            selected.id.0,
            &operands,
            alternative.applicability,
            physical,
        )? && chosen.replace(alternative.clone()).is_some()
        {
            return Err(
                TerminalPostAllocationMachineError::AmbiguousApplicableAlternatives {
                    instruction: selected.id.0,
                },
            );
        }
    }
    let alternative = chosen.ok_or(
        TerminalPostAllocationMachineError::NoApplicableAlternative {
            instruction: selected.id.0,
        },
    )?;
    let mut unit_uses = BTreeSet::from_iter(effects.unit_uses.iter().copied());
    let mut unit_defs = BTreeSet::from_iter(effects.unit_defs.iter().copied());
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

fn is_applicable(
    instruction: u32,
    operands: &[TerminalPhysicalOperandFootprint],
    applicability: TerminalMachineAlternativeApplicability,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, TerminalPostAllocationMachineError> {
    let view = |number| {
        operands
            .iter()
            .find(|operand| operand.operand == number)
            .map(|operand| operand.view)
            .ok_or(
                TerminalPostAllocationMachineError::MissingApplicabilityOperand {
                    instruction,
                    operand: number,
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
    plan: &TerminalPostAllocationMachinePlan,
) -> Result<(), TerminalPostAllocationMachineError> {
    validate_structural_allocation(selected, effects, ranges, legality, homes)?;
    if effects.receipt().selected() != selected.selected_identity()
        || ranges.receipt().selected() != selected.selected_identity()
        || plan.selected != selected.selected_identity()
    {
        return Err(TerminalPostAllocationMachineError::SelectedRootMismatch);
    }
    if plan.effects != effects.receipt().identity() {
        return Err(TerminalPostAllocationMachineError::EffectRootMismatch);
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
    if legality.receipt().ranges() != ranges.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
    {
        return Err(TerminalPostAllocationMachineError::RangeRootMismatch);
    }
    if plan.legality != legality.receipt().identity() {
        return Err(TerminalPostAllocationMachineError::LegalityRootMismatch);
    }
    if homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
        || plan.homes != homes.receipt().identity()
    {
        return Err(TerminalPostAllocationMachineError::HomeRootMismatch);
    }
    if effects.plan().register_environment != register_environment
        || legality.receipt().register_environment() != register_environment
        || homes.receipt().register_environment() != register_environment
        || plan.register_environment != register_environment
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
        || plan.post_allocation_manifest != record.identity
    {
        return Err(TerminalPostAllocationMachineError::PostAllocationManifestMismatch);
    }
    if effects.plan().target != selected.selected_plan().target
        || plan.target != selected.selected_plan().target
    {
        return Err(TerminalPostAllocationMachineError::TargetMismatch);
    }
    if physical.model().architecture != selected.selected_plan().target.architecture
        || plan.physical_register_model != physical.identity()
    {
        return Err(TerminalPostAllocationMachineError::PhysicalRegisterModelMismatch);
    }
    if constraints.physical_identity() != physical.identity()
        || constraints.identity() != effects.plan().register_constraints
        || plan.register_constraints != constraints.identity()
    {
        return Err(TerminalPostAllocationMachineError::RegisterConstraintCatalogMismatch);
    }
    if plan.register_constraints != effects.plan().register_constraints
        || plan.machine_effect_catalog != effects.plan().machine_effect_catalog
        || plan.choice_rule
            != TerminalMachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1
    {
        return Err(TerminalPostAllocationMachineError::EffectRootMismatch);
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
