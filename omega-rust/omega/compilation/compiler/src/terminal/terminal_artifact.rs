/*
We batch and reuse the published `PortableTerminalProgram` across target
applications that share a checked source and Psi optimization projection.

`produce_retained_terminal_artifacts` partitions requests first by shared
checked source (`shares_source`) so target provider publications cover the full
set of requested realization scopes, and then by equal Psi production inputs
(`shares_production`).
*/

/*
For each production cohort, we obtain the shared `PortableTerminalProgram` once
(`produce_shared_terminal`), attach each application's entry and callback roots
(`attach_application`), enforce authored behavior exclusions (`policy.admit`),
and publish the target's provider proposals before restoring the caller's
request order.
*/

use super::behavior_exclusions::TerminalBehaviorExclusions;
use super::product_code::ProductCodeSource;
use crate::checked::CheckedCompilation;
use crate::execution_settlement::SelectedProviderCustody;
use crate::observations::PsiStageTimings;
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use native_compiler::TerminalApplication;
use optimization_core::OptimizationSelections;
use psi_compiler::TerminalApplicationCustody;
use std::sync::Arc;

const TERMINAL_PRODUCTION_STAGE: StageMeta = StageMeta::new(
    "terminal-production",
    "CheckedTrees",
    "CanonicalTerminalArtifact",
);

pub(crate) struct TerminalProductionView<'a> {
    source_identity: compilation_products::ProductSourceIdentity,
    publication: &'a compilation_products::ProductTerminalPublication,
    sources: &'a package_compilation::CheckedSourceCustody,
    build: &'a compilation_products::ComputedBuildConfig,
    provider_settlement: &'a provider_planning::ProviderSettlement,
    selected_target_profile: Option<target::TargetProfile>,
    entry: Option<&'a provider_planning::program_entry::SelectedCompilerProgramEntry>,
    provenance: &'a [provider_planning::SelectedProviderReviewProvenance],
    plan_facts: &'a effects::SelectedProviderPlanFacts,
    native_sources: &'a Result<Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
    callbacks: &'a psi_compiler::PreparedTerminalRoots,
    callback_declarations: &'a psi_compiler::PreparedExternalProviderDeclarations,
    callback_calling_requirements: Vec<terminal_psi::CallbackRequirementId>,
    timings: &'a mut CompileTimings,
    native_components:
        Option<&'a Result<Vec<compilation_products::SelectedIndependentComponent>, String>>,
}

impl<'a> TerminalProductionView<'a> {
    // Roots decide which machines the published Terminal program retains, so
    // two products share a production only when they share that publication
    // as well as checked source: a separately selected test root over the same
    // source must not adopt the application's program.
    fn shares_source(&self, other: &Self) -> bool {
        self.source_identity.same_source(&other.source_identity)
            && self.sources == other.sources
            && std::ptr::eq(self.publication, other.publication)
    }

    pub(crate) fn selection(
        selection: &'a mut SelectedProviderCustody,
        native_components: Option<
            &'a Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
        >,
    ) -> Self {
        Self {
            source_identity: compilation_products::ProductSourceIdentity::new(
                selection.source_inspection.program(),
            ),
            publication: &selection.terminal_publication,
            sources: &selection.sources,
            build: &selection.build,
            provider_settlement: &selection.settlement,
            selected_target_profile: selection.execution.selected_target_profile,
            entry: selection.execution.selected_program_entry.as_ref(),
            provenance: &selection.execution.selected_provider_provenance,
            plan_facts: selection.execution.plans(),
            native_sources: &selection.execution.native_provider_sources,
            callbacks: &selection.execution.callback_roots,
            callback_declarations: &selection.execution.external_providers,
            callback_calling_requirements: selection
                .settlement
                .callback_calling_requirements(&selection.execution.external_providers.callbacks),
            timings: &mut selection.timings,
            native_components,
        }
    }

    pub(crate) fn checked(
        checked: &'a CheckedCompilation,
        timings: &'a mut CompileTimings,
    ) -> Self {
        Self {
            source_identity: compilation_products::ProductSourceIdentity::new(
                checked.source_inspection().program(),
            ),
            publication: checked.terminal_publication(),
            sources: checked.sources(),
            build: checked.build(),
            provider_settlement: checked.providers(),
            selected_target_profile: checked.execution().selected_target_profile,
            entry: checked.execution().selected_program_entry.as_ref(),
            provenance: &checked.execution().selected_provider_provenance,
            plan_facts: checked.execution().plans(),
            native_sources: &checked.execution().native_provider_sources,
            callbacks: &checked.execution().callback_roots,
            callback_declarations: &checked.execution().external_providers,
            callback_calling_requirements: checked
                .providers()
                .callback_calling_requirements(&checked.execution().external_providers.callbacks),
            timings,
            native_components: None,
        }
    }
}

