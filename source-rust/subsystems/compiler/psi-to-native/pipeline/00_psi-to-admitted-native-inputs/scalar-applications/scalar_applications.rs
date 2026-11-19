/*
Application-sponsored completion of canonical Terminal scalar applications
(`complete`).

Post-Terminal Product checking calls `prepare` and retains the completed scope
with its actual execution inputs. `extend_native` adds the executable's roots
and demand to that scope without running identical scalar requests again.
Source-free inputs use `complete_native` and `complete_native_demand` to finish the values a native
compile needs before native lowering starts, so the native pipeline
runs once with them as input and never pauses for evaluation. Any
attempted evaluation usage is recorded in `accounting` even when a condition
refuses. Each deployed scope also receives `TargetRealizedIntrinsics`, so a
constant that reaches a canonical float operation executes the realization its
target selected. An unscoped request supplies only portable facts; demands for
target-dependent operations must still resolve a deployment.
*/

pub use artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt as EvaluationReceipt;
use backend_plan::TerminalTargetProviderProposal;
use psi_target_layouts::terminal_target_context;
use runtime_abi::target_carrier_layout;
use std::sync::Arc;

/// The Product phase's completed scalar work and its unchanged provider inputs.
/// Native completion adds application roots and demanded values to this owner.
/// It is never encoded in saved native inputs.
pub struct ProductScalarCompletion {
    pub completed: artifact_evaluation::scalar_scopes::ScalarApplicationCompletion,
    pub proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
}

/// The scalar values native compilation of `retained`'s program entry reads,
/// evaluated for its deployment, or `None` when it needs none.
pub fn complete_native(
    retained: &verified_optimization_input::RetainedTerminalArtifact,
    accounting: &mut impl Extend<artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt>,
) -> Result<Option<terminal_installation::AdmittedScalarApplications>, String> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("native constant completion needs the retained native proposal")?;
    let demand = proposal
        .entry_scalar_demand(retained.authorization())
        .map_err(diagnostic_text)?;
    complete_native_demand(retained.shared_artifact(), proposal, demand, accounting)
}

/// Evaluate a native scalar demand (such as an independent component's
/// `ComponentNativeInputs::scalar_demand`) for `proposal`'s deployment.
pub fn complete_native_demand(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    proposal: &verified_optimization_input::TerminalNativeRealizationProposal,
    demand: Option<Vec<semantic_vocabulary::ScalarApplicationId>>,
    accounting: &mut impl Extend<artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt>,
) -> Result<Option<terminal_installation::AdmittedScalarApplications>, String> {
    let Some(demand) = demand else {
        return Ok(None);
    };
    complete(
        artifact,
        proposal.target_profile().target_name(),
        &demand,
        proposal.target_provider_proposals(),
        accounting,
    )
    .map(Some)
}

fn diagnostic_text(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn complete<'scope>(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut impl Extend<artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt>,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    prepare(artifact, primary, demand, proposals, accounting)
        .map(|completion| completion.completed.into_admitted())
}

/// Preserve the evaluated scope with its actual target adapters until the
/// executable's application roots and native scalar demand are available.
pub fn prepare<'scope>(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut impl Extend<artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt>,
) -> Result<ProductScalarCompletion, String> {
    let deployed = proposals
        .iter()
        .any(|proposal| proposal.application_roots.is_some());
    let scopes = proposals
        .iter()
        .filter(|proposal| proposal.application_required)
        .map(|proposal| proposal.target.clone())
        .collect::<Vec<_>>();
    artifact_evaluation::scalar_scopes::prepare_scalar_applications(
        artifact,
        primary,
        deployed.then_some(scopes.as_slice()),
        demand,
        None,
        accounting,
        |scope| {
            let Some(scope) = scope else {
                return Ok(artifact_evaluation::scalar_scopes::ScalarScopeEvaluation {
                    application_roots: None,
                    targets: None,
                    providers: psi_provider_selections::admit_provider_installation(
                        artifact.view(),
                        &[],
                        &[],
                    )
                    .map_err(|error| format!("unscoped scalar provider installation: {error:?}"))?,
                    address_model: Ok(None),
                    carrier_layout: None,
                    layout_target: None,
                    import_realizations: None,
                });
            };
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
            Ok(artifact_evaluation::scalar_scopes::ScalarScopeEvaluation {
                application_roots: proposal.application_roots.clone(),
                layout_target: Some(
                    profile
                        .layout_target_facts()
                        .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                ),
                targets: Some(terminal_target_context(artifact.view(), scope)),
                providers,
                import_realizations: Some(Box::new(realizations)),
                address_model: Ok(address_model),
                carrier_layout: profile.native_realization().map(target_carrier_layout),
            })
        },
    )
    .map(|completed| ProductScalarCompletion {
        completed,
        proposals: proposals.to_vec(),
    })
}

