/*
Application-sponsored completion of canonical Terminal scalar applications
(`scalar_completion::complete`).

Both post-Terminal source checking and native preparation call `complete` to
evaluate `ScalarApplicationId` demands against the retained
`TerminalTargetProviderProposal` companions without reopening source trees. Any
attempted evaluation usage is recorded in `accounting` even when a condition
refuses. Each scope also receives `TargetRealizedIntrinsics`, so a constant that
reaches a canonical float operation executes the realization its target
selected.
*/

use backend_plan::TerminalTargetProviderProposal;
use runtime_abi::target_carrier_layout;
use std::sync::Arc;
use terminal_installation::admit_target_machine_selections;

pub fn complete(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: &str,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut build_time_evaluation::SourceCompletionAccounting,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    let deployed = proposals
        .iter()
        .any(|proposal| proposal.application_roots.is_some());
    let scopes = proposals
        .iter()
        .filter(|proposal| proposal.application_required)
        .map(|proposal| proposal.target.clone())
        .collect::<Vec<_>>();
    build_time_evaluation::complete_scalar_applications(
        artifact,
        primary,
        deployed.then_some(scopes.as_slice()),
        demand,
        None,
        accounting,
        |scope| {
            let proposal = proposals
                .iter()
                .find(|proposal| proposal.target == scope)
                .ok_or_else(|| format!("scalar checking has no provider inputs for {scope}"))?;
            if proposal.terminal_psi != artifact.view().identity() {
                return Err("scalar application roots belong to another artifact".into());
            }
            let selection = proposal
                .selection
                .as_ref()
                .map_err(|reasons| reasons.join("; "))?;
            let providers = selection.admit(artifact.view(), scope)?;
            let realizations = TargetRealizedIntrinsics::retain(artifact, selection)?;
            let profile = target::TargetProfile::from_canonical_target_name(scope)
                .map_err(|diagnostic| diagnostic.message)?;
            let address_model =
                target::TargetSemantics::close_address_model(profile).map(|model| {
                    terminal_installation::ScalarAddressModel {
                        target: scope.to_owned(),
                        carrier: model.carrier(),
                        semantics_version: model.semantics_version(),
                        commitment: model.commitment(),
                    }
                });
            Ok(build_time_evaluation::ScalarScopeEvaluation {
                application_roots: proposal.application_roots.clone(),
                layout_target: profile
                    .layout_target_facts()
                    .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                targets: admit_target_machine_selections(artifact.view(), scope),
                providers,
                import_realizations: Some(Box::new(realizations)),
                address_model: Ok(address_model),
                carrier_layout: profile.native_realization().map(target_carrier_layout),
            })
        },
    )
}

/// The numeric catalog operations the scope's target selection realizes.
///
/// A canonical float operation reaches Terminal Psi as a bodyless boundary.
/// Provider selection has already chosen each one from the target's declared
/// realization table, as a compiler-intrinsic row of the retained plans, and
/// left unchosen any the target declares unsupported. Constant evaluation
/// executes exactly those rows, through the same catalog admission and
/// `FloatSemantics` adapters that run them for a Build program, so a constant
/// never sees a value the target's own realization would not produce.
struct TargetRealizedIntrinsics {
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    selected: effects::SelectedProviderPlanFacts,
    declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
}

impl TargetRealizedIntrinsics {
    fn retain(
        artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
        selection: &backend_plan::TerminalTargetProviderSelection,
    ) -> Result<Self, String> {
        let selected = effects::SelectedProviderPlanFacts::from_selected_plans(
            selection
                .plans
                .iter()
                .map(|selected| selected.plan.clone())
                .collect(),
        )?
        .with_assumed_bindings(selection.assumed_bindings.clone());
        Ok(Self {
            artifact: Arc::clone(artifact),
            selected,
            declarations: selection.plans.clone(),
        })
    }
}

impl build_time_evaluation::TargetImportRealizations for TargetRealizedIntrinsics {
    fn admit(
        &self,
        imports: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Box<dyn build_time_evaluation::AdmittedImportRealizations>, String> {
        use provider_admission::provider_admission as admission;
        let module = self.artifact.view().module();
        // Catalog admission matches selected rows by callable identity. An
        // import with no identity cannot match one and stays unrealized.
        let demanded = imports
            .iter()
            .filter_map(|slot| {
                let boundary =
                    terminal_psi::find_boundary_machine(&module.boundary_machines, *slot)?;
                module
                    .identities
                    .text(boundary.identity)
                    .filter(|identity| !identity.is_empty())
                    .map(str::to_owned)
            })
            .collect::<std::collections::BTreeSet<_>>();
        // Constant evaluation holds no host or device review, so no row
        // carries a reviewed execution: numeric rows close their sealed
        // meaning through the catalog and any other demanded intrinsic is
        // refused by the admission below.
        let unreviewed = self
            .selected
            .plans()
            .iter()
            .map(|plan| vec![None; plan.rows.len()])
            .collect::<Vec<_>>();
        let intrinsics = admission::admit_terminal_intrinsics(
            self.artifact.view().into(),
            &self.selected,
            &self.declarations,
            self.declarations
                .iter()
                .zip(&unreviewed)
                .map(
                    |(declaration, executions)| admission::CompilerIntrinsicPlanProvenance {
                        plan: &declaration.plan,
                        semantic_schema: &declaration.plan.schema,
                        row_executions: executions,
                        row_numeric_realizations: &declaration.row_numeric_realizations,
                    },
                ),
            &demanded,
        )
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        Ok(Box::new(AdmittedIntrinsics(intrinsics)))
    }
}

struct AdmittedIntrinsics(provider_admission::provider_admission::AdmittedTerminalIntrinsics);

impl build_time_evaluation::AdmittedImportRealizations for AdmittedIntrinsics {
    fn realizes(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> bool {
        self.0
            .operations()
            .iter()
            .any(|(slot, _)| *slot == boundary)
    }

    fn bind(
        &self,
        instance: &mut terminal_interpreter::TerminalInstance<'_, '_>,
    ) -> Result<(), String> {
        terminal_runtime::bind_admitted_intrinsics(instance, &self.0)
            .map_err(|reason| format!("constant evaluation intrinsic binding: {reason:?}"))
    }
}
