//! Produce each checked entry closure once, with independent target admission.
//!
//! Terminal has one entry today. Targets selecting the same source closure,
//! entry and Psi optimization share its artifact; different roots remain
//! distinct products. Target signatures, exclusion judgments and native
//! proposals never participate in portable production identity.

use crate::checked::CheckedCompilation;
use crate::terminal::native_proposal;
use artifacts::allocations::AllocationDelta;
use artifacts::compile_timings::{StageMeta, TimingCategory};
use compilation_products::RetainedTerminalArtifact;
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
const NATIVE_REALIZATION_PROPOSAL_STAGE: StageMeta = StageMeta::new(
    "native-realization-proposal",
    "CanonicalTerminalArtifact",
    "NativeRealizationProposal",
    TimingCategory::Pipeline,
);

pub(crate) struct TerminalProductRequest<'a> {
    pub checked: &'a mut CheckedCompilation,
    pub selections: &'a OptimizationSelections,
}

struct TerminalEntry {
    portable_machine: symbols::SymbolHandle,
    selected_machine: symbols::SymbolHandle,
    signature_identity: [u8; 32],
    target: target::TargetProfile,
}

/// Invocation-local sharing, not a cache: source custody and checked arenas
/// must be the same owners, not merely equal-looking programs. The small
/// target roster is grouped by direct scans; no source hashing or copying.
pub(crate) fn produce_retained_terminal_artifacts(
    mut requests: Vec<TerminalProductRequest<'_>>,
) -> Vec<Result<RetainedTerminalArtifact, Vec<Diagnostic>>> {
    let entries: Vec<_> = requests
        .iter()
        .map(|request| {
            request.checked.admit_terminal_production().map(|entry| {
                let selected_machine = entry.machine_symbol();
                let typed = &request.checked.execution().program.typed;
                let portable_machine = typed
                    .target_machine_families
                    .iter()
                    .find(|family| {
                        (family.canonical == selected_machine
                            || family
                                .members
                                .iter()
                                .any(|member| member.machine == selected_machine))
                            && typed
                                .machine_by_symbol(family.canonical)
                                .is_some_and(|machine| machine.target_family_interface)
                    })
                    .map_or(selected_machine, |family| family.canonical);
                TerminalEntry {
                    portable_machine,
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
        let Ok(entry) = entry else {
            continue;
        };
        let request = &requests[request_index];
        let group = groups.iter_mut().find(|group| {
            let first_index = group[0];
            let first = &requests[first_index];
            entries[first_index]
                .as_ref()
                .is_ok_and(|first_entry| first_entry.portable_machine == entry.portable_machine)
                && Arc::ptr_eq(
                    &first.checked.execution().program,
                    &request.checked.execution().program,
                )
                && std::ptr::eq(first.checked.sources(), request.checked.sources())
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
        let mut timings = std::mem::take(requests[first_index].checked.timings_mut());
        timings.add_completed(TERMINAL_PRODUCTION_STAGE, 0, AllocationDelta::default());
        let started = std::time::Instant::now();
        let applications: Vec<_> = group
            .iter()
            .map(|&request_index| {
                let checked = &requests[request_index].checked;
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
                        checked.build().config.behavior_exclusions.as_slice(),
                        &checked.execution().selected_provider_plan_facts,
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
                let context = provider_planning::ProviderPlanningContext::new(
                    &first.checked.execution().program.typed,
                    source::DependencyScope::Product,
                );
                provider_planning::target_machines::provider_selection_roots(
                    context,
                    first
                        .checked
                        .provider_defaults()
                        .iter()
                        .flat_map(|row| &row.selections)
                        .chain(&first.checked.build().config.provider_selections),
                )
                .and_then(|boundary_roots| {
                    terminal_production::TerminalProductionRequest {
                        checked: &first.checked.execution().program,
                        exact_toolchain_sources: first.checked.sources().exact_toolchain_sources(),
                        machine: terminal_production::TerminalMachineSelection::Symbol(
                            entry.portable_machine,
                        ),
                        boundary_roots: &boundary_roots,
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
        *requests[first_index].checked.timings_mut() = timings;
        for &request_index in &group[1..] {
            // The shared cost is visible in each product report; per-leg rows
            // remain on the producing request, not fabricated executions.
            requests[request_index].checked.timings_mut().add_completed(
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
                    let request = &mut requests[request_index];
                    let mut timings = std::mem::take(request.checked.timings_mut());
                    let result = application.and_then(|receipt| {
                        let receipt = receipt.ok_or_else(|| vec![Diagnostic::error(
                            "terminal-artifact production retained no checked ProgramEntry receipt",
                        )])?;
                        timings
                            .record_result(NATIVE_REALIZATION_PROPOSAL_STAGE, || {
                                native_proposal::project_terminal_native_realization_proposal(
                                    request.checked,
                                    &artifact,
                                    receipt,
                                    request.selections,
                                )
                            })
                            .map(|proposal| {
                                let target_providers = publish_target_providers(
                                    request.checked,
                                    &artifact,
                                    request.selections,
                                );
                                RetainedTerminalArtifact::new_with_native_realization_proposal(
                                    Arc::clone(&artifact),
                                    proposal.with_target_provider_proposals(target_providers),
                                )
                            })
                    });
                    *request.checked.timings_mut() = timings;
                    results[request_index] = Some(result);
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
