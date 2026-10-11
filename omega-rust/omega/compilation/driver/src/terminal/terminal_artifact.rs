//! Produce one checked program with all authored roots, then admit each use.
//!
//! Entry selection belongs to an application, not portable production identity.
//! The shared artifact retains every Build root independently of the requested
//! targets. Signatures, exclusion judgments and native proposals stay per use.

use crate::checked::CheckedCompilation;
use crate::execution_settlement::SelectedProviderCustody;
use crate::terminal::native_proposal;
use artifacts::allocations::AllocationDelta;
use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use std::sync::Arc;
use terminal_production::{TerminalApplicationCustody, TerminalProductionCustody};

const TERMINAL_PRODUCTION_STAGE: StageMeta = StageMeta::new(
    "terminal-production",
    "CheckedTrees",
    "CanonicalTerminalArtifact",
    TimingCategory::Pipeline,
);

/// The producing half's view into per-target realization custody. Standalone
/// compilation produces between selection and installation; the package route
/// arrives with the installed checked compilation already sealed. Both expose
/// the same read set — production consumes nothing installation mints.
pub(crate) enum TerminalProductionView<'a> {
    Selection(&'a mut SelectedProviderCustody),
    Installed(&'a mut CheckedCompilation),
}

impl TerminalProductionView<'_> {
    fn sources(&self) -> &package_compilation::CheckedSourceCustody {
        match self {
            Self::Selection(selection) => &*selection.sources,
            Self::Installed(checked) => checked.sources(),
        }
    }

    fn build(&self) -> &compilation_products::ComputedBuildConfig {
        match self {
            Self::Selection(selection) => &*selection.build,
            Self::Installed(checked) => checked.build(),
        }
    }

    fn provider_defaults(
        &self,
    ) -> &[build_time_evaluation::provider_declarations::TargetProviderDefaults] {
        match self {
            Self::Selection(selection) => &selection.provider_defaults,
            Self::Installed(checked) => checked.provider_defaults(),
        }
    }

    fn timings(&mut self) -> &mut CompileTimings {
        match self {
            Self::Selection(selection) => &mut selection.timings,
            Self::Installed(checked) => checked.timings_mut(),
        }
    }
}

pub(crate) struct TerminalProductRequest<'a> {
    pub view: TerminalProductionView<'a>,
    pub selections: &'a OptimizationSelections,
}

/// One target's produced canonical artifact beside the entry receipt its
/// installation and native proposal consume. The artifact is shared storage;
/// the receipt is this application's own.
pub(crate) struct ProducedTerminalApplication {
    pub artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub receipt: terminal_psi::CheckedProgramEntryTerminalReceipt,
}

struct TerminalEntry {
    selected_machine: symbols::SymbolHandle,
    signature_identity: [u8; 32],
    target: target::TargetProfile,
}

