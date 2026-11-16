use target_operations::TargetOperationPlan;

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
}

impl PreparedCallableInvocations {
    pub fn new(
        unit: &optimization_unit::PsiOptimizationUnit,
        roots: &terminal_semantics::callable_values::CallableInvocationRoots,
    ) -> Result<Self, &'static str> {
        let program =
            optimization_unit_semantics::callable_values::optimized_callable_values(unit)?;
        let invocations =
            terminal_semantics::callable_values::reachable_callable_invocations_with_roots(
                &program,
                [unit.entry],
                roots,
            )?;
        Ok(Self {
            psi: unit.psi,
            optimization_unit: unit.identity,
            work_schedule: unit.work_schedule,
            entry: unit.entry,
            invocations: std::sync::Arc::new(invocations),
        })
    }

    pub(crate) fn for_target(
        &self,
        target: &TargetOperationPlan,
        evidence: crate::PreparationEvidence<'_>,
    ) -> Result<
        std::sync::Arc<terminal_semantics::callable_values::ReachableCallableInvocations>,
        &'static str,
    > {
        if self.psi != target.psi
            || self.optimization_unit != evidence.optimization_unit
            || self.work_schedule != evidence.work_schedule
            || self.entry != target.entry
        {
            return Err(
                "callable invocation facts differ from the actual target unit, schedule or entry",
            );
        }
        Ok(std::sync::Arc::clone(&self.invocations))
    }
}
