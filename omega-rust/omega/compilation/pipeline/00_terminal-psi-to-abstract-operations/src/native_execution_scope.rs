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
    LenderPreservationUnavailable {
        boundary: BoundaryMachineId,
        reason: &'static str,
    },
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
            Self::LenderPreservationUnavailable { boundary, reason } => write!(
                formatter,
                "native provider for {boundary:?} cannot preserve its captured lender: {reason}"
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
    assumptions: BoundarySupplies<BoundaryMachineId>,
}

impl NativeExecutionScope {
    pub(crate) fn derive(
        module: &TerminalModule,
        entry: MachineId,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
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
                    let callable = terminal_psi::TargetMachineCallable::Machine(machine.id);
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
            assumptions: assumed_boundary_supplies(assumptions),
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
        let mut boundary_supplies = Vec::new();
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
                                &mut boundary_supplies,
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
                        &mut boundary_supplies,
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
        if !scope.boundaries.is_empty() {
            let supplies = BoundarySupplies::new(boundary_supplies);
            let preservation =
                terminal_semantics::descriptor_results::BoundaryLenderPreservation::new(module);
            for &boundary in &scope.boundaries {
                // Keep the original cycle refusal, even when an explicit entry
                // supplies the call before its assumption would be followed.
                if matches!(
                    index.assumptions.supply(boundary),
                    Some(BoundarySupply::Alias(_))
                ) {
                    index
                        .assumptions
                        .resolution(boundary)
                        .map_err(|reason| boundary_resolution_error(boundary, reason))?;
                }
                // Check the body chosen by the reachability walk. Re-selecting
                // here could check a different helper from the one that runs.
                let implementation = supplies
                    .resolution(boundary)
                    .map_err(|reason| boundary_resolution_error(boundary, reason))?;
                preservation
                    .require(boundary, implementation)
                    .map_err(
                        |reason| NativeExecutionScopeError::LenderPreservationUnavailable {
                            boundary,
                            reason,
                        },
                    )?;
            }
        }
        Ok(scope)
    }

    /// Calls and receiving obligations share one exact selected-candidate join.
    /// This selects physical work; provider installation still owns admission.
    fn retain_boundary(
        &mut self,
        module: &TerminalModule,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        index: &ScopeBoundaryIndex<'_>,
        supplies: &mut Vec<(BoundaryMachineId, BoundarySupply<Option<MachineId>>)>,
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
            supplies.push((boundary, BoundarySupply::Final(Some(entry.entry()))));
            return Ok(());
        }
        if let Some(BoundarySupply::Alias(supplied)) = index.assumptions.supply(boundary) {
            supplies.push((boundary, BoundarySupply::Alias(supplied)));
            return self.retain_boundary(
                module,
                targets,
                entry_call,
                index,
                supplies,
                function_positions,
                supplied,
                pending,
            );
        }
        if let Some(terminal_psi::TargetMachineCallable::Machine(member)) =
            crate::optimization::target_family_member(module, Some(targets), boundary)
                .map_err(NativeExecutionScopeError::TargetFamily)?
        {
            pending.push(member);
            supplies.push((boundary, BoundarySupply::Final(Some(member))));
            return Ok(());
        }
        let Some(candidates) = index.candidates_by_boundary.get(&boundary) else {
            supplies.push((boundary, BoundarySupply::Final(None)));
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
            supplies.push((boundary, BoundarySupply::Final(None)));
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
        supplies.push((boundary, BoundarySupply::Final(Some(candidate.1.candidate))));
        pending.push(candidate.1.candidate);
        Ok(())
    }

