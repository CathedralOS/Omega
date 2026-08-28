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
};

use crate::{
    TerminalInstructionMachineEffects, TerminalMachineAlternativeChoiceRule,
    TerminalPhysicalOperandFootprint, TerminalPostAllocationMachineError,
    TerminalPostAllocationMachineInstruction, TerminalPostAllocationMachinePlan,
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
    if terminal_post_allocation_machine_identity(&plan) != plan.identity {
        return Err(TerminalPostAllocationMachineError::IdentityMismatch);
    }
    let receipt = post_allocation_receipt(&plan)?;
    Ok(ValidatedTerminalPostAllocationMachinePlan::new(
        plan, receipt,
    ))
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
    if !selected
        .selected_plan()
        .structural_unit_functions
        .is_empty()
        || !effects.plan().structural_unit_functions.is_empty()
    {
        return Err(TerminalPostAllocationMachineError::UnsupportedStructuralUnitFunctions);
    }
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
