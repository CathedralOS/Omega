/*
We run the whole-program `SelectedLowering` rewrites a build selects, before
liveness is staged and the pair-rule literal folds start:
`SelectedAcyclicCallInliningV1` (`call_inlining.rs`) and then
`SelectedPackedVectorPromotionV1` (`vector_promotion.rs`). Neither is
legalization. Every target lowers a program without them, so a build that
does not name them in `builder.optimizations` never runs them
([exact selection](../../../../../../../../wiki/spec/build/optimizations.md#exact-selection)).
Instruction selection used to run both on every build.

The catalog row (`SELECTED_LOWERING_RULE_CATALOG`) names the rewrite, so this
coordinator reads the selected rows rather than its own name table. Promotion
needs the target's 128-bit packed register forms. Only x86-64 supplies them,
so a build selecting it for an AArch64 target is refused by name before any
rewrite runs.

These rewrites do not need the pair-rule fixed point, which still refuses
many program shapes, so selecting only them never starts it. The run records
its own `SelectedLoweringOptimizationCompletionIdentity` in the program's
`SelectedOptimizationRecord`; a pair-rule run that follows replaces it with its
own and keeps this run as its source.

Each step keeps the program it read and the program it produced. The default
compile drops them with the rest of the selected-optimization run. An explicit
validator (`SelectedOptimizationReplay::validate`) calls
`validate_structural_rewrites`, which checks the selection, completion
identity and applicability again, re-executes every step from its kept input,
and checks each rewrite's own postconditions without trusting the producer:
- every step keeps the program's contracts, target, entry and function roster,
  and leaves each function with resolvable block, instruction and register
  references;
- inlining keeps each function's machine signature and only calls machines the
  function already reached through calls in its input;
- promotion is intra-block: each function keeps its blocks, their successor
  blocks and its calls.
*/

use crate::selected_rewrites::rewrites::SELECTED_LOWERING_RULE_CATALOG;
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelections,
    SelectedLoweringOptimizationCompletionIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterConstraintKey, RegisterInstructionConstraint};
use selected_instructions::{
    SelectedCallTarget, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionProvenance,
    SelectedProgram, StagedOptimizedCustodyCore, VirtualRegisterId, VirtualRegisterOrigin,
    terminator_instruction, terminator_successors,
};
use semantic_vocabulary::MachineId;
use std::sync::Arc;

/// One whole-program `SelectedLowering` rewrite. Its catalog row is the only
/// place that binds it to an optimization name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralRewrite {
    AcyclicCallInlining,
    PackedVectorPromotion,
}

impl StructuralRewrite {
    fn optimization(self) -> Optimization {
        SELECTED_LOWERING_RULE_CATALOG
            .iter()
            .find(|entry| entry.payload().structural() == Some(self))
            .map(|entry| entry.optimization())
            .expect("every structural rewrite has a catalog row")
    }

    /// Whether this target supplies every instruction form the rewrite emits.
    fn supports(self, environment: &ValidatedTargetRegisterEnvironment) -> bool {
        match self {
            Self::AcyclicCallInlining => true,
            Self::PackedVectorPromotion => {
                crate::selected_rewrites::vector_promotion::supports_target(environment)
            }
        }
    }

