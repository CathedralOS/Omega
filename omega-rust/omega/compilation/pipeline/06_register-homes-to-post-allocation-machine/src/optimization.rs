/*
We execute the post-allocation optimization phase over the freshly constructed
`PostAllocationMachinePlan`, rewriting instructions onto ISA-declared
`OptimizationSelected` forms when their extra defined or clobbered physical
register units are proved dead (`:DeclaredFormLiveness`).

We project the `PostAllocationMachine` slice of the build's
`OptimizationSelections` and execute each selected rule in catalog order. A
rule contributes only its target architecture requirement and its predicate over
`SelectedInstructionKind` (for example, `X86SelectXorZeroI64MaterializationV1`
matches exact `i64` zero materializations on x86-64). The shared rewrite
(`rewrite_declared_forms`) locates the alternative tagged with that rule in the
instruction's pre-allocation effect row, computes the set of extra physical
register units (`extra_units`) that the optimizer form defines or clobbers
beyond the baseline alternative, and queries `UnitLivenessTable` to verify that
none of those units is read before the whole set is overwritten along any
control-flow path.

Because rewriting an instruction onto an optimizer form only adds clobbers, it
can only shorten unit liveness; a single `UnitLivenessTable` computed per
function and unit set remains conservatively sound across all candidates in
that function. After all selected rules finish and work usage is checked
against the phase budget, we recompute the plan's canonical digest so rewritten
plans bind their new contents while identity runs preserve the source digest.
*/

use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
};
use physical_instructions::{
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, post_allocation_machine_identity,
};
use register_model::RegisterUnitId;
use selected_instructions::{
    FunctionMachineEffects, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeKey, PreAllocationMachineEffectPlan, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, terminator_successors,
};
use semantic_vocabulary::IntegerValue;
use target::Architecture;

