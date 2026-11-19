//! A target-specific canonical artifact commits the exact providers used by scalar evaluation.
use backend_plan::{SelectedCheckedInvocationRow, selected_checked_invocation_rows};
use provider_plans::ProviderPlan;
use std::sync::Arc;
use terminal_codec::{CanonicalTerminalArtifact, EvidenceSeals};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldedTargetProgram {
    original: terminal_psi::TerminalPsiIdentity,
    artifact: Arc<CanonicalTerminalArtifact>,
    evidence: Option<Arc<EvidenceSeals>>,
}
impl FoldedTargetProgram {
    pub(crate) fn close(
        original: &CanonicalTerminalArtifact,
        accepted_evidence: Option<&EvidenceSeals>,
        folded: terminal_psi::FoldedConstantClosure,
    ) -> Result<Self, String> {
        if original.semantic_module().folded_constants.target.is_some()
            || !original.optimization().selections().is_empty()
        {
            return Err("folded constant custody requires an unbound portable product".into());
        }
        let prior = original.view();
        folded.validate(prior.module()).map_err(str::to_owned)?;
        let mut module = prior.module().clone();
        module.folded_constants = folded;
        let debug = original
            .debug_bytes()
            .map(|bytes| terminal_codec::decode_debug_map(prior.module(), bytes))
            .transpose()
            .map_err(|error| error.to_string())?;
        let closed =
            terminal_codec::ClosedTerminalModule::new(module).map_err(|error| error.to_string())?;
        let accepted_evidence = if original.publication_metadata().is_some() {
            let published = original.published_evidence().map(Arc::as_ref);
            if accepted_evidence.is_some() && accepted_evidence != published {
                return Err(
                    "folding received different evidence from the published program".into(),
                );
            }
            published
        } else {
            accepted_evidence
        };
        let evidence = accepted_evidence
            .map(|evidence| {
                if evidence.module != prior.identity() {
                    return Err("accepted evidence belongs to another portable program".to_owned());
                }
                Ok(Arc::new(closed.evidence_seals(evidence.seals.clone())))
            })
            .transpose()?;
        let identity = closed.identity();
        let record = terminal_codec::PsiOptimizationExecutionRecord::new(
            optimization::PsiOptimizationSelections::default(),
            identity,
            identity,
        )
        .map_err(|error| format!("{error:?}"))?;
        let artifact = CanonicalTerminalArtifact::from_closed_parts(closed, record, debug.as_ref())
            .map_err(|error| error.to_string())?;
        let artifact = match original.publication_metadata() {
            Some(metadata) => artifact
                .with_publication(Arc::clone(metadata), evidence.clone())
                .map_err(|error| error.to_string())?,
            None => artifact,
        };
        Ok(Self {
            original: prior.identity(),
            artifact: Arc::new(artifact),
            evidence,
        })
    }
    pub fn original(&self) -> terminal_psi::TerminalPsiIdentity {
        self.original
    }
    pub fn artifact(&self) -> &Arc<CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn evidence(&self) -> Option<&Arc<EvidenceSeals>> {
        self.evidence.as_ref()
    }
}

/// Scalar invocation reach over the published logical bodies of the authorized program.
/// Binding joins these exact bodies to the selected target plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FoldedEntryReach {
    program: terminal_psi::TerminalPsiIdentity,
    target: String,
    has_applications: bool,
    selected: Vec<(
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::MachineId,
    )>,
    reached: Vec<semantic_vocabulary::BoundaryMachineId>,
}

impl FoldedEntryReach {
    pub(crate) fn capture(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        target: &str,
        bindings: &[terminal_semantics::callable_values::CallableBoundaryImplementation],
        has_applications: bool,
    ) -> Result<Self, String> {
        let module = artifact.module();
        let root = module.program_entry_contract(target)?.entry;
        let mut selected = Vec::new();
        let mut reached = Default::default();
        if !module.scalar_applications.declarations().is_empty() {
            selected = bindings
                .iter()
                .map(|binding| (binding.boundary, binding.machine))
                .collect();
            selected.sort_unstable();
            selected.dedup();
            reached = scalar_boundaries(module, target, &[root], &selected)?;
        }
        Ok(Self {
            program: artifact.identity(),
            target: target.to_owned(),
            selected,
            reached,
            has_applications,
        })
    }