pub(crate) struct TerminalProductRequest<'a> {
    pub view: TerminalProductionView<'a>,
    pub selections: &'a OptimizationSelections,
    pub use_kind: TerminalProductUse<'a>,
}

#[derive(Clone, Copy)]
pub(crate) enum TerminalProductUse<'a> {
    Declarations,
    Inspection {
        name: &'a str,
        source: ProductCodeSource<'a>,
    },
    Application,
}

impl PartialEq for TerminalProductUse<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Declarations, Self::Declarations) | (Self::Application, Self::Application) => {
                true
            }
            (Self::Inspection { name: left, .. }, Self::Inspection { name: right, .. }) => {
                left == right
            }
            _ => false,
        }
    }
}
impl Eq for TerminalProductUse<'_> {}

impl<'a> TerminalProductRequest<'a> {
    pub(crate) fn inspection(
        selection: &'a mut SelectedProviderCustody,
        name: &'a str,
        selections: &'a OptimizationSelections,
    ) -> Self {
        let source = ProductCodeSource {
            program: selection.source_inspection.program(),
            inventory: selection.source_inspection.inventory(),
            sources: &selection.sources,
            build: &selection.build,
            defaults: &selection.provider_defaults,
            publication: &selection.terminal_publication,
        };
        let view = TerminalProductionView {
            source_identity: compilation_products::ProductSourceIdentity::new(
                selection.source_inspection.program(),
            ),
            publication: &selection.terminal_publication,
            sources: &selection.sources,
            build: &selection.build,
            provider_settlement: &selection.settlement,
            selected_target_profile: selection.execution.selected_target_profile,
            entry: selection.execution.selected_program_entry.as_ref(),
            provenance: &selection.execution.selected_provider_provenance,
            plan_facts: selection.execution.plans(),
            native_sources: &selection.execution.native_provider_sources,
            callbacks: &selection.execution.callback_roots,
            callback_declarations: &selection.execution.external_providers,
            callback_calling_requirements: selection
                .settlement
                .callback_calling_requirements(&selection.execution.external_providers.callbacks),
            timings: &mut selection.timings,
            native_components: None,
        };
        Self {
            view,
            selections,
            use_kind: TerminalProductUse::Inspection { name, source },
        }
    }
}

pub(crate) enum ProducedTerminalProduct {
    Descriptive {
        program: Arc<psi_compiler::PortableTerminalProgram>,
        proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    },
    Application(TerminalApplication),
}

impl ProducedTerminalProduct {
    pub(crate) fn module(&self) -> &terminal_psi::TerminalModule {
        match self {
            Self::Descriptive { program, .. } => program.artifact().view().module(),
            Self::Application(application) => application.artifact.view().module(),
        }
    }

    pub(crate) fn into_application(self) -> Result<TerminalApplication, Vec<Diagnostic>> {
        match self {
            Self::Application(application) => Ok(application),
            Self::Descriptive { .. } => Err(vec![Diagnostic::error(
                "declaration checking produced no executable application",
            )]),
        }
    }

    pub(crate) fn check_conditions(
        &self,
        selection: &mut SelectedProviderCustody,
    ) -> Result<(), Vec<Diagnostic>> {
        let (artifact, proposals) = match self {
            Self::Descriptive { program, proposals } => (program.artifact(), proposals),
            Self::Application(_) => {
                return Err(vec![Diagnostic::error(
                    "declaration checking received an executable application",
                )]);
            }
        };
        let target = selection
            .execution
            .selected_target_profile
            .unwrap_or_else(target::TargetProfile::host);
        build_settings::scalar_completion::complete(
            artifact,
            target.target_name(),
            &[],
            proposals,
            &mut selection.source_completion_accounting,
        )
        .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])?;
        Ok(())
    }
}

struct PreparedTerminalProduct<'a> {
    position: usize,
    request: TerminalProductRequest<'a>,
    signature_identity: Option<[u8; 32]>,
    physical_adapter: Option<&'a terminal_psi::PreparedNominalRoot>,
    policy: Option<TerminalBehaviorExclusions<'a>>,
    provider_publication: provider_planning::target_machines::PreparedTargetProviderPublication,
}

impl<'a> PreparedTerminalProduct<'a> {
    fn prepare(
        position: usize,
        request: TerminalProductRequest<'a>,
        requested_scopes: &[String],
    ) -> Result<Self, Vec<Diagnostic>> {
        let view = &request.view;
        if request.use_kind != TerminalProductUse::Application {
            let provider_publication =
                prepare_target_provider_publication(view, request.use_kind, requested_scopes);
            return Ok(Self {
                position,
                request,
                signature_identity: None,
                physical_adapter: None,
                policy: None,
                provider_publication,
            });
        }
        let entry = admit_terminal_production(view.entry, view.provenance, view.native_components)?;
        let signature_identity = entry.source_signature().identity().bytes();
        let policy = TerminalBehaviorExclusions::new(
            view.build.config.behavior_exclusions.as_slice(),
            view.plan_facts,
            view.native_sources,
            entry.source_signature().target_slot().owner,
        );
        let physical_adapter = entry.physical_adapter();
        let provider_publication =
            prepare_target_provider_publication(view, request.use_kind, requested_scopes);
        Ok(Self {
            position,
            request,
            signature_identity: Some(signature_identity),
            physical_adapter,
            policy,
            provider_publication,
        })
    }