/// Invocation-local sharing, not a cache: source custody and checked arenas
/// must be the same owners, not merely equal-looking programs. The small
/// target roster is grouped by direct scans; no source hashing or copying.
pub(crate) fn produce_retained_terminal_artifacts(
    mut requests: Vec<TerminalProductRequest<'_>>,
) -> Vec<Result<ProducedTerminalApplication, Vec<Diagnostic>>> {
    let entries: Vec<_> = requests
        .iter()
        .map(|request| {
            let (entry, provenance) = match &request.view {
                TerminalProductionView::Selection(selection) => (
                    selection.selected_program_entry.as_ref(),
                    selection.selected_provider_provenance.as_slice(),
                ),
                TerminalProductionView::Installed(checked) => (
                    checked.execution().selected_program_entry.as_ref(),
                    checked.execution().selected_provider_provenance.as_slice(),
                ),
            };
            admit_terminal_production(entry, provenance).map(|entry| {
                let selected_machine = entry.machine_symbol();
                TerminalEntry {
                    selected_machine,
                    signature_identity: entry.source_signature().identity().bytes(),
                    target: entry.source_signature().target_slot().owner,
                }
            })
        })
        .collect();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut results: Vec<_> = entries
        .iter()
        .map(|entry| entry.as_ref().err().cloned().map(Err))
        .collect();
    for (request_index, entry) in entries.iter().enumerate() {
        let Ok(_) = entry else {
            continue;
        };
        let request = &requests[request_index];
        let program_pointer = |view: &TerminalProductionView<'_>| match view {
            TerminalProductionView::Selection(selection) => {
                Arc::as_ptr(&selection.checked.program) as *const ()
            }

            TerminalProductionView::Installed(checked) => {
                Arc::as_ptr(&checked.execution().program) as *const ()
            }
        };
        let group = groups.iter_mut().find(|group| {
            let first_index = group[0];
            let first = &requests[first_index];
            program_pointer(&first.view) == program_pointer(&request.view)
                && std::ptr::eq(first.view.sources(), request.view.sources())
                && first.selections.project_psi() == request.selections.project_psi()
        });
        if let Some(group) = group {
            group.push(request_index);
        } else {
            groups.push(vec![request_index]);
        }
    }

    for group in groups {
        let first_index = group[0];
        let mut timings = std::mem::take(requests[first_index].view.timings());
        timings.add_completed(TERMINAL_PRODUCTION_STAGE, 0, AllocationDelta::default());
        let started = std::time::Instant::now();
        let applications: Vec<_> = group
            .iter()
            .map(|&request_index| {
                let (build, plan_facts) = match &requests[request_index].view {
                    TerminalProductionView::Selection(selection) => (
                        &*selection.build,
                        &selection.checked.selected_provider_plan_facts,
                    ),
                    TerminalProductionView::Installed(checked) => (
                        checked.build(),
                        &checked.execution().selected_provider_plan_facts,
                    ),
                };
                TerminalApplicationCustody {
                    entry_identity: entries[request_index]
                        .as_ref()
                        .ok()
                        .map(|entry| entry.signature_identity),
                    entry_machine: entries[request_index].as_ref().ok().map(|entry| {
                        terminal_production::TerminalMachineSelection::Symbol(
                            entry.selected_machine,
                        )
                    }),
                    behavior_exclusions: terminal_production::TerminalBehaviorExclusions::new(
                        build.config.behavior_exclusions.as_slice(),
                        plan_facts,
                        entries[request_index]
                            .as_ref()
                            .map(|entry| entry.target)
                            .unwrap_or_else(|_| target::TargetProfile::host()),
                    ),
                }
            })
            .collect();
        // Groups contain only admitted entries. Keep a diagnostic if internal
        // request assembly ever violates that property, never guess a root.
        let produced = match &entries[first_index] {
            Ok(entry) => {
                let first = &requests[first_index];
                let (program_typed, checked_program) = match &first.view {
                    TerminalProductionView::Selection(selection) => {
                        (&selection.checked.program.typed, &selection.checked.program)
                    }
                    TerminalProductionView::Installed(checked) => (
                        &checked.execution().program.typed,
                        &checked.execution().program,
                    ),
                };
                let context = provider_planning::ProviderPlanningContext::new(
                    program_typed,
                    source::DependencyScope::Product,
                );
                provider_planning::target_machines::provider_selection_roots(
                    context,
                    first
                        .view
                        .provider_defaults()
                        .iter()
                        .flat_map(|row| &row.selections)
                        .chain(&first.view.build().config.provider_selections),
                )
                .and_then(|boundary_roots| {
                    let mut machines = first.view.build().config.program_roots();
                    if machines.is_empty() {
                        machines.push(entry.selected_machine);
                    }
                    // Normalize the default to its portable family head; exact
                    // application receipts still name the selected declaration.
                    let typed = program_typed;
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
                    terminal_production::TerminalProductionRequest {
                        checked: checked_program,
                        exact_toolchain_sources: first.view.sources().exact_toolchain_sources(),
                        machine: terminal_production::TerminalMachineSelection::Symbol(default),
                        roots: terminal_production::TerminalProgramRoots {
                            machines: &machines,
                            boundaries: &boundary_roots,
                        },
                        optimization_selections: first
                            .selections
                            .project_psi()
                            .selections()
                            .clone(),
                    }
                    .produce(TerminalProductionCustody {
                        applications: &applications,
                        timings: &mut timings,
                    })
                    .map_err(|error| error.diagnostics())
                })
            }
            Err(diagnostics) => Err(diagnostics.clone()),
        };
        drop(applications);
        let elapsed = started.elapsed().as_micros();
        timings.add_completed(
            TERMINAL_PRODUCTION_STAGE,
            elapsed,
            AllocationDelta::default(),
        );
        *requests[first_index].view.timings() = timings;
        for &request_index in &group[1..] {
            // The shared cost is visible in each product report; per-leg rows
            // remain on the producing request, not fabricated executions.
            requests[request_index].view.timings().add_completed(
                TERMINAL_PRODUCTION_STAGE,
                elapsed,
                AllocationDelta::default(),
            );
        }
        match produced {
            Err(diagnostics) => {
                for request_index in group {
                    results[request_index] = Some(Err(diagnostics.clone()));
                }
            }
            Ok(produced) => {
                let (artifact, applications) = produced.into_parts();
                let artifact = Arc::new(artifact);
                for (request_index, application) in group.into_iter().zip(applications) {
                    results[request_index] = Some(application.and_then(|receipt| {
                        let receipt = receipt.ok_or_else(|| {
                            vec![Diagnostic::error(
                                "terminal-artifact production retained no checked ProgramEntry receipt",
                            )]
                        })?;
                        Ok(ProducedTerminalApplication {
                            artifact: Arc::clone(&artifact),
                            receipt,
                        })
                    }));
                }
            }
        }
    }
    results
        .into_iter()
        .map(|result| {
            result.unwrap_or_else(|| {
                Err(vec![Diagnostic::error(
                    "Terminal production lost an application outcome",
                )])
            })
        })
        .collect()
}

