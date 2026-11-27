//! Follow the machines, callbacks and provider boundaries reachable for a selected target entry.
#![forbid(unsafe_code)]

mod assembly_returns;

use native_execution_reach::{ExecutionReach, ReachedCallback};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use target_bound_program::{NativeExecutionScope, NativeScopeBinding, SelectedProviderAdapter};
use terminal_psi::{OperationKind, TerminalModule, Terminator};
#[cfg(test)]
use terminal_semantics::closed_owned_sequence_bounds;

/// Follow the selected entry and provider edges in the checked program.
pub fn derive_native_scope(
    binding: NativeScopeBinding<'_>,
) -> Result<NativeExecutionScope, Vec<diagnostics::Diagnostic>> {
    let reach = TargetReach::derive(ScopeInputs {
        profile: binding.profile(),
        artifact: binding.artifact(),
        entry: binding.entry(),
        entry_call: binding
            .entry_call()
            .map(|call| (call.boundary(), call.entry())),
        adapters: binding.local_adapters(),
        callbacks: binding.callbacks(),
    })
    .map_err(|reason| vec![diagnostics::Diagnostic::error(reason.to_string())])?;
    binding
        .bind_target_reach(reach.emitted, reach.selected_candidate_positions, reach.assembly_returns)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])
}

/// Sources of native calls for one checked proposal and entry.
struct ScopeInputs<'a> {
    profile: target::TargetProfile,
    artifact: terminal_codec::TerminalArtifactView<'a>,
    entry: MachineId,
    entry_call: Option<(BoundaryMachineId, MachineId)>,
    adapters: &'a [SelectedProviderAdapter],
    callbacks: &'a [provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal],
}

struct TargetReach {
    assembly_returns: Vec<target_bound_program::AssemblyCallingPlan>,
    emitted: ExecutionReach,
    selected_candidate_positions: Vec<usize>,
}

#[derive(Debug)]
pub enum NativeExecutionScopeError {
    TargetContext(String),
    InstalledInterrupt(String),
    MissingMachine(MachineId),
    InvalidMachineDescriptor {
        descriptor: semantic_vocabulary::MachineDescriptorId,
        reason: &'static str,
    },
    MissingBoundary(BoundaryMachineId),
    MissingSelectedCheckedProvider(BoundaryMachineId),
    CompetingSelectedCheckedProviders(BoundaryMachineId),
}

impl std::fmt::Display for NativeExecutionScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetContext(error) | Self::InstalledInterrupt(error) => {
                formatter.write_str(error)
            }
            Self::InvalidMachineDescriptor { descriptor, reason } => write!(
                formatter,
                "native machine selection {descriptor:?}: {reason}"
            ),
            Self::MissingMachine(machine) => write!(
                formatter,
                "native execution scope reaches absent machine {machine:?}"
            ),
            Self::MissingBoundary(boundary) => write!(
                formatter,
                "native execution scope reaches absent boundary {boundary:?}"
            ),
            Self::MissingSelectedCheckedProvider(boundary) => write!(
                formatter,
                "native execution scope has no selected checked provider for {boundary:?}"
            ),
            Self::CompetingSelectedCheckedProviders(boundary) => write!(
                formatter,
                "native execution scope has competing selected checked providers for {boundary:?}"
            ),
        }
    }
}

impl std::error::Error for NativeExecutionScopeError {}

struct ScopeBoundaryIndex<'a> {
    reached: std::collections::BTreeSet<BoundaryMachineId>,
    selected_candidate_positions: Vec<usize>,
    boundary_set: std::collections::BTreeSet<BoundaryMachineId>,
    candidates_by_boundary: std::collections::BTreeMap<
        BoundaryMachineId,
        Vec<(usize, &'a terminal_psi::ProviderCandidateConformance)>,
    >,
    selected_by_req: std::collections::BTreeMap<&'a str, Vec<&'a SelectedProviderAdapter>>,
}