    fn shares_production(&self, other: &Self) -> bool {
        self.request.use_kind == other.request.use_kind
            && self.request.view.publication.optimizations().project_psi()
                == other.request.view.publication.optimizations().project_psi()
            && self.request.view.shares_source(&other.request.view)
            && self.request.selections.project_psi() == other.request.selections.project_psi()
    }

    fn prepare_application(&self) -> TerminalApplicationCustody<'a> {
        let entry = self
            .request
            .view
            .entry
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "application has no selected entry declaration",
                )]
            })
            .and_then(|entry| {
                let signature = entry.source_signature();
                terminal_psi::PreparedNominalRoot::new(
                    signature.declaration(),
                    signature.machine_name(),
                    None,
                )
                .map_err(|reason| {
                    psi_compiler::TerminalArtifactProductionError::EntryReceipt(reason.into())
                        .diagnostics()
                })
            });
        TerminalApplicationCustody::prepare(
            entry,
            self.signature_identity,
            self.policy
                .as_ref()
                .map(|_| psi_compiler::TerminalCompositionRoots {
                    callbacks: Some(self.request.view.callbacks),
                    physical_adapter: self.physical_adapter,
                }),
        )
    }
}

pub(crate) fn produce_retained_terminal_artifacts(
    requests: Vec<TerminalProductRequest<'_>>,
) -> Vec<Result<ProducedTerminalProduct, Vec<Diagnostic>>> {
    let mut outcomes = Vec::with_capacity(requests.len());
    let mut source_cohorts: Vec<Vec<(usize, TerminalProductRequest<'_>)>> = Vec::new();
    for (position, request) in requests.into_iter().enumerate() {
        match source_cohorts
            .iter_mut()
            .find(|cohort| cohort[0].1.view.shares_source(&request.view))
        {
            Some(cohort) => cohort.push((position, request)),
            None => source_cohorts.push(vec![(position, request)]),
        }
    }

    for source_requests in source_cohorts {
        let mut requested_scopes = source_requests
            .iter()
            .map(|(_, request)| {
                request
                    .view
                    .selected_target_profile
                    .unwrap_or_else(target::TargetProfile::host)
                    .target_name()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        requested_scopes.sort();
        requested_scopes.dedup();

        let mut cohorts: Vec<Vec<PreparedTerminalProduct<'_>>> = Vec::new();
        let mut realization_scopes = Vec::new();
        for (position, request) in source_requests {
            let application =
                match PreparedTerminalProduct::prepare(position, request, &requested_scopes) {
                    Ok(application) => application,
                    Err(diagnostics) => {
                        outcomes.push((position, Err(diagnostics)));
                        continue;
                    }
                };
            realization_scopes.push(
                application
                    .request
                    .view
                    .selected_target_profile
                    .unwrap_or_else(target::TargetProfile::host)
                    .target_name()
                    .to_owned(),
            );
            match cohorts
                .iter_mut()
                .find(|cohort| cohort[0].shares_production(&application))
            {
                Some(cohort) => cohort.push(application),
                None => cohorts.push(vec![application]),
            }
        }
        realization_scopes.sort();
        realization_scopes.dedup();

        for mut cohort in cohorts {
            let mut timings = std::mem::take(cohort[0].request.view.timings);
            timings.add_completed(TERMINAL_PRODUCTION_STAGE, 0);
            let applications = cohort
                .iter()
                .map(PreparedTerminalProduct::prepare_application)
                .collect::<Vec<_>>();
            let started = std::time::Instant::now();
            let produced = produce_shared_terminal(&cohort, &mut timings);
            let elapsed = started.elapsed().as_micros();
            timings.add_completed(TERMINAL_PRODUCTION_STAGE, elapsed);
            *cohort[0].request.view.timings = timings;
            for application in &mut cohort[1..] {
                application
                    .request
                    .view
                    .timings
                    .add_completed(TERMINAL_PRODUCTION_STAGE, elapsed);
            }
            if cohort[0].request.use_kind != TerminalProductUse::Application {
                match produced {
                    Err(diagnostics) => {
                        for application in cohort {
                            outcomes.push((application.position, Err(diagnostics.clone())));
                        }
                    }
                    Ok(program) => {
                        for application in cohort {
                            let outcome = Ok(ProducedTerminalProduct::Descriptive {
                                proposals: application.provider_publication.publish(
                                    program.artifact().view(),
                                    application.request.selections.identity(),
                                    &realization_scopes,
                                    &application.request.view.callback_declarations.identities,
                                    &application.request.view.callback_declarations.callbacks,
                                    &application.request.view.callback_calling_requirements,
                                ),
                                program: Arc::clone(&program),
                            });
                            outcomes.push((application.position, outcome));
                        }
                    }
                }
                continue;
            }
            match produced {
                Err(diagnostics) => {
                    for application in cohort {
                        outcomes.push((application.position, Err(diagnostics.clone())));
                    }
                }
                Ok(program) => {
                    for (application, custody) in cohort.into_iter().zip(applications) {
                        let mut timings = std::mem::take(application.request.view.timings);
                        let produced = program
                            .attach_application(&custody, &mut PsiStageTimings::new(&mut timings));
                        *application.request.view.timings = timings;
                        let outcome = produced.and_then(|produced| {
                            if let Some(policy) = &application.policy {
                                policy.admit(
                                    program.artifact().semantic_module(),
                                    &mut produced.roots.into_iter(),
                                )?;
                            }
                            let receipt = produced.receipt?.ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "terminal-artifact production retained no checked ProgramEntry receipt",
                                )]
                            })?;
                            Ok(ProducedTerminalProduct::Application(TerminalApplication {
                                physical_entry_adapter: application
                                    .request
                                    .view
                                    .entry
                                    .and_then(|entry| entry.physical_adapter())
                                    .is_some(),
                                provider_proposals: application.provider_publication.publish(
                                    program.artifact().view(),
                                    application.request.selections.identity(),
                                    &realization_scopes,
                                    &application.request.view.callback_declarations.identities,
                                    &application.request.view.callback_declarations.callbacks,
                                    &application.request.view.callback_calling_requirements,
                                ),
                                artifact: Arc::clone(program.artifact()),
                                receipt,
                            }))
                        });
                        outcomes.push((application.position, outcome));
                    }
                }
            }
        }
    }
    outcomes.sort_unstable_by_key(|(position, _)| *position);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

