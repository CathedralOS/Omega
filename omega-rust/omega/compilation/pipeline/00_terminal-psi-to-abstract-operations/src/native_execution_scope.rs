//! Select the executable work domain without rewriting canonical Terminal.
//!
//! Retention is not execution demand: Psi keeps every authored target body,
//! while one native invocation needs the entry's transitive calls, ordered
//! cleanup hooks and selected checked adapters. The same scope constrains
//! function lowering, provider installation and external call occurrences;
//! trimming functions after installation would orphan its admitted call rows.
//! Selection here supplies no provider authority: ordinary provider admission
//! still admits the exact selected candidate and its occurrence contracts.
//! Receiving field obligations are roots too: forwarding across a callable's
//! region telescope does not erase the entry's distinct closed application.
//!
//! Image emission currently materializes the complete dynamic table catalog,
//! so every present table row is an address root, even without an invocation.
//! Those rows cannot be pruned until the image owner supports a selected table
//! pool. Retained callback bodies share the canonical namespace; a reached
//! registrar occurrence makes its exact callback entry an executable root too.

use crate::SelectedProviderAdapter;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{OperationKind, TerminalModule, Terminator};
mod storage;

#[derive(Debug)]
pub(crate) enum NativeExecutionScopeError {
    TargetFamily(String),
    MissingMachine(MachineId),
    MissingBoundary(BoundaryMachineId),
    MissingSelectedCheckedProvider(BoundaryMachineId),
    CompetingSelectedCheckedProviders(BoundaryMachineId),
}

impl std::fmt::Display for NativeExecutionScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetFamily(error) => formatter.write_str(error),
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

pub(crate) struct NativeExecutionScope {
    machines: Vec<MachineId>,
    boundaries: Vec<BoundaryMachineId>,
    boundaries_set: std::collections::BTreeSet<BoundaryMachineId>,
    // Exact rows in this input's unchanged candidate catalog, not membership
    // inferred from a body that might also be reachable by another route.
    selected_candidate_positions: Vec<usize>,
}

struct ScopeBoundaryIndex<'a> {
    boundary_set: std::collections::BTreeSet<BoundaryMachineId>,
    candidates_by_boundary: std::collections::BTreeMap<
        BoundaryMachineId,
        Vec<(usize, &'a terminal_psi::ProviderCandidateConformance)>,
    >,
    selected_by_req: std::collections::BTreeMap<&'a str, Vec<&'a SelectedProviderAdapter>>,
}

