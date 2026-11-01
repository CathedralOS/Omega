//! Produce one checked program with all authored roots, then admit each use.
//!
//! Entry selection belongs to an application, not portable production identity.
//! The shared artifact retains every Build root independently of the requested
//! targets. Signatures, exclusion judgments and native proposals stay per use.

use super::behavior_exclusions::TerminalBehaviorExclusions;
use super::product_code::ProductCodeSource;
use crate::checked::CheckedCompilation;
use crate::execution_settlement::SelectedProviderCustody;
use crate::observations::PsiStageTimings;
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use native_production::TerminalApplication;
use optimization_core::OptimizationSelections;
use psi_compiler::TerminalApplicationCustody;
use std::sync::Arc;

const TERMINAL_PRODUCTION_STAGE: StageMeta = StageMeta::new(
    "terminal-production",
    "CheckedTrees",
    "CanonicalTerminalArtifact",
);

/// Production borrows precisely the source, entry and provider facts it reads.
/// Review descriptions and new product selections supply this same view; the
/// producer does not branch on the caller's installation history.
pub(crate) struct TerminalProductionView<'a> {
    program: &'a Arc<checked_trees::CheckedTrees>,
    provider_inventory: &'a checked_trees::PreparedProviderInventory,
    publication: &'a compilation_products::ProductTerminalPublication,
    sources: &'a package_compilation::CheckedSourceCustody,
    build: &'a compilation_products::ComputedBuildConfig,
    provider_defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
    provider_settlement: &'a provider_planning::ProviderSettlement,
    selected_target_profile: Option<target::TargetProfile>,
    entry: Option<&'a provider_planning::program_entry::SelectedCompilerProgramEntry>,
    provenance: &'a [provider_planning::SelectedProviderReviewProvenance],
    plan_facts: &'a effects::SelectedProviderPlanFacts,
    native_sources: &'a Result<Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
    callbacks: &'a psi_compiler::PreparedTerminalRoots,
    timings: &'a mut CompileTimings,
    native_components:
        Option<&'a Result<Vec<compilation_products::SelectedIndependentComponent>, String>>,
}

impl<'a> TerminalProductionView<'a> {
    pub(crate) fn selection(
        selection: &'a mut SelectedProviderCustody,
        native_components: Option<
            &'a Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
        >,
    ) -> Self {
        Self {
            program: selection.execution.provider_source.program(),
            provider_inventory: selection.execution.provider_source.inventory(),
            publication: &selection.terminal_publication,
            sources: &selection.sources,
            build: &selection.build,
            provider_defaults: &selection.provider_defaults,
            provider_settlement: &selection.settlement,
            selected_target_profile: selection.execution.selected_target_profile,
            entry: selection.execution.selected_program_entry.as_ref(),
            provenance: &selection.execution.selected_provider_provenance,
            plan_facts: selection.execution.provider_source.plans(),
            native_sources: &selection.execution.native_provider_sources,
            callbacks: &selection.execution.callback_roots,
            timings: &mut selection.timings,
            native_components,
        }
    }

    pub(crate) fn checked(
        checked: &'a CheckedCompilation,
        timings: &'a mut CompileTimings,
    ) -> Self {
        Self {
            program: checked.execution().provider_source.program(),
            provider_inventory: checked.execution().provider_source.inventory(),
            publication: checked.terminal_publication(),
            sources: checked.sources(),
            build: checked.build(),
            provider_defaults: checked.provider_defaults(),
            provider_settlement: checked.providers(),
            selected_target_profile: checked.execution().selected_target_profile,
            entry: checked.execution().selected_program_entry.as_ref(),
            provenance: &checked.execution().selected_provider_provenance,
            plan_facts: checked.execution().provider_source.plans(),
            native_sources: &checked.execution().native_provider_sources,
            callbacks: &checked.execution().callback_roots,
            timings,
            native_components: None,
        }
    }

    fn code_source(&self) -> ProductCodeSource<'_> {
        ProductCodeSource {
            program: self.program,
            inventory: self.provider_inventory,
            sources: self.sources,
            build: self.build,
            defaults: self.provider_defaults,
            publication: self.publication,
        }
    }
}

