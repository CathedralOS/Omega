//! Produce one checked program with all authored roots, then admit each use.
//!
//! Entry selection belongs to an application, not portable production identity.
//! The shared artifact retains every Build root independently of the requested
//! targets. Signatures, exclusion judgments and native proposals stay per use.

use super::behavior_exclusions::TerminalBehaviorExclusions;
use crate::checked::CheckedCompilation;
use crate::execution_settlement::SelectedProviderCustody;
use crate::observations::PsiStageTimings;
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use native_production::TerminalApplication;
use optimization_core::OptimizationSelections;
use psi_compiler::{TerminalApplicationCustody, TerminalProductionCustody};
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
    external_declarations: &'a psi_compiler::PreparedExternalProviderDeclarations,
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
            external_declarations: &selection.execution.external_providers,
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
            external_declarations: &checked.execution().external_providers,
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
struct AdmittedTerminalProduct<'a> {
    position: usize,
    request: TerminalProductRequest<'a>,
    selected_machine: symbols::SymbolHandle,
    signature_identity: Option<[u8; 32]>,
    physical_adapter: symbols::SymbolHandle,
    policy: Option<TerminalBehaviorExclusions<'a>>,
    provider_publication: provider_planning::target_machines::PreparedTargetProviderPublication,
}

impl<'a> AdmittedTerminalProduct<'a> {
    fn admit(
        position: usize,
        request: TerminalProductRequest<'a>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let view = &request.view;
        if request.use_kind != TerminalProductUse::Application {
            let provider_publication = prepare_target_provider_publication(view);
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
        let provider_publication = prepare_target_provider_publication(view);
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
        self.request.use_kind == other.request.use_kind
            && Arc::ptr_eq(self.request.view.program, other.request.view.program)
            && self.request.view.sources == other.request.view.sources
            && self.request.selections.project_psi() == other.request.selections.project_psi()
    }

    fn custody(&self) -> TerminalApplicationCustody<'_> {
        TerminalApplicationCustody {
            callback_roots: Some(self.request.view.callbacks),
            physical_adapter: self.physical_adapter,
            entry_identity: self.signature_identity,
            entry_machine: self.selected_machine.is_valid().then_some(
                psi_compiler::TerminalMachineSelection::Symbol(self.selected_machine),
            ),
            admission: self
                .policy
                .as_ref()
                .map(|policy| policy as &dyn psi_compiler::TerminalCompositionAdmission),
        }
    }
}