impl NativeExecutionScope {
    pub(crate) fn derive(
        module: &TerminalModule,
        entry: MachineId,
        targets: &terminal_installation::TerminalTargetContext,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        selected: &[SelectedProviderAdapter],
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        callbacks: &[backend_plan::TerminalCallbackOccurrenceProposal],
    ) -> Result<Self, NativeExecutionScopeError> {
        let mut function_positions = module
            .machines
            .iter()
            .enumerate()
            .filter(|(_, machine)| {
                module.target_machine_families.iter().all(|family| {
                    let callable = terminal_psi::TerminalCallable::Machine(machine.id);
                    !family
                        .members
                        .iter()
                        .any(|member| member.callable == callable)
                        || targets
                            .member(family.canonical)
                            .is_ok_and(|selected| selected.callable() == callable)
                })
            })
            .map(|(position, machine)| (machine.id, position))
            .collect::<Vec<_>>();
        function_positions.sort_unstable_by_key(|(machine, _)| *machine);
        let boundary_set = module
            .boundary_machines
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut candidates_by_boundary = std::collections::BTreeMap::<
            BoundaryMachineId,
            Vec<(usize, &terminal_psi::ProviderCandidateConformance)>,
        >::new();
        for (pos, candidate) in module.provider_candidates.iter().enumerate() {
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
        let index = ScopeBoundaryIndex {
            boundary_set,
            candidates_by_boundary,
            selected_by_req,
        };
        let mut visited = vec![false; module.machines.len()];
        let mut pending = vec![entry];
        pending.extend(
            module
                .dynamic_dispatch
                .tables
                .iter()
                .flat_map(|table| table.rows.iter().flatten())
                .copied(),
        );
        let mut scope = Self {
            machines: Vec::new(),
            boundaries: Vec::new(),
            boundaries_set: std::collections::BTreeSet::new(),
            selected_candidate_positions: Vec::new(),
        };
        while let Some(machine) = pending.pop() {
            let position = function_positions
                .binary_search_by_key(&machine, |(identity, _)| *identity)
                .map_err(|_| NativeExecutionScopeError::MissingMachine(machine))?;
            let function_position = function_positions[position].1;
            if std::mem::replace(&mut visited[function_position], true) {
                continue;
            }
            scope.machines.push(machine);
            let body = &module.machines[function_position];
            // Preserve native's all-block work domain, including no-normal calls.
            // This is not the interpreter's dynamic execution reachability walk.
            for block in &body.blocks {
                for operation in &block.operations {
                    if let OperationKind::BoundaryCall { .. } = operation.kind {
                        pending.extend(
                            callbacks
                                .iter()
                                .filter(|callback| callback.terminal_operation() == operation.id)
                                .map(|callback| callback.callback_thunk_artifact().entry()),
                        );
                    }
                    if let Some(invocation) = operation.kind.module_invocation() {
                        pending.push(invocation.callee);
                    }
                    match &operation.kind {
                        // Metadata demands executable realization without invoking it.
                        OperationKind::EstablishNativeCodeLayout { machine } => {
                            pending.push(*machine)
                        }
                        OperationKind::BoundaryCall { boundary, .. } => {
                            scope.retain_boundary(
                                module,
                                targets,
                                entry_call,
                                &index,
                                assumptions,
                                &function_positions,
                                *boundary,
                                &mut pending,
                            )?;
                        }
                        // Dynamic table rows are already address roots.
                        _ => {}
                    }
                }
                for (_, cleanup) in block.terminator.cleanup_rosters() {
                    for action in cleanup {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
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
                    scope.retain_boundary(
                        module,
                        targets,
                        entry_call,
                        &index,
                        assumptions,
                        &function_positions,
                        boundary,
                        &mut pending,
                    )?;
                }
            }
        }
        scope.machines.sort_unstable();
        scope.boundaries.sort_unstable();
        scope.selected_candidate_positions.sort_unstable();
        Ok(scope)
    }

    /// Calls and receiving obligations share one exact selected-candidate join.
    /// This selects physical work; provider installation still owns admission.
    fn retain_boundary(
        &mut self,
        module: &TerminalModule,
        targets: &terminal_installation::TerminalTargetContext,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        index: &ScopeBoundaryIndex<'_>,
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        function_positions: &[(MachineId, usize)],
        boundary: BoundaryMachineId,
        pending: &mut Vec<MachineId>,
    ) -> Result<(), NativeExecutionScopeError> {
        if self.boundaries_set.contains(&boundary) {
            return Ok(());
        }
        if !index.boundary_set.contains(&boundary) {
            return Err(NativeExecutionScopeError::MissingBoundary(boundary));
        }
        self.boundaries_set.insert(boundary);
        self.boundaries.push(boundary);
        if let Some(entry) = entry_call.filter(|entry| entry.boundary() == boundary) {
            // The selected continuation's complete ordinary call/provider and
            // cleanup closure participates before native demand and stack work.
            pending.push(entry.entry());
            return Ok(());
        }
        if let Some((_, supplied)) = assumptions
            .iter()
            .find(|(required, _)| *required == boundary)
        {
            return self.retain_boundary(
                module,
                targets,
                entry_call,
                index,
                assumptions,
                function_positions,
                *supplied,
                pending,
            );
        }
        if let Some(terminal_psi::TerminalCallable::Machine(member)) =
            crate::optimization::target_family_member(module, Some(targets), boundary)
                .map_err(NativeExecutionScopeError::TargetFamily)?
        {
            pending.push(member);
            return Ok(());
        }
        let Some(candidates) = index.candidates_by_boundary.get(&boundary) else {
            return Ok(());
        };
        if !candidates.iter().any(|(_, candidate)| {
            module
                .identities
                .text(candidate.requirement_identity)
                .is_some_and(|req| index.selected_by_req.contains_key(req))
        }) {
            // Retained alternatives do not select an invocation's binding.
            // Imports and builtins remain independently admitted external leaves.
            return Ok(());
        }
        let mut matches = candidates.iter().filter(|(_, candidate)| {
            // Keep actual callable IDs in the selected target projection.
            function_positions
                .binary_search_by_key(&candidate.candidate, |(machine, _)| *machine)
                .is_ok()
                && module
                    .identities
                    .text(candidate.requirement_identity)
                    .and_then(|req| index.selected_by_req.get(req))
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

    pub(crate) fn contains_machine(&self, machine: MachineId) -> bool {
        self.machines.binary_search(&machine).is_ok()
    }

    /// Values read by any selected native block, plus condition-bearing
    /// evaluation entries invoked by that scope. Native selection includes
    /// address roots and callbacks; interpreter reachability cannot replace it.
    /// Storage geometry may impose additional extent demand independently.
    pub(crate) fn scalar_value_demand(
        &self,
        module: &TerminalModule,
        targets: &terminal_installation::TerminalTargetContext,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut demanded = std::collections::BTreeSet::new();
        for machine in module
            .machines
            .iter()
            .filter(|machine| self.contains_machine(machine.id))
        {
            for operation in machine.operations() {
                if let OperationKind::ReadScalarApplication { application } = operation.kind {
                    demanded.insert(application);
                }
            }
        }
        let catalog = &module.scalar_applications;
        for (position, declaration) in catalog.declarations().iter().enumerate() {
            let terminal_psi::ScalarApplication::Evaluate { entry, .. } = declaration else {
                continue;
            };
            let machine = match entry {
                terminal_psi::TerminalCallable::Machine(machine) => *machine,
                terminal_psi::TerminalCallable::Boundary(_) => {
                    let Ok(member) = targets.member(*entry) else {
                        continue;
                    };
                    let terminal_psi::TerminalCallable::Machine(machine) = member.callable()
                    else {
                        continue;
                    };
                    machine
                }
            };
            if !self.contains_machine(machine) {
                continue;
            }
            let application = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(semantic_vocabulary::ScalarApplicationId::new)
                .ok_or("native scalar application coordinate exceeds its catalog")?;
            if catalog.conditions.contains_subject(
                terminal_psi::ScalarConditionSubject::Application(application),
            ) {
                demanded.insert(application);
            }
        }
        Ok(demanded.into_iter().collect())
    }

    pub(crate) fn scalar_application_demand(
        &self,
        module: &TerminalModule,
        targets: &terminal_installation::TerminalTargetContext,
        layouts: &terminal_installation::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut applications = self.storage_extent_demand(module, layouts)?;
        applications.extend(self.scalar_value_demand(module, targets)?);
        Ok(applications.into_iter().collect())
    }

    pub(crate) fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.boundaries.binary_search(&boundary).is_ok()
    }

    pub(crate) fn contains_candidate_position(&self, position: usize) -> bool {
        self.selected_candidate_positions
            .binary_search(&position)
            .is_ok()
    }
}