pub(crate) struct TerminalProductRequest<'a> {
    pub view: TerminalProductionView<'a>,
    pub selections: &'a OptimizationSelections,
    pub use_kind: TerminalProductUse<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalProductUse<'a> {
    Declarations,
    Inspection(&'a str),
    Application,
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
        build_evaluation::scalar_completion::complete(
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

/// Admission retains the request's entry and composition policy together.
/// The position is only for restoring caller order after source-cohort work;
/// it never indexes another table to recover semantic identity.
struct PreparedTerminalProduct<'a> {
    position: usize,
    request: TerminalProductRequest<'a>,
    selected_machine: symbols::SymbolHandle,
    signature_identity: Option<[u8; 32]>,
    physical_adapter: symbols::SymbolHandle,
    policy: Option<TerminalBehaviorExclusions<'a>>,
    provider_publication: provider_planning::target_machines::PreparedTargetProviderPublication,
}

impl<'a> PreparedTerminalProduct<'a> {
    fn prepare(
        position: usize,
        request: TerminalProductRequest<'a>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let view = &request.view;
        if request.use_kind != TerminalProductUse::Application {
            let provider_publication = prepare_target_provider_publication(view)?;
            return Ok(Self {
                position,
                request,
                selected_machine: Default::default(),
                signature_identity: None,
                physical_adapter: Default::default(),
                policy: None,
                provider_publication,
            });
        }
        let entry = admit_terminal_production(view.entry, view.provenance, view.native_components)?;
        let selected_machine = entry.machine_symbol();
        let signature_identity = entry.source_signature().identity().bytes();
        let policy = TerminalBehaviorExclusions::new(
            view.build.config.behavior_exclusions.as_slice(),
            view.plan_facts,
            view.native_sources,
            entry.source_signature().target_slot().owner,
        );
        // Firmware starts in the selected adapter, not the semantic continuation.
        // Admit its behavior and callbacks while retaining the semantic receipt.
        let physical_adapter = entry.physical_adapter().unwrap_or_default();
        let provider_publication = prepare_target_provider_publication(view)?;
        Ok(Self {
            position,
            request,
            selected_machine,
            signature_identity: Some(signature_identity),
            physical_adapter,
            policy,
            provider_publication,
        })
    }

    fn shares_production(&self, other: &Self) -> bool {
        // The publication slot is per-target custody, not product identity:
        // equal fixed Psi projections let the cohort produce through the first
        // member's slot while each report keeps its own admissions.
        self.request.use_kind == other.request.use_kind
            && self.request.view.publication.optimizations().project_psi()
                == other.request.view.publication.optimizations().project_psi()
            && Arc::ptr_eq(self.request.view.program, other.request.view.program)
            && self.request.view.sources == other.request.view.sources
            && self.request.selections.project_psi() == other.request.selections.project_psi()
    }

    fn prepare_application(
        &self,
        observer: &mut impl psi_compiler::PsiCompilationObserver,
    ) -> TerminalApplicationCustody<'a> {
        TerminalApplicationCustody::prepare(
            self.request.view.program,
            psi_compiler::TerminalMachineSelection::Symbol(self.selected_machine),
            self.signature_identity,
            self.policy
                .as_ref()
                .map(|_| psi_compiler::TerminalCompositionRoots {
                    callbacks: Some(self.request.view.callbacks),
                    physical_adapter: self.physical_adapter,
                }),
            self.request.view.sources.exact_toolchain_sources(),
            observer,
        )
    }
}

/// Invocation-local sharing, not a cache: source custody and checked arenas
/// must identify the same checked owner and exact consumed source. Preparation
/// keeps each request with its policy; cohorts own applications, not indices.
pub(crate) fn produce_retained_terminal_artifacts(
    requests: Vec<TerminalProductRequest<'_>>,
) -> Vec<Result<ProducedTerminalProduct, Vec<Diagnostic>>> {
    let mut outcomes = Vec::with_capacity(requests.len());
    let mut cohorts: Vec<Vec<PreparedTerminalProduct<'_>>> = Vec::new();
    for (position, request) in requests.into_iter().enumerate() {
        let application = match PreparedTerminalProduct::prepare(position, request) {
            Ok(application) => application,
            Err(diagnostics) => {
                outcomes.push((position, Err(diagnostics)));
                continue;
            }
        };
        // Every cohort starts with its first prepared application. A direct scan
        // is sufficient for the small target roster; no source hashing or copying.
        match cohorts
            .iter_mut()
            .find(|cohort| cohort[0].shares_production(&application))
        {
            Some(cohort) => cohort.push(application),
            None => cohorts.push(vec![application]),
        }
    }

    for mut cohort in cohorts {
        let mut timings = std::mem::take(cohort[0].request.view.timings);
        timings.add_completed(TERMINAL_PRODUCTION_STAGE, 0);
        let applications = {
            let mut observer = PsiStageTimings::new(&mut timings);
            cohort
                .iter()
                .map(|application| application.prepare_application(&mut observer))
                .collect::<Vec<_>>()
        };
        let started = std::time::Instant::now();
        let produced = produce_shared_terminal(&cohort, &mut timings);
        let elapsed = started.elapsed().as_micros();
        timings.add_completed(TERMINAL_PRODUCTION_STAGE, elapsed);
        *cohort[0].request.view.timings = timings;
        for application in &mut cohort[1..] {
            // Each report shows the shared cost; stage rows remain on the
            // producing request, not fabricated per-target executions.
            application
                .request
                .view
                .timings
                .add_completed(TERMINAL_PRODUCTION_STAGE, elapsed);
        }
        // Descriptive consumers borrow one portable owner. Inspection also
        // consumes its exact source-issued Build companions; checking borrows
        // only the artifact. Neither creates an application receipt.
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
                                policy.admit(program.artifact().semantic_module(), &mut produced.roots.into_iter())?;
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
    // Cohorts can interleave in the input. Restore only the externally promised
    // request order; all semantic associations traveled with the application.
    outcomes.sort_unstable_by_key(|(position, _)| *position);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

/// Produce one nonempty source cohort. Source-root projection and optimization
/// are shared; each product checks its retained policy after publication.
fn produce_shared_terminal(
    cohort: &[PreparedTerminalProduct<'_>],
    timings: &mut CompileTimings,
) -> Result<Arc<psi_compiler::PortableTerminalProgram>, Vec<Diagnostic>> {
    let first = &cohort[0];
    let view = &first.request.view;
    let code = view.code_source();
    let mut observer = PsiStageTimings::new(timings);
    if let TerminalProductUse::Inspection(name) = first.request.use_kind {
        return code.inspect(name, first.request.selections, &mut observer);
    }
    if first.request.use_kind == TerminalProductUse::Application
        && view.build.config.program_roots().is_empty()
    {
        return Err(vec![Diagnostic::error(
            "executable Terminal production has no authored ProgramEntry root",
        )]);
    }
    code.publish(first.request.selections, &mut observer)
}

/// Bare Terminal publication still has no component process carrier. A native
/// request may produce its portable input only when the exact independently
/// verified products are retained; actual native/ABI/resource/loader admission
/// follows source-free. Catalog code retention grants no local provider supply.
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
    // Preserve the build operand's lexical package choice, including when
    // another package contains a machine with the same display name.
    selected_program_entry.ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production requires one exact selected program entry",
        )]
    })
}