/// Invocation-local sharing, not a cache: source custody and checked arenas
/// must identify the same checked owner and exact consumed source. Admission
/// keeps each request with its policy; cohorts own applications, not indices.
pub(crate) fn produce_retained_terminal_artifacts(
    requests: Vec<TerminalProductRequest<'_>>,
) -> Vec<Result<ProducedTerminalProduct, Vec<Diagnostic>>> {
    let mut outcomes = Vec::with_capacity(requests.len());
    let mut cohorts: Vec<Vec<AdmittedTerminalProduct<'_>>> = Vec::new();
    for (position, request) in requests.into_iter().enumerate() {
        let application = match AdmittedTerminalProduct::admit(position, request) {
            Ok(application) => application,
            Err(diagnostics) => {
                outcomes.push((position, Err(diagnostics)));
                continue;
            }
        };
        // Every cohort starts with its first admitted application. A direct scan
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
            match produced.and_then(|program| {
                program
                    .into_shared_portable()
                    .map_err(|error| vec![Diagnostic::error(error.to_string())])
            }) {
                Err(diagnostics) => {
                    for application in cohort {
                        outcomes.push((application.position, Err(diagnostics.clone())));
                    }
                }
                Ok(program) => {
                    let program = Arc::new(program);
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
        match produced.and_then(|produced| {
            produced
                .into_parts()
                .map_err(|error| vec![Diagnostic::error(error.to_string())])
        }) {
            Err(diagnostics) => {
                for application in cohort {
                    outcomes.push((application.position, Err(diagnostics.clone())));
                }
            }
            Ok((artifact, applications)) => {
                let artifact = Arc::new(artifact);
                let mut applications = applications.into_iter();
                for application in cohort {
                    let outcome = match applications.next() {
                        Some(receipt) => receipt.and_then(|receipt| {
                            let receipt = receipt.ok_or_else(|| {
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
                                    artifact.view(),
                                    application.request.selections.identity(),
                                ),
                                artifact: Arc::clone(&artifact),
                                receipt,
                            }))
                        }),
                        None => Err(vec![Diagnostic::error(
                            "Terminal production lost an application outcome",
                        )]),
                    };
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

/// Produce one nonempty source cohort. Application-specific admission borrows
/// each member's retained policy; portable roots and optimization are shared.
fn produce_shared_terminal(
    cohort: &[AdmittedTerminalProduct<'_>],
    timings: &mut CompileTimings,
) -> Result<psi_compiler::ProducedTerminalProgram, Vec<Diagnostic>> {
    let first = &cohort[0];
    let view = &first.request.view;
    let checked_program = view.program;
    let typed = &checked_program.typed;
    let boundary_roots = retained_boundary_roots(view)?;
    if first.request.use_kind != TerminalProductUse::Application {
        let machine = match first.request.use_kind {
            TerminalProductUse::Inspection(name) => {
                psi_compiler::TerminalMachineSelection::Name(name)
            }
            _ => psi_compiler::TerminalMachineSelection::Declarations,
        };
        return produce_terminal(
            view,
            first.request.selections,
            machine,
            psi_compiler::TerminalProgramRoots {
                machines: &[],
                boundaries: &boundary_roots,
            },
            &[],
            timings,
        );
    }
    let mut machines = view.build.config.program_roots();
    if machines.is_empty() {
        machines.push(first.selected_machine);
    }
    machines.extend(
        typed
            .constant_recipe_closure
            .receipts
            .iter()
            .filter(|receipt| receipt.provider_machine.is_valid())
            .map(|receipt| receipt.provider_machine),
    );
    // Normalize the default to its portable family head; application receipts
    // still name the exact selected declaration.
    let default = machines[0];
    let default = typed
        .target_machine_families
        .iter()
        .find(|family| {
            (family.canonical == default
                || family
                    .members
                    .iter()
                    .any(|member| member.machine == default))
                && typed
                    .machine_by_symbol(family.canonical)
                    .is_some_and(|machine| machine.target_family_interface)
        })
        .map_or(default, |family| family.canonical);
    let applications = cohort
        .iter()
        .map(AdmittedTerminalProduct::custody)
        .collect::<Vec<_>>();
    produce_terminal(
        view,
        first.request.selections,
        psi_compiler::TerminalMachineSelection::Symbol(default),
        psi_compiler::TerminalProgramRoots {
            machines: &machines,
            boundaries: &boundary_roots,
        },
        &applications,
        timings,
    )
}

fn retained_boundary_roots(
    view: &TerminalProductionView<'_>,
) -> Result<Vec<symbols::SymbolHandle>, Vec<Diagnostic>> {
    let typed = &view.program.typed;
    let context =
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut boundary_roots = provider_planning::target_machines::provider_selection_roots(
        context,
        view.provider_defaults
            .iter()
            .flat_map(|row| &row.selections)
            .chain(&view.build.config.provider_selections),
    )?;
    // Nonexecuting assumption operands retain both declarations, including
    // intermediate links. Installation owns runtime supply and root consent.
    for relationship in &view.build.config.assumed_bindings {
        boundary_roots.extend([relationship.required, relationship.supplied]);
    }
    // Folded uses may be the last reference to these requirements. Retain their
    // catalog custody without inventing calls.
    boundary_roots.extend(
        typed
            .constant_recipe_closure
            .receipts
            .iter()
            .filter(|receipt| receipt.provider_machine.is_valid())
            .map(|receipt| receipt.requirement),
    );
    boundary_roots.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    boundary_roots.dedup();
    Ok(boundary_roots)
}

fn produce_terminal(
    view: &TerminalProductionView<'_>,
    selections: &OptimizationSelections,
    machine: psi_compiler::TerminalMachineSelection<'_>,
    roots: psi_compiler::TerminalProgramRoots<'_>,
    applications: &[TerminalApplicationCustody<'_>],
    timings: &mut CompileTimings,
) -> Result<psi_compiler::ProducedTerminalProgram, Vec<Diagnostic>> {
    psi_compiler::TerminalProductionRequest {
        publication: terminal_codec::TerminalPublication::Portable,
        checked: view.program,
        external_declarations: Some(view.external_declarations),
        exact_toolchain_sources: view.sources.exact_toolchain_sources(),
        machine,
        roots,
        optimization_selections: selections.project_psi().selections().clone(),
    }
    .produce(
        TerminalProductionCustody { applications },
        &mut PsiStageTimings::new(timings),
    )
    .map_err(|error| error.diagnostics())
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
) -> provider_planning::target_machines::PreparedTargetProviderPublication {
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
        origins: source.provider_inventory.origins(),
        context: provider_planning::ProviderPlanningContext::new(
            &source.program.typed,
            source::DependencyScope::Product,
        ),
        defaults: source.provider_defaults,
        settlement: source.provider_settlement,
    };
    let targets = targets
        .into_iter()
        .map(|target| {
            let prepared = inventory.prepare(&target);
            (target, prepared)
        })
        .collect::<Vec<_>>();
    provider_planning::target_machines::TargetProviderPreparationRequest {
        context: provider_planning::ProviderPlanningContext::new(
            &source.program.typed,
            source::DependencyScope::Product,
        ),
        exact_toolchain_sources: source.sources.exact_toolchain_sources(),
        overrides: &source.build.config.provider_selections,
        assumed_bindings: source.plan_facts.assumed_bindings(),
        independent_components: &source.provider_settlement.verified_independent_components,
        targets: &targets,
    }
    .prepare()
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
    let selections = OptimizationSelections::default();
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
