//! The post-allocation rule table and the declared-form rewrite every rule
//! in it shares.
//!
//! A rule here selects, for each instruction its predicate admits, the
//! catalog alternative the ISA tagged with that rule. The shared rewrite owns
//! the guard every optimizer form needs: the units the form defines or
//! clobbers beyond the baseline alternative must be dead after the
//! instruction. A rule contributes only its predicate over the selected kind
//! and its required architecture; it never names bytes or a second form table.

mod x86_select_xor_zero_i64_materialization;

use optimization_core::{Optimization, OptimizationWorkUsage};
use physical_instructions::PostAllocationMachinePlan;
use register_model::RegisterUnitId;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, SelectedInstructionKind,
    SelectedInstructionPlan, terminator_successors,
};
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;
use target::Architecture;

use super::unit_liveness::UnitLivenessTable;
use super::{
    PostAllocationMachineAction, PostAllocationMachineOptimizationError,
    PostAllocationMachineRuleExecution,
};

/// Everything one rule execution reads and charges.
pub(super) struct RuleContext<'a> {
    pub(super) plan: &'a mut PostAllocationMachinePlan,
    pub(super) effects: &'a ValidatedPreAllocationMachineEffects,
    /// Successor block indexes per function and block index.
    pub(super) successors: &'a [Vec<Vec<usize>>],
    pub(super) usage: &'a mut OptimizationWorkUsage,
}

pub(super) struct RuleExecutor {
    pub(super) required_architecture: Architecture,
    pub(super) run:
        fn(
            Optimization,
            &mut RuleContext<'_>,
        )
            -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError>,
}

/// The stage's one rule table. A catalog rule absent here is declared but
/// not executable, and a selection of it rejects by name.
pub(super) fn executor(optimization: Optimization) -> Option<RuleExecutor> {
    match optimization {
        Optimization::X86SelectXorZeroI64MaterializationV1 => Some(RuleExecutor {
            required_architecture: Architecture::X86_64,
            run: x86_select_xor_zero_i64_materialization::run,
        }),
        _ => None,
    }
}