/// Product admission is a selection-custody check: an Independent edge has no
/// product carrier yet, and production requires one exact selected entry.
fn admit_terminal_production<'a>(
    selected_program_entry: Option<
        &'a provider_planning::program_entry::SelectedCompilerProgramEntry,
    >,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
) -> Result<&'a provider_planning::program_entry::SelectedCompilerProgramEntry, Vec<Diagnostic>> {
    provider_planning::verify_selected_compositions_are_realized(provenance)?;
    // Preserve the build operand's lexical package choice, including when
    // another package contains a machine with the same display name.
    selected_program_entry.ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production requires one exact selected program entry",
        )]
    })
}

/// Assemble one target's retained terminal product after installation: the
/// shared artifact, its per-application entry receipt projected to the native
/// proposal, and the same target's provider proposals. Proposal construction
/// reads the installed execution — placements, establishments and grants are
/// all installation products.
pub(crate) fn finish_terminal_application(
    checked: &CheckedCompilation,
    produced: ProducedTerminalApplication,
    selections: &OptimizationSelections,
) -> Result<compilation_products::RetainedTerminalArtifact, Vec<Diagnostic>> {
    let proposal = native_proposal::project_terminal_native_realization_proposal(
        checked,
        &produced.artifact,
        produced.receipt,
        selections,
    )?;
    let providers = publish_target_providers(checked, &produced.artifact, selections);
    Ok(
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            produced.artifact,
            proposal.with_target_provider_proposals(providers),
        ),
    )
}

fn publish_target_providers(
    checked: &CheckedCompilation,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    selections: &OptimizationSelections,
) -> Vec<terminal_psi_to_abstract_operations::TerminalTargetProviderProposal> {
    let mut targets = checked
        .provider_defaults()
        .iter()
        .filter_map(|row| row.target.clone())
        .collect::<Vec<_>>();
    for binding in &checked.build().config.root_bindings {
        if let Some((owner, _)) = binding.slot.rsplit_once("::")
            && let Ok(profile) = target::TargetProfile::from_root_slot_owner(owner)
        {
            targets.push(profile.target_name().to_owned());
        }
    }
    targets.push(
        checked
            .execution()
            .selected_target_profile
            .unwrap_or_else(target::TargetProfile::host)
            .target_name()
            .to_owned(),
    );
    targets.sort();
    targets.dedup();
    provider_planning::target_machines::TargetProviderPublicationRequest {
        context: provider_planning::ProviderPlanningContext::new(
            &checked.execution().program.typed,
            source::DependencyScope::Product,
        ),
        artifact: artifact.view(),
        exact_toolchain_sources: checked.sources().exact_toolchain_sources(),
        defaults: checked.provider_defaults(),
        overrides: &checked.build().config.provider_selections,
        settlement: checked.providers(),
        targets: &targets,
        optimization_selection: selections.identity(),
    }
    .publish()
}