fn produce_shared_terminal(
    cohort: &[PreparedTerminalProduct<'_>],
    timings: &mut CompileTimings,
) -> Result<Arc<psi_compiler::PortableTerminalProgram>, Vec<Diagnostic>> {
    let first = &cohort[0];
    let view = &first.request.view;
    if let TerminalProductUse::Inspection { name, source } = first.request.use_kind {
        let mut observer = PsiStageTimings::new(timings);
        return source.inspect(name, first.request.selections, &mut observer);
    }
    if first.request.use_kind == TerminalProductUse::Application
        && view.build.config.program_roots().is_empty()
    {
        return Err(vec![Diagnostic::error(
            "executable Terminal production has no authored ProgramEntry root",
        )]);
    }
    view.publication.require_published(first.request.selections)
}

fn admit_terminal_production<'a>(
    selected_program_entry: Option<
        &'a provider_planning::program_entry::SelectedCompilerProgramEntry,
    >,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    native_components: Option<
        &Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
    >,
) -> Result<&'a provider_planning::program_entry::SelectedCompilerProgramEntry, Vec<Diagnostic>> {
    if let Some(products) = native_components {
        products
            .as_ref()
            .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
    } else {
        provider_planning::verify_selected_compositions_are_realized(provenance)?;
    }
    selected_program_entry.ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production requires one exact selected program entry",
        )]
    })
}

fn prepare_target_provider_publication(
    source: &TerminalProductionView<'_>,
    use_kind: TerminalProductUse<'_>,
    requested_scopes: &[String],
) -> provider_planning::target_machines::PreparedTargetProviderPublication {
    let selected = match use_kind {
        TerminalProductUse::Inspection { .. } => None,
        _ => Some(&source.provider_settlement.selected_provider_publication),
    };
    source.provider_settlement.provider_publication.prepare(
        requested_scopes,
        selected,
        source.plan_facts.assumed_bindings(),
        &source.provider_settlement.verified_independent_components,
    )
}

pub(crate) fn check_terminal_conditions(
    selection: &mut SelectedProviderCustody,
) -> Result<Option<ProducedTerminalProduct>, Vec<Diagnostic>> {
    if !selection.execution.has_proof_obligations {
        return Ok(None);
    }
    let selections = selection.terminal_publication.optimizations().clone();
    let request = TerminalProductRequest {
        view: TerminalProductionView::selection(selection, None),
        selections: &selections,
        use_kind: TerminalProductUse::Declarations,
    };
    let produced = produce_retained_terminal_artifacts(vec![request])
        .pop()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "declaration checking lost its production",
            )]
        })??;
    produced.check_conditions(selection)?;
    Ok(Some(produced))
}