/// Prepare the Build companion while checked custody is available. Selection
/// waits for the emitted demand catalog: selecting every source candidate here
/// would demand unused providers. Preparation retains unavailable targets as
/// deferred refusal rows, never cohort failures.
fn prepare_target_provider_publication(
    source: &TerminalProductionView<'_>,
) -> Result<provider_planning::target_machines::PreparedTargetProviderPublication, Vec<Diagnostic>>
{
    let mut targets = source
        .provider_defaults
        .iter()
        .filter_map(|row| row.target.clone())
        .collect::<Vec<_>>();
    targets.extend(
        source
            .build
            .config
            .provider_selections
            .iter()
            .filter_map(|selection| {
                selection
                    .target
                    .map(|target| target.canonical_name().to_owned())
            }),
    );
    for binding in &source.build.config.root_bindings {
        if let Some((owner, _)) = binding.slot.rsplit_once("::")
            && let Ok(profile) = target::TargetProfile::from_root_slot_owner(owner)
        {
            targets.push(profile.target_name().to_owned());
        }
    }
    targets.push(
        source
            .selected_target_profile
            .unwrap_or_else(target::TargetProfile::host)
            .target_name()
            .to_owned(),
    );
    targets.extend(
        checked_trees::ProofApplications::new(
            &source.program.facts.proof_applications,
            &source.program.facts.operational,
        )
        .scopes(
            source
                .selected_target_profile
                .unwrap_or_else(target::TargetProfile::host)
                .target_name(),
        )
        .into_iter()
        .map(str::to_owned),
    );
    targets.sort();
    targets.dedup();
    let inventory = provider_planning::target_machines::TargetProviderInventoryRequest {
        body_declarations: source.provider_inventory.direct_calls(),
        origins: source.provider_inventory.origins(),
        services: source
            .provider_inventory
            .services_for(source.program)
            .map_err(|error| vec![error])?,
        defaults: source.provider_defaults,
        authored: source.provider_settlement.authored_providers(),
        calling_plans: &source
            .provider_settlement
            .boundary_calling_plan_realizations,
        bindings: &source.provider_settlement.evaluated_via_bindings,
    };
    let targets = targets
        .into_iter()
        .map(|target| {
            let prepared = inventory.prepare(&target);
            (target, prepared)
        })
        .collect::<Vec<_>>();
    let deployment = source.build.config.provider_deployment()?;
    Ok(
        provider_planning::target_machines::TargetProviderPreparationRequest {
            application_roots: deployment.roots.as_deref(),
            application_scopes: &deployment.scopes,
            context: provider_planning::ProviderPlanningContext::new(
                &source.program.typed,
                source::DependencyScope::Product,
            ),
            exact_toolchain_sources: source.sources.exact_toolchain_sources(),
            overrides: &source.build.config.provider_selections,
            assumed_bindings: source.plan_facts.assumed_bindings(),
            independent_components: &source.provider_settlement.verified_independent_components,
            selected: Some(&source.provider_settlement.selected_provider_publication),
            targets: &targets,
        }
        .prepare(),
    )
}

/// Check declarations through the ordinary Terminal producer without inventing
/// an executable product entry. Only the retained conditions demand evaluation.
pub(crate) fn check_terminal_conditions(
    selection: &mut SelectedProviderCustody,
) -> Result<Option<ProducedTerminalProduct>, Vec<Diagnostic>> {
    if !selection
        .execution
        .provider_source
        .program()
        .facts
        .proof_applications
        .has_obligations
    {
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