    fn apply(
        self,
        functions: &mut [SelectedFunction],
        contracts: &selected_instructions::SelectedProgramContracts,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Result<(), StructuralRewriteError> {
        match self {
            Self::AcyclicCallInlining => {
                crate::selected_rewrites::call_inlining::inline_and_simplify(
                    functions,
                    environment,
                    |machine| {
                        contracts
                            .function_abi(machine)
                            .is_some_and(|abi| abi.assembly_return.is_some())
                    },
                )
            }
            Self::PackedVectorPromotion => {
                crate::selected_rewrites::vector_promotion::promote_and_simplify(
                    functions,
                    environment,
                )
            }
        }
    }

    fn check_postconditions(
        self,
        input: &SelectedInstructionPlan,
        output: &SelectedInstructionPlan,
    ) -> Result<(), StructuralRewriteValidationError> {
        let optimization = self.optimization();
        if !Arc::ptr_eq(&input.contracts, &output.contracts) && input.contracts != output.contracts
            || input.psi != output.psi
            || input.work_schedule != output.work_schedule
            || input.target != output.target
            || input.entry != output.entry
            || input.functions.len() != output.functions.len()
        {
            return Err(StructuralRewriteValidationError::ProgramChanged { optimization });
        }
        let graph = match self {
            Self::AcyclicCallInlining => Some(InputCallGraph::new(input)),
            Self::PackedVectorPromotion => None,
        };
        for (position, (before, after)) in input
            .functions
            .iter()
            .zip(output.functions.iter())
            .enumerate()
        {
            let machine = before.machine;
            if after.machine != machine
                || after.attachment != before.attachment
                || after.structural != before.structural
            {
                return Err(StructuralRewriteValidationError::SignatureChanged {
                    optimization,
                    machine,
                });
            }
            references_resolve(after).map_err(|reason| {
                StructuralRewriteValidationError::MalformedOutput {
                    optimization,
                    machine,
                    reason,
                }
            })?;
            let kept = match &graph {
                Some(graph) => calls_stay_within_reach(graph, position, after),
                None => blocks_and_calls_unchanged(before, after),
            };
            if !kept {
                return Err(StructuralRewriteValidationError::PostconditionFailed {
                    optimization,
                    machine,
                });
            }
        }
        Ok(())
    }
}

/// Whether a `SelectedLowering` optimization is one of these rewrites rather
/// than a pair rule.
pub(crate) fn is_structural(optimization: Optimization) -> bool {
    SELECTED_LOWERING_RULE_CATALOG
        .iter()
        .any(|entry| entry.optimization() == optimization && entry.payload().structural().is_some())
}

/// The structural rewrites a selection names, in catalog (execution) order.
pub(crate) fn selected_structural_rewrites(
    selections: &OptimizationSelections,
) -> Vec<StructuralRewrite> {
    let selected = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    SELECTED_LOWERING_RULE_CATALOG
        .iter()
        .filter(|entry| selected.contains(entry.optimization()))
        .filter_map(|entry| entry.payload().structural())
        .collect()
}

/// One applied rewrite: the program it read and the program it produced.
#[derive(Debug, Clone)]
pub(crate) struct StructuralRewriteStep {
    rewrite: StructuralRewrite,
    input: Arc<SelectedInstructionPlan>,
    output: Arc<SelectedInstructionPlan>,
}

/// The selected structural rewrites of one compile, kept for an explicit
/// validator with the custody, register environment and selection they ran
/// under. Later rewrites start from `output`.
#[derive(Debug, Clone)]
pub(crate) struct StructuralRewriteRun {
    steps: Vec<StructuralRewriteStep>,
    custody: StagedOptimizedCustodyCore,
    environment: ValidatedTargetRegisterEnvironment,
    selections: OptimizationSelections,
    completion: SelectedLoweringOptimizationCompletionIdentity,
}

impl StructuralRewriteRun {
    /// What the program's optimization record names as this run.
    pub(crate) const fn completion(&self) -> SelectedLoweringOptimizationCompletionIdentity {
        self.completion
    }

    pub(crate) const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub(crate) fn output(&self) -> &Arc<SelectedInstructionPlan> {
        &self
            .steps
            .last()
            .expect("a structural rewrite run has at least one step")
            .output
    }