impl TargetReach {
    fn derive(input: ScopeInputs<'_>) -> Result<Self, NativeExecutionScopeError> {
        let ScopeInputs {
            profile,
            artifact,
            entry,
            entry_call,
            adapters: selected,
            callbacks,
        } = input;
        let target = profile.target_name();
        let module = artifact.module();
        let mut function_positions = module
            .machines
            .iter()
            .enumerate()
            .map(|(position, machine)| (machine.id, position))
            .collect::<Vec<_>>();
        function_positions.sort_unstable_by_key(|(machine, _)| *machine);
        let boundary_set = module
            .interfaces.boundary_machines
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut candidates_by_boundary = std::collections::BTreeMap::<
            BoundaryMachineId,
            Vec<(usize, &terminal_psi::ProviderCandidateConformance)>,
        >::new();
        for (pos, candidate) in module.interfaces.provider_candidates.iter().enumerate() {
            candidates_by_boundary
                .entry(candidate.boundary)
                .or_default()
                .push((pos, candidate));
        }
        let mut selected_by_req =
            std::collections::BTreeMap::<&str, Vec<&SelectedProviderAdapter>>::new();
        for row in selected {
            selected_by_req
                .entry(row.requirement_identity.as_str())
                .or_default()
                .push(row);
        }
        let mut index = ScopeBoundaryIndex {
            reached: Default::default(),
            selected_candidate_positions: Vec::new(),
            boundary_set,
            candidates_by_boundary,
            selected_by_req,
        };
        let installed_helpers =
            provider_correspondence::CallbackProviderDeclarations::installed_helper_roots(module);
        // Resolve each occurrence once. A registrar's local operation number
        // identifies its callback entries only together with the owning machine.
        let mut callback_entries = std::collections::BTreeMap::<_, Vec<usize>>::new();
        let mut reached_callbacks = vec![false; callbacks.len()];
        for (position, callback) in callbacks.iter().enumerate() {
            let application = module
                .callback_application(callback.application_index())
                .ok_or_else(|| {
                    NativeExecutionScopeError::TargetContext(
                        "callback occurrence lost its canonical application".into(),
                    )
                })?;
            if application.registrar != callback.terminal_operation() {
                return Err(NativeExecutionScopeError::TargetContext(
                    "callback occurrence changed its canonical registrar operation".into(),
                ));
            }
            callback_entries
                .entry((application.registrar_machine, application.registrar))
                .or_default()
                .push(position);
        }
        let mut visited = vec![false; module.machines.len()];
        let mut pending = vec![entry];
        pending.extend(
            module
                .execution.dynamic_dispatch
                .tables
                .iter()
                .flat_map(|table| table.rows.iter().flatten())
                .copied(),
        );
        let mut machines = Vec::new();
        let mut assembly_returns = Vec::new();
        while let Some(machine) = pending.pop() {
            let position = function_positions
                .binary_search_by_key(&machine, |(identity, _)| *identity)
                .map_err(|_| NativeExecutionScopeError::MissingMachine(machine))?;
            let function_position = function_positions[position].1;
            if std::mem::replace(&mut visited[function_position], true) {
                continue;
            }
            machines.push(machine);
            let body = &module.machines[function_position];
            let requirement = body.assembly_target_requirement;
            if !requirement.accepts_target_name(target) {
                let declared = body
                    .declaration
                    .as_ref()
                    .and_then(|row| module.identities.text(row.machine.identity()))
                    .map_or_else(|| format!("{machine:?}"), |name| format!("`{name}`"));
                let architecture = requirement.architecture_name();
                return Err(NativeExecutionScopeError::TargetContext(format!(
                    "reachable machine {declared} contains checked assembly that requires {architecture}; the selected target is `{target}`"
                )));
            }
            if let Some(plan) = assembly_returns::bind(profile, module, body)
                .map_err(|reason| NativeExecutionScopeError::TargetContext(format!(
                    "assembly return in machine {machine:?}: {reason}")))? {
                assembly_returns.push(plan);
            }
            for block in &body.blocks {
                for operation in &block.operations {
                    if let OperationKind::BoundaryCall { .. } = operation.kind {
                        if let Ok(position) = installed_helpers
                            .binary_search_by_key(&(body.id, operation.id), |(operation, _)| {
                                *operation
                            })
                        {
                            pending.extend(
                                installed_helpers[position]
                                    .1
                                    .as_ref()
                                    .map_err(|reason| {
                                        NativeExecutionScopeError::InstalledInterrupt(
                                            reason.clone(),
                                        )
                                    })?
                                    .iter()
                                    .copied(),
                            );
                        }
                        if let Some(positions) = callback_entries.get(&(body.id, operation.id)) {
                            for &position in positions {
                                reached_callbacks[position] = true;
                                pending.push(callbacks[position].callback_thunk_artifact().entry());
                            }
                        }
                    }
                    if let Some(invocation) = operation.kind.module_invocation() {
                        pending.push(invocation.callee);
                    }
                    match &operation.kind {
                        OperationKind::SelectMachine { descriptor } => {
                            let invalid =
                                |reason| NativeExecutionScopeError::InvalidMachineDescriptor {
                                    descriptor: *descriptor,
                                    reason,
                                };
                            let selected = module
                                .execution.callables
                                .descriptor(*descriptor)
                                .ok_or_else(|| invalid("selected descriptor is absent"))?;
                            if operation.result.scalar().map(|value| value.scalar_type)
                                != Some(semantic_vocabulary::ScalarType::Callable(
                                    selected.contract,
                                ))
                                || module.execution.callables.contract(selected.contract).is_none()
                            {
                                return Err(invalid(
                                    "selected descriptor changes its callable contract",
                                ));
                            }
                            pending.push(selected.machine);
                        }
                        OperationKind::EstablishNativeCodeLayout { machine } => {
                            pending.push(*machine)
                        }
                        OperationKind::BoundaryCall { boundary, .. } => {
                            index.retain_boundary(
                                module,
                                entry_call,
                                &function_positions,
                                *boundary,
                                &mut pending,
                            )?;
                        }
                        _ => {}
                    }
                }
                for (_, cleanup) in block.terminator.cleanup_rosters() {
                    for action in cleanup {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) =
                            action
                        {
                            pending.push(cleanup.cleanup_machine);
                        }
                    }
                }
                if let Some(invocation) = block.terminator.module_invocation() {
                    pending.push(invocation.callee);
                }
                if let Terminator::ExternalCompletion { boundary, .. } = block.terminator {
                    index.retain_boundary(
                        module,
                        entry_call,
                        &function_positions,
                        boundary,
                        &mut pending,
                    )?;
                }
            }
        }
        let emitted = ExecutionReach::new(
            machines,
            index.reached.into_iter().collect(),
            callbacks
                .iter()
                .zip(reached_callbacks)
                .filter_map(|(callback, reached)| {
                    reached.then_some(ReachedCallback {
                        application_index: callback.application_index(),
                        terminal_operation: callback.terminal_operation(),
                    })
                })
                .collect(),
        );
        Ok(Self {
            assembly_returns,
            emitted,
            selected_candidate_positions: index.selected_candidate_positions,
        })
    }
}

