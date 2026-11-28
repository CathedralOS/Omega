use target_operations::TargetOperationPlan;
use terminal_semantics::callable_values::CallableValueProgram;
use verified_optimization_input::NativeCallableBindings;

/// The actual optimized-unit query result prepared before retained selection.
/// Its private association prevents a result from another graph, entry or
/// work schedule from being attached to this target's instruction plan.
#[derive(Debug)]
pub struct PreparedCallableInvocations {
    psi: terminal_psi::TerminalPsiIdentity,
    optimization_unit: optimization_core::OptimizationUnitIdentity,
    work_schedule: semantic_vocabulary::WorkScheduleIdentity,
    entry: semantic_vocabulary::MachineId,
    invocations: std::sync::Arc<terminal_semantics::callable_values::ReachableCallableInvocations>,
    entry_invocations: Vec<(
        semantic_vocabulary::MachineId,
        std::sync::Arc<terminal_semantics::callable_values::ReachableCallableInvocations>,
    )>,
}

impl PreparedCallableInvocations {
    pub fn new(
        unit: &optimization_unit::PsiOptimizationUnit,
        bindings: &NativeCallableBindings,
    ) -> Result<Self, &'static str> {
        let program = callable_program_with_providers(unit, bindings)?;
        let roots = bindings.private_entries();
        let invocations =
            terminal_semantics::callable_values::reachable_callable_invocations_with_roots(
                &program,
                [unit.entry],
                roots,
            )?;
        // Each activated callback, interrupt or selected-body root is a real
        // independent entry. Prepare its own fixed point with the same exact
        // registrar associations: a whole-program union would widen its inputs,
        // while dropping root activation would omit nested installations.
        let mut entry_invocations = Vec::new();
        for &entry in &invocations.roots {
            if entry == unit.entry {
                continue;
            }
            let summary =
                terminal_semantics::callable_values::reachable_callable_invocations_with_roots(
                    &program,
                    [entry],
                    roots,
                )?;
            entry_invocations.push((entry, std::sync::Arc::new(summary)));
        }
        Ok(Self {
            psi: unit.psi,
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            entry: unit.entry,
            invocations: std::sync::Arc::new(invocations),
            entry_invocations,
        })
    }

    pub(crate) fn for_target(
        &self,
        target: &TargetOperationPlan,
        evidence: crate::preparation::PreparationEvidence<'_>,
        selection: optimization_core::OptimizationSelectionIdentity,
        budget_per_pass: optimization_core::OptimizationWorkBudget,
    ) -> Result<target_vocabulary::NativeExecutionFacts, target_vocabulary::NativeExecutionFactsError>
    {
        if self.psi != target.psi
            || self.optimization_unit != evidence.optimization_unit
            || self.work_schedule != evidence.work_schedule
            || self.entry != target.entry
        {
            return Err(target_vocabulary::NativeExecutionFactsError::Invalid(
                "callable invocation facts differ from the actual target unit, schedule or entry",
            ));
        }
        target_vocabulary::NativeExecutionFacts::seal(
            target_vocabulary::NativeExecutionRoots {
                psi: self.psi,
                optimization_unit: self.optimization_unit,
                work_schedule: self.work_schedule,
                selection,
                budget_per_pass,
                entry: self.entry,
            },
            std::sync::Arc::clone(&self.invocations),
            self.entry_invocations.clone(),
        )
    }
}

pub(super) fn callable_program_with_providers<'unit>(
    unit: &'unit optimization_unit::PsiOptimizationUnit,
    bindings: &NativeCallableBindings,
) -> Result<CallableValueProgram<'unit>, &'static str> {
    optimization_unit_semantics::callable_values::optimized_callable_values(unit)?
        .with_boundary_implementations(bindings.checked_bodies())
}