    /// The rewritten program under the input program's identities.
    pub(crate) fn output_program(&self, input: &SelectedProgram) -> SelectedProgram {
        SelectedProgram {
            plan: Arc::clone(self.output()),
            optimization_unit: input.optimization_unit,
            work_schedule: input.work_schedule,
        }
    }
}

/// Run every selected structural rewrite over `program`, or nothing when the
/// selection names none.
pub(crate) fn run_structural_rewrites(
    program: &SelectedProgram,
    custody: StagedOptimizedCustodyCore,
    environment: &ValidatedTargetRegisterEnvironment,
    selections: &OptimizationSelections,
) -> Result<Option<StructuralRewriteRun>, StructuralRewriteError> {
    if custody.register_environment != environment.identity() {
        return Err(StructuralRewriteError::custody_because(
            "the register environment is not the one selection used",
        ));
    }
    let rewrites = selected_structural_rewrites(selections);
    if rewrites.is_empty() {
        return Ok(None);
    }
    require_applicable(&rewrites, environment)?;
    let mut steps = Vec::with_capacity(rewrites.len());
    let mut current = Arc::clone(&program.plan);
    for rewrite in rewrites {
        let output = Arc::new(execute(rewrite, &current, environment)?);
        steps.push(StructuralRewriteStep {
            rewrite,
            input: std::mem::replace(&mut current, Arc::clone(&output)),
            output,
        });
    }
    let completion = completion_identity(custody, selections, &steps);
    Ok(Some(StructuralRewriteRun {
        steps,
        custody,
        environment: environment.clone(),
        selections: selections.clone(),
        completion,
    }))
}

/// Names the run by the custody and selection it ran under and the rewrites
/// it applied. No stage hashes a selected plan; the kept steps hold the
/// programs themselves.
fn completion_identity(
    custody: StagedOptimizedCustodyCore,
    selections: &OptimizationSelections,
    steps: &[StructuralRewriteStep],
) -> SelectedLoweringOptimizationCompletionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.selected-lowering-structural-rewrites.v1\0");
    for identity in [
        custody.optimization.bytes(),
        custody.projection.bytes(),
        custody.manifest.bytes(),
        custody.optimization_unit.bytes(),
        custody.register_environment.bytes(),
        selections.identity().bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    canonical.extend_from_slice(&custody.entry.get().to_le_bytes());
    canonical.extend_from_slice(&custody.work_schedule.marker().to_le_bytes());
    for step in steps {
        canonical.push(step.rewrite.optimization() as u8);
    }
    SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(&canonical)
}

/// Check a kept run against the selection and target it claims, re-execute
/// each step from its kept input, and check each rewrite's postconditions.
pub(crate) fn validate_structural_rewrites(
    run: &StructuralRewriteRun,
) -> Result<(), StructuralRewriteValidationError> {
    let environment = &run.environment;
    let rewrites = selected_structural_rewrites(&run.selections);
    if run
        .steps
        .iter()
        .map(|step| step.rewrite)
        .ne(rewrites.iter().copied())
    {
        return Err(StructuralRewriteValidationError::SelectionMismatch);
    }
    if run.custody.register_environment != environment.identity()
        || completion_identity(run.custody, &run.selections, &run.steps) != run.completion
    {
        return Err(StructuralRewriteValidationError::OtherProgram);
    }
    require_applicable(&rewrites, environment)
        .map_err(StructuralRewriteValidationError::Execution)?;
    for (step, next) in run.steps.iter().zip(run.steps.iter().skip(1)) {
        if !Arc::ptr_eq(&step.output, &next.input) && *step.output != *next.input {
            return Err(StructuralRewriteValidationError::OtherProgram);
        }
    }
    for step in &run.steps {
        let replayed = execute(step.rewrite, &step.input, environment)
            .map_err(StructuralRewriteValidationError::Execution)?;
        if replayed != *step.output {
            return Err(StructuralRewriteValidationError::Nondeterministic {
                optimization: step.rewrite.optimization(),
            });
        }
        step.rewrite
            .check_postconditions(&step.input, &step.output)?;
    }
    Ok(())
}

fn require_applicable(
    rewrites: &[StructuralRewrite],
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), StructuralRewriteError> {
    match rewrites
        .iter()
        .find(|rewrite| !rewrite.supports(environment))
    {
        Some(rewrite) => Err(StructuralRewriteError::UnsupportedTarget {
            optimization: rewrite.optimization(),
        }),
        None => Ok(()),
    }
}

fn execute(
    rewrite: StructuralRewrite,
    input: &SelectedInstructionPlan,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<SelectedInstructionPlan, StructuralRewriteError> {
    if input.target != environment.target() {
        return Err(StructuralRewriteError::custody_because(
            "the register environment names another target",
        ));
    }
    let mut functions: Vec<SelectedFunction> = input.functions.iter().cloned().collect();
    rewrite.apply(&mut functions, &input.contracts, environment)?;
    functions.sort_by_key(|function| function.machine);
    Ok(SelectedInstructionPlan {
        contracts: Arc::clone(&input.contracts),
        psi: input.psi,
        work_schedule: input.work_schedule,
        target: input.target,
        entry: input.entry,
        functions: functions.into(),
    })
}

/// Every block, instruction and register a function names exists in it.
fn references_resolve(function: &SelectedFunction) -> Result<(), &'static str> {
    if !function.ids_are_positions() {
        return Err("block or register ids are not their positions");
    }
    let blocks = function.blocks.len();
    if function.entry_block.0 as usize >= blocks {
        return Err("entry block is missing");
    }
    let registers = function.virtual_registers.len();
    let mut defined = Vec::new();
    for instruction in function.instructions_and_terminators() {
        let index = instruction.id.0 as usize;
        if defined.len() <= index {
            defined.resize(index + 1, false);
        }
        if std::mem::replace(&mut defined[index], true) {
            return Err("an instruction id repeats");
        }
        if instruction
            .operands
            .iter()
            .any(|operand| operand.virtual_register.0 as usize >= registers)
        {
            return Err("an operand names a missing register");
        }
    }
    let exists = |id: SelectedInstructionId| defined.get(id.0 as usize).copied() == Some(true);
    for block in function.blocks.iter() {
        if terminator_successors(&block.terminator)
            .iter()
            .any(|edge| edge.block.0 as usize >= blocks)
        {
            return Err("a successor names a missing block");
        }
    }
    for register in &function.virtual_registers {
        let resolves = match register.origin {
            VirtualRegisterOrigin::InstructionScratch { instruction, .. }
            | VirtualRegisterOrigin::StructuralObservation { instruction, .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { instruction, .. }
            | VirtualRegisterOrigin::AbiTransport { instruction, .. }
            | VirtualRegisterOrigin::InstructionResult { instruction, .. } => exists(instruction),
            VirtualRegisterOrigin::SpillAddress {
                instruction,
                register: VirtualRegisterId(spilled),
            } => exists(instruction) && (spilled as usize) < registers,
            VirtualRegisterOrigin::BlockParameter { block, .. } => (block.0 as usize) < blocks,
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. } => true,
        };
        if !resolves {
            return Err("a register origin names a missing instruction or block");
        }
    }
    if !function.calls.iter().all(|call| exists(call.instruction))
        || !function
            .normalized_foreign_calls
            .iter()
            .all(|call| exists(call.instruction))
        || !function
            .memory_accesses
            .iter()
            .all(|access| exists(access.instruction))
    {
        return Err("a call or memory access names a missing instruction");
    }
    Ok(())
}

fn call_target(instruction: &SelectedInstruction) -> Option<SelectedCallTarget> {
    match instruction.kind {
        SelectedInstructionKind::CallUnit { target }
        | SelectedInstructionKind::CallScalar { target }
        | SelectedInstructionKind::CallAggregate { target } => Some(target),
        _ => None,
    }
}

fn call_targets(function: &SelectedFunction) -> impl Iterator<Item = SelectedCallTarget> + '_ {
    function
        .instructions_and_terminators()
        .filter_map(call_target)
}