pub(crate) fn execute_post_allocation_machine_optimizations(
    plan: &mut PostAllocationMachinePlan,
    selected: &SelectedInstructionPlan,
    effects: &PreAllocationMachineEffectPlan,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<PostAllocationMachineOptimizationRun, PostAllocationMachineOptimizationError> {
    let phase = OptimizationExecutionPhase::PostAllocationMachine;
    let projection = selections.project_phase(phase);
    let source = plan.identity;
    let successors = successor_table(selected)?;
    let mut usage = OptimizationWorkUsage {
        iterations: 1,
        ..OptimizationWorkUsage::default()
    };
    let mut executions = Vec::new();
    for optimization in projection.selections().as_slice().iter().copied() {
        let rule = executor(optimization)
            .ok_or(PostAllocationMachineOptimizationError::NoExecutor { optimization })?;
        if plan.target.architecture != rule.required_architecture {
            return Err(PostAllocationMachineOptimizationError::TargetMismatch {
                optimization,
                required: rule.required_architecture,
                actual: plan.target.architecture,
            });
        }
        usage.rule_evaluations = usage.rule_evaluations.saturating_add(1);
        let mut context = RuleContext {
            plan,
            effects,
            successors: &successors,
            usage: &mut usage,
        };
        executions.push((rule.run)(optimization, &mut context)?);
    }
    let extent = plan
        .instructions()
        .try_fold(0_u64, |count, _| count.checked_add(1))
        .ok_or(PostAllocationMachineOptimizationError::CountOverflow)?;
    if !usage.within(budget.for_extent(extent)) {
        return Err(PostAllocationMachineOptimizationError::BudgetExhausted { phase });
    }
    plan.identity = post_allocation_machine_identity(plan);
    Ok(PostAllocationMachineOptimizationRun {
        complete_selection: projection.complete_selection(),
        phase_selections: projection.selections().identity(),
        source,
        result: plan.identity,
        executions,
        usage,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineOptimizationRun {
    complete_selection: OptimizationSelectionIdentity, // Identity of the complete build selection.
    phase_selections: OptimizationSelectionIdentity, // Identity of the executed post-allocation projection.
    source: PostAllocationMachineIdentity,           // Plan identity before phase rules ran.
    result: PostAllocationMachineIdentity,           // Plan identity after phase rules ran.
    executions: Vec<PostAllocationMachineRuleExecution>,
    usage: OptimizationWorkUsage,
}

impl PostAllocationMachineOptimizationRun {
    pub const fn phase(&self) -> OptimizationExecutionPhase {
        OptimizationExecutionPhase::PostAllocationMachine
    }

    copied_accessors! {
        complete_selection: OptimizationSelectionIdentity,
        phase_selections: OptimizationSelectionIdentity,
        source: PostAllocationMachineIdentity,
        result: PostAllocationMachineIdentity,
        usage: OptimizationWorkUsage,
    }

    pub fn executions(&self) -> &[PostAllocationMachineRuleExecution] {
        &self.executions
    }

    pub fn action_count(&self) -> usize {
        self.executions
            .iter()
            .map(|execution| execution.actions.len())
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineRuleExecution {
    optimization: Optimization,
    candidates: u64,
    actions: Vec<PostAllocationMachineAction>,
}

impl PostAllocationMachineRuleExecution {
    copied_accessors! {
        optimization: Optimization,
        candidates: u64,
    }

    pub fn actions(&self) -> &[PostAllocationMachineAction] {
        &self.actions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAllocationMachineAction {
    pub function: usize,
    pub block: usize,
    pub instruction: SelectedInstructionId,
    pub baseline: MachineAlternativeKey,
    pub selected: MachineAlternativeKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationMachineOptimizationError {
    NoExecutor {
        optimization: Optimization,
    },
    TargetMismatch {
        optimization: Optimization,
        required: Architecture,
        actual: Architecture,
    },
    RosterMismatch {
        function: usize,
        block: usize,
    },
    MissingDeclaredForm {
        optimization: Optimization,
        function: usize,
        instruction: u32,
    },
    BudgetExhausted {
        phase: OptimizationExecutionPhase,
    },
    CountOverflow,
}

stage_error_display!(
    PostAllocationMachineOptimizationError,
    "post-allocation machine optimization failed"
);

struct RuleContext<'a> {
    plan: &'a mut PostAllocationMachinePlan,
    effects: &'a PreAllocationMachineEffectPlan,
    successors: &'a [Vec<Vec<usize>>], // Successor block indexes per function and block index.
    usage: &'a mut OptimizationWorkUsage,
}

struct RuleExecutor {
    required_architecture: Architecture,
    run: fn(
        Optimization,
        &mut RuleContext<'_>,
    )
        -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError>,
}

fn executor(optimization: Optimization) -> Option<RuleExecutor> {
    match optimization {
        Optimization::X86SelectXorZeroI64MaterializationV1 => Some(RuleExecutor {
            required_architecture: Architecture::X86_64,
            run: run_x86_select_xor_zero_i64_materialization,
        }),
        _ => None,
    }
}

fn run_x86_select_xor_zero_i64_materialization(
    optimization: Optimization,
    context: &mut RuleContext<'_>,
) -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError> {
    select_declared_form(optimization, context, materializes_exact_zero)
}

fn materializes_exact_zero(kind: SelectedInstructionKind) -> bool {
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => match value {
            IntegerValue::Signed(value) => value == 0,
            IntegerValue::Unsigned(value) => value == 0,
        },
        _ => false,
    }
}

fn successor_table(
    selected: &SelectedInstructionPlan,
) -> Result<Vec<Vec<Vec<usize>>>, PostAllocationMachineOptimizationError> {
    selected
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            function
                .blocks
                .iter()
                .enumerate()
                .map(|(block_index, block)| {
                    terminator_successors(&block.terminator)
                        .into_iter()
                        .map(|successor| {
                            function.block_index(successor.block).ok_or(
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

fn select_declared_form(
    optimization: Optimization,
    context: &mut RuleContext<'_>,
    admits: fn(SelectedInstructionKind) -> bool,
) -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError> {
    rewrite_declared_forms(
        optimization,
        &mut context.plan.functions,
        &context.effects.functions,
        context.successors,
        context.usage,
        admits,
    )
}

fn rewrite_declared_forms(
    optimization: Optimization,
    functions: &mut [PostAllocationMachineFunction],
    effect_functions: &[FunctionMachineEffects],
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

fn declared_form(
    optimization: Optimization,
    alternatives: &[MachineAlternative],
) -> Option<&MachineAlternative> {
    alternatives.iter().find(|alternative| {
        alternative.applicability
            == MachineAlternativeApplicability::OptimizationSelected { optimization }
    })
}

fn extra_units(
    instruction: &PostAllocationMachineInstruction,
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

fn apply_form(instruction: &mut PostAllocationMachineInstruction, form: &MachineAlternative) {
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

struct UnitLivenessTable {
    live_in: Vec<bool>,
    suffix_effects: Vec<Vec<BlockEffect>>, // Per block and instruction index `i`, the effect of `instructions[i + 1..]`.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockEffect {
    Reads,       // Some instruction reads a unit of the set before the set is rewritten.
    Kills,       // Every unit of the set is rewritten before any is read.
    Transparent, // The block neither reads nor fully rewrites the set.
}

impl UnitLivenessTable {
    fn compute(
        function: &PostAllocationMachineFunction,
        successors: &[Vec<usize>],
        units: &[RegisterUnitId],
    ) -> Self {
        let mut effects = Vec::with_capacity(function.blocks.len());
        let mut suffix_effects = Vec::with_capacity(function.blocks.len());
        for block in &function.blocks {
            let (block_eff, suffixes) =
                compute_block_and_suffix_effects(&block.instructions, units);
            effects.push(block_eff);
            suffix_effects.push(suffixes);
        }
        let mut live_in = vec![false; function.blocks.len()];
        for _ in 0..=function.blocks.len() {
            let mut changed = false;
            for (block, effect) in effects.iter().enumerate().rev() {
                if live_in[block] {
                    continue;
                }
                let value = match effect {
                    BlockEffect::Reads => true,
                    BlockEffect::Kills => false,
                    BlockEffect::Transparent => successors.get(block).is_some_and(|successors| {
                        successors.iter().any(|successor| live_in[*successor])
                    }),
                };
                if value {
                    live_in[block] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Self {
            live_in,
            suffix_effects,
        }
    }

    fn live_after(
        &self,
        function: &PostAllocationMachineFunction,
        successors: &[Vec<usize>],
        block: usize,
        index: usize,
        units: &[RegisterUnitId],
    ) -> (bool, u64) {
        let instructions = &function.blocks[block].instructions[index + 1..];
        let scanned = instructions.len() as u64;
        let effect = self
            .suffix_effects
            .get(block)
            .and_then(|s| s.get(index))
            .copied()
            .unwrap_or_else(|| block_effect(instructions, units));
        match effect {
            BlockEffect::Reads => (true, scanned),
            BlockEffect::Kills => (false, scanned),
            BlockEffect::Transparent => (
                successors.get(block).is_some_and(|successors| {
                    successors
                        .iter()
                        .any(|successor| self.live_in.get(*successor).copied().unwrap_or(true))
                }),
                scanned,
            ),
        }
    }
}

fn compute_block_and_suffix_effects(
    instructions: &[PostAllocationMachineInstruction],
    units: &[RegisterUnitId],
) -> (BlockEffect, Vec<BlockEffect>) {
    let mut suffixes = vec![BlockEffect::Transparent; instructions.len()];
    let mut running = BlockEffect::Transparent;
    for index in (0..instructions.len()).rev() {
        suffixes[index] = running;
        let instruction = &instructions[index];
        if reads_any(instruction, units) {
            running = BlockEffect::Reads;
        } else if rewrites_every(instruction, units) {
            running = BlockEffect::Kills;
        }
    }
    (running, suffixes)
}

fn block_effect(
    instructions: &[PostAllocationMachineInstruction],
    units: &[RegisterUnitId],
) -> BlockEffect {
    for instruction in instructions {
        if reads_any(instruction, units) {
            return BlockEffect::Reads;
        }
        if rewrites_every(instruction, units) {
            return BlockEffect::Kills;
        }
    }
    BlockEffect::Transparent
}

fn reads_any(instruction: &PostAllocationMachineInstruction, units: &[RegisterUnitId]) -> bool {
    units.iter().any(|unit| {
        instruction.unit_uses.contains(unit) || instruction.implicit_unit_uses.contains(unit)
    })
}

fn rewrites_every(
    instruction: &PostAllocationMachineInstruction,
    units: &[RegisterUnitId],
) -> bool {
    !units.is_empty()
        && units.iter().all(|unit| {
            instruction.unit_defs.contains(unit)
                || instruction.unit_clobbers.contains(unit)
                || instruction.implicit_unit_defs.contains(unit)
                || instruction.implicit_unit_clobbers.contains(unit)
        })
}

#[cfg(test)]
mod tests {
    use optimization_core::{Optimization, OptimizationWorkUsage};
    use physical_instructions::{
        PhysicalOperandFootprint, PostAllocationMachineBlock, PostAllocationMachineFunction,
        PostAllocationMachineInstruction,
    };
    use register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
        RegisterUnitId, RegisterViewId,
    };
    use selected_instructions::{
        BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, MachineAlternative,
        MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
        MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEncodedEffects,
        MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance, VirtualRegisterId,
    };
    use semantic_vocabulary::{IntegerValue, MachineId};

    use super::{
        PostAllocationMachineOptimizationError, PostAllocationMachineRuleExecution,
        UnitLivenessTable, executor, rewrite_declared_forms,
    };

    const RULE: Optimization = Optimization::X86SelectXorZeroI64MaterializationV1;
    const FLAGS: RegisterUnitId = RegisterUnitId(99);
    const DESTINATION_VIEW: RegisterViewId = RegisterViewId(7);
    const DESTINATION_UNIT: RegisterUnitId = RegisterUnitId(7);

    fn baseline_materialization() -> MachineAlternative {
        MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::MaterializeI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(10),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
        }
    }

    fn xor_zero_form() -> MachineAlternative {
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
        encoded.implicit_unit_clobbers = vec![FLAGS];
        MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::MaterializeI64,
                variant: 1,
            },
            applicability: MachineAlternativeApplicability::OptimizationSelected {
                optimization: RULE,
            },
            size: MachineSizeKnowledge::ExactBytes(3),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }
    }

    fn instruction(
        id: u32,
        uses: &[RegisterUnitId],
        defs: &[RegisterUnitId],
    ) -> PostAllocationMachineInstruction {
        PostAllocationMachineInstruction {
            instruction: SelectedInstructionId(id),
            alternative: baseline_materialization(),
            operands: vec![PhysicalOperandFootprint {
                operand: 0,
                virtual_register: VirtualRegisterId(id),
                class: RegisterClassId(0),
                view: DESTINATION_VIEW,
                access: RegisterOperandAccess::Def,
                storage_units: vec![DESTINATION_UNIT],
                read_units: vec![],
                write_units: vec![DESTINATION_UNIT],
                write_semantics: None,
            }],
            address: None,
            implicit_unit_uses: uses.to_vec(),
            implicit_unit_defs: defs.to_vec(),
            implicit_unit_clobbers: vec![],
            unit_uses: uses.to_vec(),
            unit_defs: defs.to_vec(),
            unit_clobbers: vec![],
        }
    }

    fn effects(
        id: u32,
        kind: SelectedInstructionKind,
        declared: bool,
    ) -> InstructionMachineEffects {
        let mut alternatives = vec![baseline_materialization()];
        if declared {
            alternatives.push(xor_zero_form());
        }
        InstructionMachineEffects {
            instruction: SelectedInstructionId(id),
            kind,
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 0,
            },
            unit_uses: vec![],
            unit_defs: vec![],
            unit_clobbers: vec![],
            memory: MachineMemoryEffect::NoneV1,
            trap: MachineTrapBehavior::NeverV1,
            barrier: MachineBarrier::None,
            call: MachineCallEffect::NoneV1,
            cleanup: MachineCleanupEffect::NoneV1,
            provenance: SelectedInstructionProvenance {
                operations: vec![],
                values: vec![],
                edges: vec![],
                obligations: vec![],
                work: vec![],
            },
            alternatives,
        }
    }

    fn zero() -> SelectedInstructionKind {
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(0),
        }
    }

    fn seven() -> SelectedInstructionKind {
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(7),
        }
    }

    fn roster(
        blocks: Vec<(
            Vec<PostAllocationMachineInstruction>,
            Vec<InstructionMachineEffects>,
        )>,
    ) -> (PostAllocationMachineFunction, FunctionMachineEffects) {
        let machine = MachineId::new(1).unwrap();
        let mut function = PostAllocationMachineFunction {
            machine,
            local_storage_slots: vec![],
            outgoing_arguments: vec![],
            blocks: vec![],
        };
        let mut effect_function = FunctionMachineEffects {
            machine,
            blocks: vec![],
        };
        for (index, (instructions, rows)) in blocks.into_iter().enumerate() {
            function.blocks.push(PostAllocationMachineBlock {
                block: SelectedBlockId(index as u32),
                instructions,
            });
            effect_function.blocks.push(BlockMachineEffects {
                block: SelectedBlockId(index as u32),
                instructions: rows,
            });
        }
        (function, effect_function)
    }

    fn admits_zero(kind: SelectedInstructionKind) -> bool {
        matches!(
            kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Signed(0) | IntegerValue::Unsigned(0)
            }
        )
    }

    fn run(
        function: PostAllocationMachineFunction,
        effect_function: FunctionMachineEffects,
        successors: Vec<Vec<usize>>,
    ) -> (
        Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError>,
        PostAllocationMachineFunction,
        OptimizationWorkUsage,
    ) {
        let mut functions = vec![function];
        let mut usage = OptimizationWorkUsage::default();
        let result = rewrite_declared_forms(
            RULE,
            &mut functions,
            &[effect_function],
            &[successors],
            &mut usage,
            admits_zero,
        );
        (result, functions.remove(0), usage)
    }

    #[test]
    fn zero_materialization_with_dead_flags_takes_the_declared_form() {
        let (function, rows) = roster(vec![(
            vec![instruction(1, &[], &[]), instruction(2, &[], &[])],
            vec![effects(1, zero(), true), effects(2, seven(), true)],
        )]);
        let (result, function, usage) = run(function, rows, vec![vec![]]);
        let execution = result.unwrap();
        assert_eq!(execution.candidates(), 1);
        assert_eq!(execution.actions().len(), 1);
        assert_eq!(execution.actions()[0].instruction, SelectedInstructionId(1));
        assert_eq!(execution.actions()[0].selected.variant, 1);
        let rewritten = &function.blocks[0].instructions[0];
        assert_eq!(rewritten.alternative, xor_zero_form());
        assert_eq!(rewritten.unit_clobbers, vec![FLAGS]);
        assert_eq!(rewritten.implicit_unit_clobbers, vec![FLAGS]);
        assert_eq!(
            function.blocks[0].instructions[1].alternative.key.variant,
            0
        );
        assert_eq!(usage.candidates, 1);
        assert_eq!(usage.commits, 1);
    }

    #[test]
    fn a_later_flag_reader_in_the_block_keeps_the_baseline() {
        let (function, rows) = roster(vec![(
            vec![instruction(1, &[], &[]), instruction(2, &[FLAGS], &[])],
            vec![effects(1, zero(), true), effects(2, seven(), true)],
        )]);
        let (result, function, usage) = run(function, rows, vec![vec![]]);
        let execution = result.unwrap();
        assert_eq!(execution.candidates(), 1);
        assert!(execution.actions().is_empty());
        assert_eq!(
            function.blocks[0].instructions[0].alternative.key.variant,
            0
        );
        assert_eq!(usage.commits, 0);
        assert_eq!(usage.validation_steps, 1);
    }

    #[test]
    fn a_flag_definition_before_the_reader_frees_the_candidate() {
        let (function, rows) = roster(vec![(
            vec![
                instruction(1, &[], &[]),
                instruction(2, &[], &[FLAGS]),
                instruction(3, &[FLAGS], &[]),
            ],
            vec![
                effects(1, zero(), true),
                effects(2, seven(), true),
                effects(3, seven(), true),
            ],
        )]);
        let (result, _, _) = run(function, rows, vec![vec![]]);
        assert_eq!(result.unwrap().actions().len(), 1);
    }

    #[test]
    fn flags_live_into_a_successor_block_keep_the_baseline() {
        let (function, rows) = roster(vec![
            (
                vec![instruction(1, &[], &[]), instruction(2, &[], &[])],
                vec![effects(1, zero(), true), effects(2, seven(), false)],
            ),
            (
                vec![instruction(3, &[FLAGS], &[])],
                vec![effects(3, seven(), false)],
            ),
        ]);
        let (result, _, _) = run(function, rows, vec![vec![1], vec![]]);
        assert!(result.unwrap().actions().is_empty());
    }

    #[test]
    fn flags_dead_through_a_loop_back_edge_take_the_declared_form() {
        let (function, rows) = roster(vec![
            (
                vec![instruction(1, &[], &[]), instruction(2, &[], &[])],
                vec![effects(1, zero(), true), effects(2, seven(), false)],
            ),
            (
                vec![
                    instruction(3, &[], &[FLAGS]),
                    instruction(4, &[FLAGS], &[]),
                    instruction(5, &[FLAGS], &[]),
                ],
                vec![
                    effects(3, seven(), false),
                    effects(4, seven(), false),
                    effects(5, seven(), false),
                ],
            ),
            (
                vec![instruction(6, &[], &[])],
                vec![effects(6, seven(), false)],
            ),
        ]);
        let (result, _, _) = run(function, rows, vec![vec![1], vec![0, 2], vec![]]);
        assert_eq!(result.unwrap().actions().len(), 1);
    }

    #[test]
    fn a_candidate_without_a_declared_form_rejects_by_rule_and_site() {
        let (function, rows) = roster(vec![(
            vec![instruction(1, &[], &[])],
            vec![effects(1, zero(), false)],
        )]);
        let (result, _, _) = run(function, rows, vec![vec![]]);
        assert_eq!(
            result,
            Err(
                PostAllocationMachineOptimizationError::MissingDeclaredForm {
                    optimization: RULE,
                    function: 0,
                    instruction: 1,
                }
            )
        );
    }

    #[test]
    fn an_already_selected_form_is_not_rewritten_again() {
        let (mut function, rows) = roster(vec![(
            vec![instruction(1, &[], &[])],
            vec![effects(1, zero(), true)],
        )]);
        function.blocks[0].instructions[0].alternative = xor_zero_form();
        let (result, _, usage) = run(function, rows, vec![vec![]]);
        let execution = result.unwrap();
        assert_eq!(execution.candidates(), 1);
        assert!(execution.actions().is_empty());
        assert_eq!(usage.commits, 0);
    }

    #[test]
    fn unit_liveness_reads_block_locally_then_through_successors() {
        let (function, _) = roster(vec![
            (
                vec![instruction(1, &[], &[]), instruction(2, &[], &[FLAGS])],
                vec![],
            ),
            (vec![instruction(3, &[FLAGS], &[])], vec![]),
        ]);
        let successors = vec![vec![1], vec![]];
        let table = UnitLivenessTable::compute(&function, &successors, &[FLAGS]);
        assert!(!table.live_after(&function, &successors, 0, 0, &[FLAGS]).0);
        assert!(table.live_after(&function, &successors, 0, 1, &[FLAGS]).0);
        assert!(!table.live_after(&function, &successors, 1, 0, &[FLAGS]).0);
    }

    #[test]
    fn the_rule_table_executes_only_rules_it_names() {
        assert!(executor(RULE).is_some());
        assert!(executor(Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1).is_none());
        assert!(executor(Optimization::CopyPropagation).is_none());
    }
}