impl ScopeBoundaryIndex<'_> {
    fn retain_boundary(
        &mut self,
        module: &TerminalModule,
        entry_call: Option<(BoundaryMachineId, MachineId)>,
        function_positions: &[(MachineId, usize)],
        boundary: BoundaryMachineId,
        pending: &mut Vec<MachineId>,
    ) -> Result<(), NativeExecutionScopeError> {
        if self.reached.contains(&boundary) {
            return Ok(());
        }
        if !self.boundary_set.contains(&boundary) {
            return Err(NativeExecutionScopeError::MissingBoundary(boundary));
        }
        self.reached.insert(boundary);
        if let Some(entry) = entry_call.filter(|(required, _)| *required == boundary) {
            pending.push(entry.1);
            return Ok(());
        }
        let Some(candidates) = self.candidates_by_boundary.get(&boundary) else {
            return Ok(());
        };
        if !candidates.iter().any(|(_, candidate)| {
            module
                .identities
                .text(candidate.requirement_identity)
                .is_some_and(|req| self.selected_by_req.contains_key(req))
        }) {
            return Ok(());
        }
        let mut matches = candidates.iter().filter(|(_, candidate)| {
            function_positions
                .binary_search_by_key(&candidate.candidate, |(machine, _)| *machine)
                .is_ok()
                && module
                    .identities
                    .text(candidate.requirement_identity)
                    .and_then(|req| self.selected_by_req.get(req))
                    .is_some_and(|rows| {
                        rows.iter().any(|row| {
                            module.identities.text(candidate.provider_identity)
                                == Some(row.provider_identity.as_str())
                                && module
                                    .identities
                                    .text(candidate.provider_declaration.identity())
                                    == Some(row.provider_declaration.as_str())
                                && module.identities.text(candidate.candidate_identity)
                                    == Some(row.machine_identity.as_str())
                        })
                    })
        });
        let candidate =
            matches
                .next()
                .ok_or(NativeExecutionScopeError::MissingSelectedCheckedProvider(
                    boundary,
                ))?;
        if matches.next().is_some() {
            return Err(NativeExecutionScopeError::CompetingSelectedCheckedProviders(boundary));
        }
        self.selected_candidate_positions.push(candidate.0);
        pending.push(candidate.1.candidate);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