    pub(crate) fn machines(&self) -> &[MachineId] {
        &self.machines
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
        targets: &terminal_installation::AdmittedTargetMachineSelections,
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
                terminal_psi::TargetMachineCallable::Machine(machine) => *machine,
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    let Ok(member) = targets.member(*entry) else {
                        continue;
                    };
                    let terminal_psi::TargetMachineCallable::Machine(machine) = member.callable()
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
        targets: &terminal_installation::AdmittedTargetMachineSelections,
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

#[derive(Clone, Copy)]
enum BoundarySupply<T> {
    Alias(BoundaryMachineId),
    Final(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundaryResolutionError {
    Cycle,
    Missing(BoundaryMachineId),
}

/// Resolve a chosen supply once, then share it with every caller that uses it.
/// Sparse boundary identities index sorted rows, never an ID-sized allocation.
struct BoundarySupplies<T> {
    rows: Vec<(BoundaryMachineId, BoundarySupply<T>)>,
    resolved: Vec<Result<T, BoundaryResolutionError>>,
}

impl<T: Copy> BoundarySupplies<T> {
    fn new(mut rows: Vec<(BoundaryMachineId, BoundarySupply<T>)>) -> Self {
        // The original slice lookup used its first matching row.
        rows.sort_by_key(|(boundary, _)| *boundary);
        rows.dedup_by_key(|(boundary, _)| *boundary);
        let mut resolved = vec![None; rows.len()];
        let mut active = vec![false; rows.len()];
        let mut path = Vec::new();
        for start in 0..rows.len() {
            if resolved[start].is_some() {
                continue;
            }
            let mut position = start;
            let result = loop {
                if let Some(result) = resolved[position] {
                    break result;
                }
                if active[position] {
                    break Err(BoundaryResolutionError::Cycle);
                }
                active[position] = true;
                path.push(position);
                match rows[position].1 {
                    BoundarySupply::Final(body) => break Ok(body),
                    BoundarySupply::Alias(boundary) => {
                        position = match rows.binary_search_by_key(&boundary, |(id, _)| *id) {
                            Ok(position) => position,
                            Err(_) => break Err(BoundaryResolutionError::Missing(boundary)),
                        };
                    }
                }
            };
            for position in path.drain(..) {
                resolved[position] = Some(result);
                active[position] = false;
            }
        }
        let resolved = rows
            .iter()
            .zip(resolved)
            .map(|((boundary, _), result)| {
                result.unwrap_or(Err(BoundaryResolutionError::Missing(*boundary)))
            })
            .collect();
        Self { rows, resolved }
    }

    fn supply(&self, boundary: BoundaryMachineId) -> Option<BoundarySupply<T>> {
        self.rows
            .binary_search_by_key(&boundary, |(id, _)| *id)
            .ok()
            .map(|position| self.rows[position].1)
    }

    fn resolution(&self, boundary: BoundaryMachineId) -> Result<T, BoundaryResolutionError> {
        let position = self
            .rows
            .binary_search_by_key(&boundary, |(id, _)| *id)
            .map_err(|_| BoundaryResolutionError::Missing(boundary))?;
        self.resolved[position]
    }
}

fn assumed_boundary_supplies(
    assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
) -> BoundarySupplies<BoundaryMachineId> {
    let rows = assumptions
        .iter()
        .map(|&(required, supplied)| (required, BoundarySupply::Alias(supplied)))
        // An endpoint with no further assumption denotes itself. Required rows
        // come first so a repeated ID retains the original slice's first match.
        .chain(
            assumptions
                .iter()
                .map(|&(_, supplied)| (supplied, BoundarySupply::Final(supplied))),
        )
        .collect();
    BoundarySupplies::new(rows)
}

fn boundary_resolution_error(
    boundary: BoundaryMachineId,
    reason: BoundaryResolutionError,
) -> NativeExecutionScopeError {
    match reason {
        BoundaryResolutionError::Missing(missing) => {
            NativeExecutionScopeError::MissingBoundary(missing)
        }
        BoundaryResolutionError::Cycle => {
            NativeExecutionScopeError::LenderPreservationUnavailable {
                boundary,
                reason: "the assumed provider selection is cyclic",
            }
        }
    }
}

#[cfg(test)]
mod boundary_resolution_tests {
    use super::*;

    fn boundary(id: u64) -> BoundaryMachineId {
        BoundaryMachineId::new(id).expect("nonzero boundary")
    }

    #[test]
    fn assumed_sources_keep_the_first_row_and_sparse_chain_endpoint() {
        let first = boundary(1);
        let middle = boundary(u64::MAX - 1);
        let last = boundary(u64::MAX);
        let supplies =
            assumed_boundary_supplies(&[(first, middle), (first, first), (middle, last)]);
        assert_eq!(supplies.resolution(first), Ok(last));
        assert_eq!(supplies.resolution(middle), Ok(last));
        assert_eq!(supplies.resolution(last), Ok(last));
        assert!(matches!(supplies.supply(first), Some(BoundarySupply::Alias(id)) if id == middle));
    }

    #[test]
    fn missing_and_cyclic_supplies_cannot_become_checked_bodies() {
        let body = MachineId::new(7).expect("nonzero machine");
        let supplies = BoundarySupplies::new(vec![
            (boundary(1), BoundarySupply::Alias(boundary(2))),
            (boundary(2), BoundarySupply::Alias(boundary(1))),
            (boundary(3), BoundarySupply::Alias(boundary(u64::MAX))),
            (boundary(4), BoundarySupply::Final(None)),
            (boundary(5), BoundarySupply::Final(Some(body))),
            (boundary(6), BoundarySupply::Alias(boundary(5))),
        ]);
        assert_eq!(
            supplies.resolution(boundary(1)),
            Err(BoundaryResolutionError::Cycle)
        );
        assert_eq!(
            supplies.resolution(boundary(2)),
            Err(BoundaryResolutionError::Cycle)
        );
        assert_eq!(
            supplies.resolution(boundary(3)),
            Err(BoundaryResolutionError::Missing(boundary(u64::MAX)))
        );
        assert_eq!(supplies.resolution(boundary(4)), Ok(None));
        assert_eq!(supplies.resolution(boundary(5)), Ok(Some(body)));
        assert_eq!(supplies.resolution(boundary(6)), Ok(Some(body)));
        assert_eq!(
            supplies.resolution(boundary(9)),
            Err(BoundaryResolutionError::Missing(boundary(9)))
        );
    }
}