    pub(crate) fn bind(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        target: &str,
        plans: &[ProviderPlan],
    ) -> Result<terminal_psi::FoldedConstantClosure, String> {
        if self.program != artifact.identity() || self.target != target {
            return Err("folded scalar entry belongs to another program or deployment".into());
        }
        let module = artifact.module();
        if module.scalar_applications.declarations().is_empty() {
            return Ok(Default::default());
        }
        let rows = selected_checked_invocation_rows(plans, module)?;
        if selected_bodies(&rows) != self.selected {
            return Err(
                "folded scalar entry differs from the published logical provider bodies".into(),
            );
        }
        let closure = folded_dependencies_for_rows(module, target, rows, &self.reached)?;
        if self.has_applications && !closure.dependencies.is_empty() {
            return Err("folded constant custody requires an unbound portable product".into());
        }
        Ok(closure)
    }
}

fn scalar_boundaries(
    module: &terminal_psi::TerminalModule,
    target: &str,
    roots: &[semantic_vocabulary::MachineId],
    selected: &[(
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::MachineId,
    )],
) -> Result<Vec<semantic_vocabulary::BoundaryMachineId>, String> {
    let graph = terminal_semantics::synchronous_calls::SynchronousCallGraph::new(module, None)?
        .with_boundary_calls(selected.to_vec())?;
    Ok(graph
        .from_callables(graph.scalar_evaluation_roots(roots, Some(target))?)?
        .boundaries
        .into_iter()
        .collect())
}

fn selected_bodies(
    rows: &[SelectedCheckedInvocationRow<'_, '_>],
) -> Vec<(
    semantic_vocabulary::BoundaryMachineId,
    semantic_vocabulary::MachineId,
)> {
    let mut selected: Vec<_> = rows
        .iter()
        .flat_map(|row| &row.candidates)
        .map(|candidate| {
            (
                candidate.conformance.boundary,
                candidate.conformance.candidate,
            )
        })
        .collect();
    selected.sort_unstable();
    selected.dedup();
    selected
}

/// Independently reconstruct a received artifact's scalar closure from the selected plans.
pub(crate) fn folded_constant_dependencies(
    module: &terminal_psi::TerminalModule,
    target: &str,
    plans: &[ProviderPlan],
    roots: &[semantic_vocabulary::MachineId],
) -> Result<terminal_psi::FoldedConstantClosure, String> {
    if roots.is_empty() || module.scalar_applications.declarations().is_empty() {
        return Ok(Default::default());
    }
    let rows = selected_checked_invocation_rows(plans, module)?;
    let reached = scalar_boundaries(module, target, roots, &selected_bodies(&rows))?;
    folded_dependencies_for_rows(module, target, rows, &reached)
}

fn folded_dependencies_for_rows(
    module: &terminal_psi::TerminalModule,
    target: &str,
    rows: Vec<SelectedCheckedInvocationRow<'_, '_>>,
    reached: &[semantic_vocabulary::BoundaryMachineId],
) -> Result<terminal_psi::FoldedConstantClosure, String> {
    let mut dependencies = Vec::new();
    for row in rows {
        if row.plan.target.is_empty() {
            return Err("selected provider plan has no deployment target".into());
        }
        if row.plan.target != target {
            return Err("folded scalar evaluation received a plan for another target".into());
        }
        for candidate in row.candidates {
            let candidate = candidate.conformance;
            if reached.binary_search(&candidate.boundary).is_ok() {
                dependencies.push(terminal_psi::FoldedConstantDependency {
                    target: target.to_owned(),
                    requirement: candidate.source_requirement_identity,
                    plan: *row.plan.identity_digest().as_bytes(),
                    candidate: Some(terminal_psi::FoldedConstantCandidate {
                        boundary: candidate.boundary,
                        machine: candidate.candidate,
                        declaration: candidate.provider_declaration.clone(),
                    }),
                });
            }
        }
    }
    dependencies.sort_unstable();
    dependencies.dedup();
    let target = (!dependencies.is_empty()).then(|| target.to_owned());
    let closure = terminal_psi::FoldedConstantClosure {
        target,
        dependencies,
    };
    closure.validate(module).map_err(str::to_owned)?;
    Ok(closure)
}

#[cfg(test)]
mod tests {
    use super::{FoldedEntryReach, folded_constant_dependencies};
    use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};

    #[test]
    fn a_declaration_only_program_has_no_executable_entry_to_fold() {
        let module = terminal_psi::TerminalModule::declarations();
        let record = build_identity_optimization_execution_record(&module).unwrap();
        let program = CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
        let target = "linux_x86_64";
        assert!(FoldedEntryReach::capture(program.view(), target, &[], false).is_err());
        let roots = [];
        let closure =
            folded_constant_dependencies(program.semantic_module(), target, &[], &roots).unwrap();
        assert_eq!(closure, terminal_psi::FoldedConstantClosure::default());
    }
}