/// The calls each input function makes, indexed by function position, with a
/// sorted machine lookup.
struct InputCallGraph {
    positions: Vec<(MachineId, usize)>,
    targets: Vec<Vec<SelectedCallTarget>>,
}

impl InputCallGraph {
    fn new(plan: &SelectedInstructionPlan) -> Self {
        let mut positions: Vec<(MachineId, usize)> = plan
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.machine, index))
            .collect();
        positions.sort_unstable();
        Self {
            positions,
            targets: plan
                .functions
                .iter()
                .map(|function| call_targets(function).collect())
                .collect(),
        }
    }

    fn position(&self, machine: MachineId) -> Option<usize> {
        self.positions
            .binary_search_by_key(&machine, |(candidate, _)| *candidate)
            .ok()
            .map(|found| self.positions[found].1)
    }
}

/// Inlining copies callee bodies, so a caller may afterwards call what its
/// callees called, but never a machine its input calls could not reach.
fn calls_stay_within_reach(graph: &InputCallGraph, start: usize, after: &SelectedFunction) -> bool {
    let mut reached = vec![false; graph.targets.len()];
    let mut expanded = vec![false; graph.targets.len()];
    let mut other_targets: Vec<SelectedCallTarget> = Vec::new();
    let mut pending = vec![start];
    expanded[start] = true;
    while let Some(caller) = pending.pop() {
        for target in &graph.targets[caller] {
            let callee = match target {
                SelectedCallTarget::Machine(machine) => graph.position(*machine),
                _ => None,
            };
            match callee {
                Some(callee) => {
                    reached[callee] = true;
                    if !std::mem::replace(&mut expanded[callee], true) {
                        pending.push(callee);
                    }
                }
                None if !other_targets.contains(target) => other_targets.push(*target),
                None => {}
            }
        }
    }
    call_targets(after).all(|target| {
        let callee = match target {
            SelectedCallTarget::Machine(machine) => graph.position(machine),
            _ => None,
        };
        callee.map_or_else(|| other_targets.contains(&target), |callee| reached[callee])
    })
}

