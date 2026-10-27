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
    sources: &'a package_compilation::CheckedSourceCustody,
    build: &'a compilation_products::ComputedBuildConfig,
    provider_defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
    provider_settlement: &'a provider_planning::ProviderSettlement,
    selected_target_profile: Option<target::TargetProfile>,
    entry: Option<&'a provider_planning::program_entry::SelectedCompilerProgramEntry>,
    provenance: &'a [provider_planning::SelectedProviderReviewProvenance],
    plan_facts: &'a effects::SelectedProviderPlanFacts,
    callbacks: &'a [provider_planning::calling_policy_plans::SelectedCallbackPlacement],
    timings: &'a mut CompileTimings,
    native_components: Option<&'a [Arc<compilation_products::IndependentComponentProduct>]>,
    component_verdicts: &'a [Arc<component_description::VerifiedComponent>],
}

impl<'a> TerminalProductionView<'a> {
    pub(crate) fn selection(
        selection: &'a mut SelectedProviderCustody,
        native_components: Option<&'a [Arc<compilation_products::IndependentComponentProduct>]>,
    ) -> Self {
        Self {
            program: &selection.checked.program,
            sources: &selection.sources,
            build: &selection.build,
            provider_defaults: &selection.provider_defaults,
            provider_settlement: &selection.settlement,
            selected_target_profile: selection.selected_target_profile,
            entry: selection.selected_program_entry.as_ref(),
            provenance: &selection.selected_provider_provenance,
            plan_facts: &selection.checked.selected_provider_plan_facts,
            callbacks: &selection.checked.callback_placements,
            timings: &mut selection.timings,
            native_components,
            component_verdicts: &selection.settlement.verified_independent_components,
        }
    }

    pub(crate) fn checked(
        checked: &'a CheckedCompilation,
        timings: &'a mut CompileTimings,
    ) -> Self {
        Self {
            program: &checked.execution().program,
            sources: checked.sources(),
            build: checked.build(),
            provider_defaults: checked.provider_defaults(),
            provider_settlement: checked.providers(),
            selected_target_profile: checked.execution().selected_target_profile,
            entry: checked.execution().selected_program_entry.as_ref(),
            provenance: &checked.execution().selected_provider_provenance,
            plan_facts: &checked.execution().selected_provider_plan_facts,
            callbacks: &checked.execution().callback_placements,
            timings,
            native_components: None,
            component_verdicts: &checked.providers().verified_independent_components,
        }
    }
}

pub(crate) struct TerminalProductRequest<'a> {
    pub view: TerminalProductionView<'a>,
    pub selections: &'a OptimizationSelections,
}

/// One target's produced canonical artifact beside the entry receipt its
/// installation and native proposal consume. The artifact is shared storage;
/// the receipt and Build provider companion belong to this application. No
/// source owner is needed to recover provider proposals after production.
pub(crate) struct ProducedTerminalApplication {
    pub artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub receipt: terminal_psi::CheckedProgramEntryTerminalReceipt,
    pub provider_proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
    /// Authored adapter selection, not authority to enter it. Completion still
    /// binds the exact physical arrival contract against canonical candidates.
    pub physical_entry_adapter: bool,
}

/// Admission retains the request's entry and composition policy together.
/// The position is only for restoring caller order after source-cohort work;
/// it never indexes another table to recover semantic identity.
struct AdmittedTerminalApplication<'a> {
    position: usize,
    request: TerminalProductRequest<'a>,
    selected_machine: symbols::SymbolHandle,
    signature_identity: [u8; 32],
    admission_roots: Vec<symbols::SymbolHandle>,
    policy: Option<TerminalBehaviorExclusions<'a>>,
}

impl<'a> AdmittedTerminalApplication<'a> {
    fn admit(
        position: usize,
        request: TerminalProductRequest<'a>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let view = &request.view;
        let entry = admit_terminal_production(
            view.entry,
            view.provenance,
            view.native_components,
            view.component_verdicts,
        )?;
        let selected_machine = entry.machine_symbol();
        let signature_identity = entry.source_signature().identity().bytes();
        let policy = TerminalBehaviorExclusions::new(
            view.build.config.behavior_exclusions.as_slice(),
            view.plan_facts,
            entry.source_signature().target_slot().owner,
        );
        // Firmware starts in the selected adapter, not the semantic continuation.
        // Admit its behavior and callbacks while retaining the semantic receipt.
        let admission_roots = view
            .callbacks
            .iter()
            .map(|placement| placement.selected_machine)
            .chain(entry.physical_adapter())
            .collect();
        Ok(Self {
            position,
            request,
            selected_machine,
            signature_identity,
            admission_roots,
            policy,
        })
    }

    fn shares_production(&self, other: &Self) -> bool {
        crate::execution_settlement::same_checked_program(
            self.request.view.program,
            other.request.view.program,
        ) && self.request.view.sources == other.request.view.sources
            && self.request.selections.project_psi() == other.request.selections.project_psi()
    }