/// Successor block indexes for every block of every function, read from the
/// selected terminators the post-allocation plan mirrors instruction for
/// instruction.
pub(super) fn successor_table(
    selected: &SelectedInstructionPlan,
) -> Result<Vec<Vec<Vec<usize>>>, PostAllocationMachineOptimizationError> {
    selected
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            let index_of = |id| function.blocks.iter().position(|block| block.id == id);
            function
                .blocks
                .iter()
                .enumerate()
                .map(|(block_index, block)| {
                    terminator_successors(&block.terminator)
                        .into_iter()
                        .map(|successor| {
                            index_of(successor.block).ok_or(
                                PostAllocationMachineOptimizationError::RosterMismatch {
                                    function: function_index,
                                    block: block_index,
                                },
                            )
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

/// Rewrite every instruction whose selected kind `admits` onto the catalog
/// alternative tagged with `optimization`, when the units that alternative
/// defines or clobbers beyond the current one are dead afterwards.
pub(super) fn select_declared_form(
    optimization: Optimization,
    context: &mut RuleContext<'_>,
    admits: fn(SelectedInstructionKind) -> bool,
) -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError> {
    rewrite_declared_forms(
        optimization,
        &mut context.plan.functions,
        &context.effects.plan().functions,
        context.successors,
        context.usage,
        admits,
    )
}

/// The declared-form rewrite over plain function rosters: the post-allocation
/// functions being rewritten, their machine-effect rows (which carry each
/// instruction's selected kind and declared alternatives), and the successor
/// table. Tests drive this directly with hand-built rosters.
pub(super) fn rewrite_declared_forms(
    optimization: Optimization,
    functions: &mut [physical_instructions::PostAllocationMachineFunction],
    effect_functions: &[selected_instructions::FunctionMachineEffects],
    successor_table: &[Vec<Vec<usize>>],
    usage: &mut OptimizationWorkUsage,
    admits: fn(SelectedInstructionKind) -> bool,
) -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError> {
    let mut candidates = 0_u64;
    let mut actions = Vec::new();
    for (function_index, function) in functions.iter_mut().enumerate() {
        let effect_function = effect_functions
            .get(function_index)
            .filter(|effects| {
                effects.machine == function.machine && effects.blocks.len() == function.blocks.len()
            })
            .ok_or(PostAllocationMachineOptimizationError::RosterMismatch {
                function: function_index,
                block: 0,
            })?;
        let successors = successor_table
            .get(function_index)
            .filter(|successors| successors.len() == function.blocks.len())
            .ok_or(PostAllocationMachineOptimizationError::RosterMismatch {
                function: function_index,
                block: 0,
            })?;
        // The declared form's extra units are one set per rule and ISA, so
        // one table serves every candidate of the function. The table reads
        // the plan before this rule's rewrites; a rewrite only adds clobbers,
        // which can only shorten liveness, so the table stays conservative.
        let mut table: Option<(Vec<RegisterUnitId>, UnitLivenessTable)> = None;
        for block_index in 0..function.blocks.len() {
            let effect_block = &effect_function.blocks[block_index];
            if effect_block.block != function.blocks[block_index].block
                || effect_block.instructions.len()
                    != function.blocks[block_index].instructions.len()
            {
                return Err(PostAllocationMachineOptimizationError::RosterMismatch {
                    function: function_index,
                    block: block_index,
                });
            }
            for index in 0..function.blocks[block_index].instructions.len() {
                let effects = &effect_block.instructions[index];
                if !admits(effects.kind) {
                    continue;
                }
                candidates = candidates.saturating_add(1);
                usage.candidates = usage.candidates.saturating_add(1);
                let form = declared_form(optimization, &effects.alternatives).ok_or(
                    PostAllocationMachineOptimizationError::MissingDeclaredForm {
                        optimization,
                        function: function_index,
                        instruction: effects.instruction.0,
                    },
                )?;
                let instruction = &function.blocks[block_index].instructions[index];
                if instruction.alternative.key == form.key {
                    continue;
                }
                let extra = extra_units(instruction, form);
                let (live, scanned) = {
                    let (units, table) = table.get_or_insert_with(|| {
                        (
                            extra.clone(),
                            UnitLivenessTable::compute(function, successors, &extra),
                        )
                    });
                    debug_assert_eq!(*units, extra);
                    table.live_after(function, successors, block_index, index, &extra)
                };
                usage.validation_steps = usage.validation_steps.saturating_add(scanned);
                if live {
                    continue;
                }
                let instruction = &mut function.blocks[block_index].instructions[index];
                actions.push(PostAllocationMachineAction {
                    function: function_index,
                    block: block_index,
                    instruction: instruction.instruction,
                    baseline: instruction.alternative.key,
                    selected: form.key,
                });
                apply_form(instruction, form);
                usage.commits = usage.commits.saturating_add(1);
            }
        }
    }
    Ok(PostAllocationMachineRuleExecution {
        optimization,
        candidates,
        actions,
    })
}

/// The catalog alternative the ISA tagged with `optimization`, when declared.
fn declared_form(
    optimization: Optimization,
    alternatives: &[MachineAlternative],
) -> Option<&MachineAlternative> {
    alternatives.iter().find(|alternative| {
        alternative.applicability
            == MachineAlternativeApplicability::OptimizationSelected { optimization }
    })
}

/// Units the declared form defines or clobbers that the current instruction
/// does not already rewrite: the ones that must be dead for the form to be
/// observation-preserving.
fn extra_units(
    instruction: &physical_instructions::PostAllocationMachineInstruction,
    form: &MachineAlternative,
) -> Vec<RegisterUnitId> {
    let mut extra = form
        .encoded
        .implicit_unit_defs
        .iter()
        .chain(&form.encoded.implicit_unit_clobbers)
        .copied()
        .filter(|unit| {
            !(instruction.unit_defs.contains(unit) || instruction.unit_clobbers.contains(unit))
        })
        .collect::<Vec<_>>();
    extra.sort_unstable();
    extra.dedup();
    extra
}

/// Move the instruction onto `form`, carrying the form's implicit units into
/// the instruction's complete unit actions.
fn apply_form(
    instruction: &mut physical_instructions::PostAllocationMachineInstruction,
    form: &MachineAlternative,
) {
    instruction.alternative = form.clone();
    instruction.implicit_unit_uses = sorted(form.encoded.implicit_unit_uses.iter().copied());
    instruction.implicit_unit_defs = sorted(form.encoded.implicit_unit_defs.iter().copied());
    instruction.implicit_unit_clobbers =
        sorted(form.encoded.implicit_unit_clobbers.iter().copied());
    instruction.unit_uses = sorted(
        instruction
            .unit_uses
            .iter()
            .chain(&form.encoded.implicit_unit_uses)
            .copied(),
    );
    instruction.unit_defs = sorted(
        instruction
            .unit_defs
            .iter()
            .chain(&form.encoded.implicit_unit_defs)
            .copied(),
    );
    instruction.unit_clobbers = sorted(
        instruction
            .unit_clobbers
            .iter()
            .chain(&form.encoded.implicit_unit_clobbers)
            .copied(),
    );
}

fn sorted(units: impl Iterator<Item = RegisterUnitId>) -> Vec<RegisterUnitId> {
    let mut units = units.collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}