/// Promotion rewrites inside blocks only: blocks, their successors and the
/// calls they make stay as they were.
fn blocks_and_calls_unchanged(before: &SelectedFunction, after: &SelectedFunction) -> bool {
    before.entry_block == after.entry_block
        && before.blocks.len() == after.blocks.len()
        && before
            .blocks
            .iter()
            .zip(after.blocks.iter())
            .all(|(old, new)| {
                old.id == new.id
                    && old.origin == new.origin
                    && terminator_successors(&old.terminator)
                        .iter()
                        .map(|edge| edge.block)
                        .eq(terminator_successors(&new.terminator)
                            .iter()
                            .map(|edge| edge.block))
                    && old
                        .instructions
                        .iter()
                        .chain(std::iter::once(terminator_instruction(&old.terminator)))
                        .filter_map(call_target)
                        .eq(new
                            .instructions
                            .iter()
                            .chain(std::iter::once(terminator_instruction(&new.terminator)))
                            .filter_map(call_target))
            })
}

/// Build one instruction from a target constraint row, which must describe
/// exactly the given registers.
pub(crate) fn instruction_from_row(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    registers: &[VirtualRegisterId],
    provenance: SelectedInstructionProvenance,
    row: &RegisterInstructionConstraint,
) -> Result<SelectedInstruction, StructuralRewriteError> {
    if row.operands.len() != registers.len() {
        return Err(StructuralRewriteError::MissingConstraint(row.key));
    }
    let mut instruction =
        selected_instruction_analyses::rewriting::instruction(id, kind, row, registers);
    instruction.provenance = provenance;
    Ok(instruction)
}