/// Extend the Product completion without replacing its target inputs. A
/// source-free artifact has no such owner and uses `complete_native` instead.
pub fn extend_native(
    retained: &verified_optimization_input::RetainedTerminalArtifact,
    completion: ProductScalarCompletion,
    accounting: &mut impl Extend<artifact_evaluation::scalar_receipts::CanonicalScalarEvaluationReceipt>,
) -> Result<Option<terminal_installation::AdmittedScalarApplications>, String> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("native constant completion needs the retained native proposal")?;
    let demand = proposal
        .entry_scalar_demand(retained.authorization())
        .map_err(diagnostic_text)?;
    if completion.completed.artifact().view().identity() != retained.artifact().view().identity()
        || completion.completed.primary_target() != Some(proposal.target_profile().target_name())
    {
        return Err("native scalar completion changed its artifact or target".into());
    }
    let proposals = proposal.target_provider_proposals();
    if completion.proposals.len() != proposals.len()
        || completion
            .proposals
            .iter()
            .zip(proposals)
            .any(|(previous, selected)| {
                let mut expected = previous.clone();
                expected.application_roots = selected.application_roots.clone();
                expected != *selected
            })
    {
        return Err("native scalar completion changed its retained provider proposal".into());
    }
    let Some(demand) = demand else {
        return Ok(None);
    };
    let deployed = proposals
        .iter()
        .any(|proposal| proposal.application_roots.is_some());
    let scopes = proposals
        .iter()
        .filter(|proposal| proposal.application_required)
        .map(|proposal| proposal.target.clone())
        .collect::<Vec<_>>();
    let roots = proposals
        .iter()
        .filter_map(|proposal| {
            proposal
                .application_roots
                .as_ref()
                .map(|roots| (proposal.target.clone(), roots.clone()))
        })
        .collect::<Vec<_>>();
    completion
        .completed
        .extend(
            deployed.then_some(scopes.as_slice()),
            &roots,
            &demand,
            accounting,
        )
        .map(Some)
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
}

impl TargetRealizedIntrinsics {
    fn retain(
        artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
        selection: &backend_plan::TerminalTargetProviderSelection,
    ) -> Result<Self, String> {
        let selected =
            effects::SelectedProviderPlanFacts::from_selected_plans(selection.plans.clone())?
                .with_assumed_bindings(selection.assumed_bindings.clone());
        Ok(Self {
            artifact: Arc::clone(artifact),
            selected,
        })
    }
}

impl artifact_evaluation::scalar_applications::TargetImportRealizations
    for TargetRealizedIntrinsics
{
    fn admit(
        &self,
        imports: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Box<dyn artifact_evaluation::scalar_applications::AdmittedImportRealizations>, String>
    {
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
            self.selected
                .plans()
                .iter()
                .zip(&unreviewed)
                .map(
                    |(declaration, executions)| admission::CompilerIntrinsicPlanProvenance {
                        plan: declaration,
                        semantic_schema: &declaration.schema,
                        row_executions: executions,
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

impl artifact_evaluation::scalar_applications::AdmittedImportRealizations for AdmittedIntrinsics {
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
