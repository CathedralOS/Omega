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
    entry_invocations: Vec<(
        semantic_vocabulary::MachineId,
        std::sync::Arc<terminal_semantics::callable_values::ReachableCallableInvocations>,
    )>,
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
        evidence: crate::PreparationEvidence<'_>,
        pre_physical: optimization_core::PrePhysicalOptimizationManifestIdentity,
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
                pre_physical,
                entry: self.entry,
            },
            std::sync::Arc::clone(&self.invocations),
            self.entry_invocations.clone(),
        )
    }
}