/// Why a selected structural rewrite could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralRewriteError {
    /// The target's register environment lacks a row the rewrite emits.
    MissingConstraint(RegisterConstraintKey),
    /// The selected program is not the shape the rewrite was given.
    MalformedInput(Option<&'static str>),
    /// The target supplies none of the forms this selected rewrite emits.
    UnsupportedTarget { optimization: Optimization },
}

impl StructuralRewriteError {
    pub(crate) fn custody() -> Self {
        Self::MalformedInput(None)
    }

    pub(crate) fn custody_because(reason: &'static str) -> Self {
        Self::MalformedInput(Some(reason))
    }
}

failed_display!(StructuralRewriteError, "selected structural rewrite failed");

/// Why a kept structural rewrite run failed its explicit validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralRewriteValidationError {
    /// The run's steps are not the rewrites the build selected.
    SelectionMismatch,
    /// A step did not start from the program before it.
    OtherProgram,
    /// Re-executing a step did not reproduce its output.
    Nondeterministic { optimization: Optimization },
    /// The rewrite could not run again on its own input.
    Execution(StructuralRewriteError),
    /// A step changed the program's contracts, target, entry or function roster.
    ProgramChanged { optimization: Optimization },
    /// A step changed a function's machine, attachment or structural signature.
    SignatureChanged {
        optimization: Optimization,
        machine: MachineId,
    },
    /// A step left a function naming something it does not contain.
    MalformedOutput {
        optimization: Optimization,
        machine: MachineId,
        reason: &'static str,
    },
    /// A step broke the rewrite's own postcondition on one function.
    PostconditionFailed {
        optimization: Optimization,
        machine: MachineId,
    },
}