    fn custody(&self) -> TerminalApplicationCustody<'_> {
        TerminalApplicationCustody {
            admission_roots: &self.admission_roots,
            entry_identity: Some(self.signature_identity),
            entry_machine: Some(psi_compiler::TerminalMachineSelection::Symbol(
                self.selected_machine,
            )),
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
) -> Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>> {
    let mut outcomes = Vec::with_capacity(requests.len());
    let mut cohorts: Vec<Vec<AdmittedTerminalApplication<'_>>> = Vec::new();
    for (position, request) in requests.into_iter().enumerate() {
        let application = match AdmittedTerminalApplication::admit(position, request) {
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
                            Ok(ProducedTerminalApplication {
                                physical_entry_adapter: application
                                    .request
                                    .view
                                    .entry
                                    .and_then(|entry| entry.physical_adapter())
                                    .is_some(),
                                provider_proposals: produce_target_provider_proposals(
                                    &application.request.view,
                                    &artifact,
                                    application.request.selections,
                                ),
                                artifact: Arc::clone(&artifact),
                                receipt,
                            })
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
    cohort: &[AdmittedTerminalApplication<'_>],
    timings: &mut CompileTimings,
) -> Result<psi_compiler::ProducedTerminalProgram, Vec<Diagnostic>> {
    let first = &cohort[0];
    let view = &first.request.view;
    let checked_program = view.program;
    let typed = &checked_program.typed;
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
        .map(AdmittedTerminalApplication::custody)
        .collect::<Vec<_>>();
    psi_compiler::TerminalProductionRequest {
        publication: terminal_codec::TerminalPublication::Portable,
        checked: checked_program,
        exact_toolchain_sources: view.sources.exact_toolchain_sources(),
        machine: psi_compiler::TerminalMachineSelection::Symbol(default),
        roots: psi_compiler::TerminalProgramRoots {
            machines: &machines,
            boundaries: &boundary_roots,
        },
        optimization_selections: first.request.selections.project_psi().selections().clone(),
    }
    .produce(
        TerminalProductionCustody {
            applications: &applications,
        },
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
    native_components: Option<&[Arc<compilation_products::IndependentComponentProduct>]>,
    verdicts: &[Arc<component_description::VerifiedComponent>],
) -> Result<&'a provider_planning::program_entry::SelectedCompilerProgramEntry, Vec<Diagnostic>> {
    if let Some(products) = native_components {
        for selected in provenance {
            let mode = selected
                .selected_by
                .composition_mode()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if mode == provider_planning::CompositionMode::Fused {
                continue;
            }
            let mut candidates = verdicts
                .iter()
                .filter(|component| component.realizes_selected_plan(&selected.plan).is_ok());
            let component = candidates.next().ok_or_else(|| {
                vec![Diagnostic::error(
                    "independent native production has no exact verified component",
                )]
            })?;
            if candidates.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "independent native production has competing component verdicts",
                )]);
            }
            let mut matching = products.iter().filter(|product| {
                product.description().expected_subject() == component.subject()
                    && product.retained().artifact() == component.artifact()
                    && product
                        .retained()
                        .native_realization_proposal()
                        .is_some_and(|proposal| {
                            proposal.target_profile().target_name() == selected.plan.target
                        })
            });
            if matching.next().is_none() || matching.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "independent native production requires exactly one complete retained producing product",
                )]);
            }
        }
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

/// Complete the Build companion against the exact emitted demand catalog while
/// the source-producing operation still owns checked custody. This must follow
/// Psi production: selecting against every source candidate would demand unused
/// providers. Retain unavailable targets as refusal rows, never cohort failures.
fn produce_target_provider_proposals(
    source: &TerminalProductionView<'_>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    selections: &OptimizationSelections,
) -> Vec<terminal_psi_to_abstract_operations::TerminalTargetProviderProposal> {
    let mut targets = source
        .provider_defaults
        .iter()
        .filter_map(|row| row.target.clone())
        .collect::<Vec<_>>();
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
    targets.sort();
    targets.dedup();
    provider_planning::target_machines::TargetProviderPublicationRequest {
        context: provider_planning::ProviderPlanningContext::new(
            &source.program.typed,
            source::DependencyScope::Product,
        ),
        artifact: artifact.view(),
        exact_toolchain_sources: source.sources.exact_toolchain_sources(),
        defaults: source.provider_defaults,
        overrides: &source.build.config.provider_selections,
        assumed_bindings: source.plan_facts.assumed_bindings(),
        settlement: source.provider_settlement,
        targets: &targets,
        optimization_selection: selections.identity(),
    }
    .publish()
}

/// Assemble one target's retained terminal product after installation: the
/// shared artifact, its per-application entry receipt projected to the native
/// proposal, and the same target's provider proposals. Proposal construction
/// consumes explicit installed inputs — placements, establishments and grants
/// are installation products. Build provider proposals and adapter intent already
/// belong to the produced application; completion has no checked-source input.
pub(crate) fn finish_terminal_application(
    produced: ProducedTerminalApplication,
    request: terminal_psi_to_abstract_operations::TerminalNativeProposalRequest<'_>,
) -> Result<compilation_products::RetainedTerminalArtifact, Vec<Diagnostic>> {
    let mut proposal =
        terminal_psi_to_abstract_operations::project_terminal_native_realization_proposal(
            &produced.artifact,
            produced.receipt,
            request,
        )?;
    if produced.physical_entry_adapter {
        proposal
            .bind_physical_adapter(&produced.artifact)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }
    Ok(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            produced.artifact,
            proposal.with_target_provider_proposals(produced.provider_proposals),
        ),
    )
}
