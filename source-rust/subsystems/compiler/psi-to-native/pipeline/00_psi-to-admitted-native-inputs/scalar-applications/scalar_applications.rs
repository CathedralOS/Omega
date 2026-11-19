//! Complete scalar values for an artifact, a deployment and its selected provider inputs.
//! The native driver determines which applications its entry demands. Product completion
//! retains its actual execution inputs and can extend them with newly selected roots.

pub use build_scalar_evaluation::CanonicalScalarEvaluationReceipt as EvaluationReceipt;
use psi_target_layouts::terminal_target_context;
use runtime_abi::target_carrier_layout;
use std::sync::Arc;
use target_provider_admission::TerminalTargetProviderProposal;

mod completed;
pub use completed::ProductScalarCompletion;

pub fn complete<'scope>(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut impl Extend<build_scalar_evaluation::CanonicalScalarEvaluationReceipt>,
) -> Result<psi_scalar_applications::AdmittedScalarApplications, String> {
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
    accounting: &mut impl Extend<build_scalar_evaluation::CanonicalScalarEvaluationReceipt>,
) -> Result<ProductScalarCompletion, String> {
    let deployed = proposals
        .iter()
        .any(|proposal| proposal.application_roots.is_some());
    let scopes = proposals
        .iter()
        .filter(|proposal| proposal.application_required)
        .map(|proposal| proposal.target.clone())
        .collect::<Vec<_>>();
    build_scalar_evaluation::prepare_scalar_applications(
        artifact,
        primary,
        deployed.then_some(scopes.as_slice()),
        demand,
        None,
        accounting,
        |scope| {
            let Some(scope) = scope else {
                return Ok(build_scalar_evaluation::ScalarScopeEvaluation {
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
                    semantic_vocabulary::ScalarAddressModel {
                        target: scope.to_owned(),
                        carrier: model.carrier(),
                        semantics_version: model.semantics_version(),
                        commitment: model.commitment(),
                    }
                });
            Ok(build_scalar_evaluation::ScalarScopeEvaluation {
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
    selected: selected_provider_plans::SelectedProviderPlanFacts,
}

impl TargetRealizedIntrinsics {
    fn retain(
        artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
        selection: &target_provider_admission::TerminalTargetProviderSelection,
    ) -> Result<Self, String> {
        let selected = selected_provider_plans::SelectedProviderPlanFacts::from_selected_plans(
            selection.plans.clone(),
        )?
        .with_assumed_bindings(selection.assumed_bindings.clone());
        Ok(Self {
            artifact: Arc::clone(artifact),
            selected,
        })
    }
}

impl build_scalar_evaluation::TargetImportRealizations for TargetRealizedIntrinsics {
    fn admit(
        &self,
        imports: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Box<dyn build_scalar_evaluation::AdmittedImportRealizations>, String> {
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

impl build_scalar_evaluation::AdmittedImportRealizations for AdmittedIntrinsics {
    fn realizes(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> bool {
        self.0.realizes(boundary)
    }

    fn bind(
        &self,
        instance: &mut terminal_interpreter::TerminalInstance<'_, '_>,
    ) -> Result<(), String> {
        let operations = self
            .0
            .operations_for(instance.program().execution_identity())
            .map_err(|reason| format!("constant evaluation intrinsic binding: {reason}"))?;
        for &(slot, operation) in operations {
            terminal_runtime::embedding::bind_numeric_import_slot(instance, slot, operation)
                .map_err(|reason| format!("constant evaluation intrinsic binding: {reason:?}"))?;
        }
        Ok(())
    }
}