failed_display!(
    StructuralRewriteValidationError,
    "selected structural rewrite validation failed"
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selected_rewrites::call_inlining::tests::{CallerAndCallee, caller_and_callee};
    use register_environment::baseline_target_register_environment;
    use target::NativeTarget;

    fn selections(optimizations: &[Optimization]) -> OptimizationSelections {
        OptimizationSelections::new(optimizations.iter().copied()).unwrap()
    }

    fn program(target: NativeTarget, functions: Vec<SelectedFunction>) -> SelectedProgram {
        let mut plan = (*selected_instructions::test_support::placeholder_plan(1)).clone();
        plan.target = target;
        plan.entry = functions[0].machine;
        plan.functions = functions.into();
        SelectedProgram {
            work_schedule: plan.work_schedule,
            plan: Arc::new(plan),
            optimization_unit: optimization_core::OptimizationUnitIdentity::from_bytes([2; 32]),
        }
    }

    fn custody(
        environment: &ValidatedTargetRegisterEnvironment,
        program: &SelectedProgram,
    ) -> StagedOptimizedCustodyCore {
        StagedOptimizedCustodyCore {
            psi: program.plan.psi,
            target: program.plan.target,
            entry: program.plan.entry,
            optimization: optimization_core::OptimizationIdentityBundleIdentity::from_bytes(
                [3; 32],
            ),
            projection: optimization_core::OptimizedAbstractPlanProjectionIdentity::from_bytes(
                [4; 32],
            ),
            manifest: optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                [5; 32],
            ),
            optimization_unit: program.optimization_unit,
            work_schedule: program.work_schedule,
            register_environment: environment.identity(),
        }
    }

    /// The fixture caller calls its callee once; the plan is sorted by machine.
    fn caller_and_callee_program() -> (ValidatedTargetRegisterEnvironment, SelectedProgram) {
        let environment =
            baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
        let CallerAndCallee { caller, callee, .. } = caller_and_callee(&environment);
        let program = program(NativeTarget::windows_x64(), vec![caller, callee]);
        (environment, program)
    }

    fn calls(plan: &SelectedInstructionPlan, position: usize) -> usize {
        call_targets(&plan.functions[position]).count()
    }

    #[test]
    fn a_build_that_selects_neither_rewrite_runs_neither() {
        let (environment, program) = caller_and_callee_program();
        for selected in [
            selections(&[]),
            selections(&[Optimization::SelectedIncomingU12ExactAddImmediate]),
            selections(&[Optimization::SelectedSameBlockCopyI64RemovalV1]),
        ] {
            assert!(selected_structural_rewrites(&selected).is_empty());
            assert!(
                run_structural_rewrites(
                    &program,
                    custody(&environment, &program),
                    &environment,
                    &selected
                )
                .unwrap()
                .is_none()
            );
        }
        assert_eq!(calls(&program.plan, 0), 1);
    }

    #[test]
    fn selected_inlining_removes_the_call_and_passes_its_validator() {
        let (environment, program) = caller_and_callee_program();
        let selected = selections(&[Optimization::SelectedAcyclicCallInliningV1]);
        let run = run_structural_rewrites(
            &program,
            custody(&environment, &program),
            &environment,
            &selected,
        )
        .unwrap()
        .unwrap();
        assert_eq!(run.steps.len(), 1);
        assert!(Arc::ptr_eq(&run.steps[0].input, &program.plan));
        assert_eq!(calls(run.output(), 0), 0);
        assert_eq!(calls(run.output(), 1), 0);
        let rewritten = run.output_program(&program);
        assert!(Arc::ptr_eq(&rewritten.plan, run.output()));
        assert_eq!(rewritten.optimization_unit, program.optimization_unit);
        assert_eq!(validate_structural_rewrites(&run), Ok(()));
    }

    #[test]
    fn both_rewrites_run_in_catalog_order_and_validate() {
        let (environment, program) = caller_and_callee_program();
        let selected = selections(&[
            Optimization::SelectedPackedVectorPromotionV1,
            Optimization::SelectedAcyclicCallInliningV1,
        ]);
        let run = run_structural_rewrites(
            &program,
            custody(&environment, &program),
            &environment,
            &selected,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            run.steps
                .iter()
                .map(|step| step.rewrite)
                .collect::<Vec<_>>(),
            [
                StructuralRewrite::AcyclicCallInlining,
                StructuralRewrite::PackedVectorPromotion
            ]
        );
        assert!(Arc::ptr_eq(&run.steps[0].output, &run.steps[1].input));
        assert_eq!(validate_structural_rewrites(&run), Ok(()));
    }

    #[test]
    fn promotion_is_refused_by_name_on_aarch64() {
        let x86 = baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
        assert!(StructuralRewrite::PackedVectorPromotion.supports(&x86));
        let selected = selections(&[Optimization::SelectedPackedVectorPromotionV1]);
        let (_, program) = caller_and_callee_program();
        for target in [
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_arm64(),
        ] {
            let environment = baseline_target_register_environment(target).unwrap();
            assert!(!StructuralRewrite::PackedVectorPromotion.supports(&environment));
            assert!(StructuralRewrite::AcyclicCallInlining.supports(&environment));
            assert_eq!(
                run_structural_rewrites(
                    &program,
                    custody(&environment, &program),
                    &environment,
                    &selected
                )
                .unwrap_err(),
                StructuralRewriteError::UnsupportedTarget {
                    optimization: Optimization::SelectedPackedVectorPromotionV1
                }
            );
        }
    }

    #[test]
    fn a_tampered_or_mismatched_run_is_refused() {
        let (environment, program) = caller_and_callee_program();
        let selected = selections(&[Optimization::SelectedAcyclicCallInliningV1]);
        let run = run_structural_rewrites(
            &program,
            custody(&environment, &program),
            &environment,
            &selected,
        )
        .unwrap()
        .unwrap();

        // The build's selection must name exactly the run's steps.
        let mut unselected = run.clone();
        unselected.selections = selections(&[]);
        assert_eq!(
            validate_structural_rewrites(&unselected),
            Err(StructuralRewriteValidationError::SelectionMismatch)
        );

        // The completion the program's record names must be this run's.
        let mut renamed = run.clone();
        renamed.completion =
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"other");
        assert_eq!(
            validate_structural_rewrites(&renamed),
            Err(StructuralRewriteValidationError::OtherProgram)
        );

        // An output the producer does not reproduce is refused.
        let mut unreproduced = run.clone();
        unreproduced.steps[0].output = Arc::clone(&program.plan);
        assert_eq!(
            validate_structural_rewrites(&unreproduced),
            Err(StructuralRewriteValidationError::Nondeterministic {
                optimization: Optimization::SelectedAcyclicCallInliningV1
            })
        );

        // A step that does not start where the previous one ended is refused.
        let both = selections(&[
            Optimization::SelectedAcyclicCallInliningV1,
            Optimization::SelectedPackedVectorPromotionV1,
        ]);
        let mut detached = run_structural_rewrites(
            &program,
            custody(&environment, &program),
            &environment,
            &both,
        )
        .unwrap()
        .unwrap();
        detached.steps[1].input = Arc::clone(&program.plan);
        assert_eq!(
            validate_structural_rewrites(&detached),
            Err(StructuralRewriteValidationError::OtherProgram)
        );
    }

    #[test]
    fn each_rewrite_checks_its_own_postconditions() {
        let (_, program) = caller_and_callee_program();
        let input = &*program.plan;
        for rewrite in [
            StructuralRewrite::AcyclicCallInlining,
            StructuralRewrite::PackedVectorPromotion,
        ] {
            assert_eq!(rewrite.check_postconditions(input, input), Ok(()));
        }
        let callee_machine = input.functions[1].machine;

        // The callee calls nothing, so it may not start calling its caller.
        let mut new_call = input.clone();
        let mut call = input.functions[0].blocks[0].instructions[0].clone();
        call.kind = SelectedInstructionKind::CallScalar {
            target: SelectedCallTarget::Machine(input.functions[0].machine),
        };
        call.id = SelectedInstructionId(3);
        call.operands = Vec::new();
        new_call.functions[1].blocks[1].instructions.push(call);
        assert_eq!(
            StructuralRewrite::AcyclicCallInlining.check_postconditions(input, &new_call),
            Err(StructuralRewriteValidationError::PostconditionFailed {
                optimization: Optimization::SelectedAcyclicCallInliningV1,
                machine: callee_machine,
            })
        );

        // Promotion stays inside blocks, so it may not drop one.
        let mut merged = input.clone();
        let mut blocks: Vec<_> = input.functions[1].blocks.iter().cloned().collect();
        blocks.truncate(1);
        blocks[0].terminator = input.functions[1].blocks[1].terminator.clone();
        merged.functions[1].blocks = blocks.into();
        assert_eq!(
            StructuralRewrite::PackedVectorPromotion.check_postconditions(input, &merged),
            Err(StructuralRewriteValidationError::PostconditionFailed {
                optimization: Optimization::SelectedPackedVectorPromotionV1,
                machine: callee_machine,
            })
        );

        // Every rewrite leaves references that resolve.
        let mut dangling = input.clone();
        dangling.functions[1].blocks[0].instructions[0].operands[0].virtual_register =
            VirtualRegisterId(99);
        assert_eq!(
            StructuralRewrite::AcyclicCallInlining.check_postconditions(input, &dangling),
            Err(StructuralRewriteValidationError::MalformedOutput {
                optimization: Optimization::SelectedAcyclicCallInliningV1,
                machine: callee_machine,
                reason: "an operand names a missing register",
            })
        );

        // Neither rewrite may change the program's entry.
        let mut other_entry = input.clone();
        other_entry.entry = callee_machine;
        assert_eq!(
            StructuralRewrite::PackedVectorPromotion.check_postconditions(input, &other_entry),
            Err(StructuralRewriteValidationError::ProgramChanged {
                optimization: Optimization::SelectedPackedVectorPromotionV1
            })
        );
    }
}
