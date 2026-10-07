//! Select the executable work domain without rewriting canonical Terminal.
//!
//! Retention is not execution demand: Psi keeps every authored target body,
//! while one native invocation needs the entry's transitive calls, ordered
//! cleanup hooks and selected checked adapters. The same scope constrains
//! function lowering, provider installation and external call occurrences;
//! trimming functions after installation would orphan its admitted call rows.
//! Selection here supplies no provider authority: ordinary provider admission
//! still admits the exact selected candidate and its occurrence contracts.
//!
//! Image emission currently materializes the complete dynamic table catalog,
//! so every present table row is an address root, even without an invocation.
//! Those rows cannot be pruned until the image owner supports a selected table
//! pool. Callback thunks are independent canonical artifacts, not machine IDs
//! inside this program; their registrar calls remain ordinary boundary edges.

use crate::SelectedProviderAdapter;
use abstract_operations::{AbstractCallTarget, AbstractOperation, AbstractOperationPlan};
use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug)]
pub enum NativeExecutionScopeError {
    MissingMachine(MachineId),
    MissingSelectedCheckedProvider(BoundaryMachineId),
    CompetingSelectedCheckedProviders(BoundaryMachineId),
}

impl std::fmt::Display for NativeExecutionScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingMachine(machine) => write!(
                formatter,
                "native execution scope reaches absent machine {machine:?}"
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
    // Exact rows in this input's unchanged candidate catalog, not membership
    // inferred from a body that might also be reachable by another route.
    selected_candidate_positions: Vec<usize>,
}

impl NativeExecutionScope {
    pub(crate) fn derive(
        plan: &AbstractOperationPlan,
        selected: &[SelectedProviderAdapter],
    ) -> Result<Self, NativeExecutionScopeError> {
        let mut function_positions = plan
            .functions
            .iter()
            .enumerate()
            .map(|(position, function)| (function.machine, position))
            .collect::<Vec<_>>();
        function_positions.sort_unstable_by_key(|(machine, _)| *machine);
        let mut visited = vec![false; plan.functions.len()];
        let mut pending = vec![plan.entry];
        pending.extend(
            plan.dynamic_dispatch
                .tables
                .iter()
                .flat_map(|table| table.rows.iter().flatten())
                .copied(),
        );
        let mut scope = Self {
            machines: Vec::new(),
            boundaries: Vec::new(),
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
            for operation in &plan.functions[function_position].operations {
                for (_, cleanup) in operation.cleanup_rosters() {
                    for action in cleanup {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                            action
                        {
                            pending.push(cleanup.cleanup_machine);
                        }
                    }
                }
                match operation {
                    AbstractOperation::Call { callee, .. } => pending.push(*callee),
                    AbstractOperation::CallStructural {
                        callee: AbstractCallTarget::Machine(callee),
                        ..
                    } => pending.push(*callee),
                    // Every table row is already an address root. A dynamic
                    // call cannot reach a body beyond that complete catalog.
                    AbstractOperation::CallStructural {
                        callee: AbstractCallTarget::Dynamic { .. },
                        ..
                    } => {}
                    AbstractOperation::BoundaryCall { boundary, .. }
                    | AbstractOperation::ExternalCompletion { boundary, .. } => {
                        if scope.boundaries.contains(boundary) {
                            continue;
                        }
                        scope.boundaries.push(*boundary);
                        let candidates = plan
                            .provider_candidates
                            .iter()
                            .enumerate()
                            .filter(|(_, candidate)| candidate.boundary == *boundary)
                            .collect::<Vec<_>>();
                        if !selected.iter().any(|row| {
                            candidates.iter().any(|(_, candidate)| {
                                row.requirement_identity == candidate.requirement_identity
                            })
                        }) {
                            // A retained checked alternative does not choose
                            // this invocation's binding. Imports and builtins
                            // remain external leaves with their own admission.
                            continue;
                        }
                        let mut matches = candidates.iter().filter(|(_, candidate)| {
                            // One portable overload identity covers every
                            // target body. Match only an actual callable in
                            // this deployment's projection, without aliasing
                            // its ID or renumbering the canonical catalog.
                            function_positions
                                .binary_search_by_key(&candidate.candidate, |(machine, _)| *machine)
                                .is_ok()
                                && selected.iter().any(|row| {
                                    row.requirement_identity == candidate.requirement_identity
                                        && row.provider_identity == candidate.provider_identity
                                        && row.machine_identity == candidate.candidate_identity
                                })
                        });
                        let candidate = matches.next().ok_or(
                            NativeExecutionScopeError::MissingSelectedCheckedProvider(*boundary),
                        )?;
                        if matches.next().is_some() {
                            return Err(
                                NativeExecutionScopeError::CompetingSelectedCheckedProviders(
                                    *boundary,
                                ),
                            );
                        }
                        scope.selected_candidate_positions.push(candidate.0);
                        pending.push(candidate.1.candidate);
                    }
                    _ => {}
                }
            }
        }
        scope.machines.sort_unstable();
        scope.boundaries.sort_unstable();
        scope.selected_candidate_positions.sort_unstable();
        Ok(scope)
    }

    pub(crate) fn contains_machine(&self, machine: MachineId) -> bool {
        self.machines.binary_search(&machine).is_ok()
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
