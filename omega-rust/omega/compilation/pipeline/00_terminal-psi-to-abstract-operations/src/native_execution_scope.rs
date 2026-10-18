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
use abstract_operations::{AbstractCallTarget, AbstractOperation};
use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug)]
pub enum NativeExecutionScopeError {
    TerminalIdentityMismatch,
    TargetFamily(String),
    TargetLayout(String),
    MissingMachine(MachineId),
    MissingBoundary(BoundaryMachineId),
    MissingSelectedCheckedProvider(BoundaryMachineId),
    CompetingSelectedCheckedProviders(BoundaryMachineId),
}

impl std::fmt::Display for NativeExecutionScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TerminalIdentityMismatch => write!(
                formatter,
                "native deployment selection names a different Terminal artifact"
            ),
            Self::TargetFamily(error) | Self::TargetLayout(error) => formatter.write_str(error),
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
    // Exact rows in this input's unchanged candidate catalog, not membership
    // inferred from a body that might also be reachable by another route.
    selected_candidate_positions: Vec<usize>,
}

impl NativeExecutionScope {
    pub(crate) fn derive(
        input: &crate::VerifiedPsiOptimizationInput,
        selected: &[SelectedProviderAdapter],
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        receiving_provider_roots: &[BoundaryMachineId],
        callbacks: &[backend_plan::TerminalCallbackOccurrenceProposal],
    ) -> Result<Self, NativeExecutionScopeError> {
        let plan = input.plan();
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
        for boundary in receiving_provider_roots {
            scope.retain_boundary(
                input,
                selected,
                assumptions,
                &function_positions,
                *boundary,
                &mut pending,
            )?;
        }
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
                if let AbstractOperation::BoundaryCall { psi_operation, .. } = operation {
                    pending.extend(
                        callbacks
                            .iter()
                            .filter(|callback| callback.terminal_operation() == *psi_operation)
                            .map(|callback| callback.callback_thunk_artifact().entry()),
                    );
                }
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
                        scope.retain_boundary(
                            input,
                            selected,
                            assumptions,
                            &function_positions,
                            *boundary,
                            &mut pending,
                        )?;
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

    /// Calls and receiving obligations share one exact selected-candidate join.
    /// This selects physical work; provider installation still owns admission.
    fn retain_boundary(
        &mut self,
        input: &crate::VerifiedPsiOptimizationInput,
        selected: &[SelectedProviderAdapter],
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        function_positions: &[(MachineId, usize)],
        boundary: BoundaryMachineId,
        pending: &mut Vec<MachineId>,
    ) -> Result<(), NativeExecutionScopeError> {
        if self.boundaries.contains(&boundary) {
            return Ok(());
        }
        let plan = input.plan();
        if !plan.boundary_machines.iter().any(|row| row.id == boundary) {
            return Err(NativeExecutionScopeError::MissingBoundary(boundary));
        }
        self.boundaries.push(boundary);
        if let Some((_, supplied)) = assumptions
            .iter()
            .find(|(required, _)| *required == boundary)
        {
            return self.retain_boundary(
                input,
                selected,
                assumptions,
                function_positions,
                *supplied,
                pending,
            );
        }
        if let Some(terminal_psi::TargetMachineCallable::Machine(member)) = input
            .target_family_member(boundary)
            .map_err(NativeExecutionScopeError::TargetFamily)?
        {
            pending.push(member);
            return Ok(());
        }
        let candidates = plan
            .provider_candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.boundary == boundary)
            .collect::<Vec<_>>();
        if !selected.iter().any(|row| {
            candidates.iter().any(|(_, candidate)| {
                plan.identities.text(candidate.requirement_identity)
                    == Some(row.requirement_identity.as_str())
            })
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
                && selected.iter().any(|row| {
                    plan.identities.text(candidate.requirement_identity)
                        == Some(row.requirement_identity.as_str())
                        && plan.identities.text(candidate.provider_identity)
                            == Some(row.provider_identity.as_str())
                        && plan
                            .identities
                            .text(candidate.provider_declaration.identity())
                            == Some(row.provider_declaration.as_str())
                        && plan.identities.text(candidate.candidate_identity)
                            == Some(row.machine_identity.as_str())
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

    pub(crate) fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.boundaries.binary_search(&boundary).is_ok()
    }

    pub(crate) fn contains_candidate_position(&self, position: usize) -> bool {
        self.selected_candidate_positions
            .binary_search(&position)
            .is_ok()
    }
}
