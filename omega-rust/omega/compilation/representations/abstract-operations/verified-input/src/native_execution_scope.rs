/*
  Keep every function that the selected entry, providers and callbacks can run.
  The checked proposal supplies their complete inputs; this owner follows
  machine, boundary, cleanup and interrupt edges without changing the module.
  Later stages retain that same selection with the lowered program.
*/
use provider_admission::provider_installation::SelectedProviderAdapter;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{OperationKind, TerminalModule, Terminator};
mod storage {
    use super::NativeExecutionScope;
    use semantic_vocabulary::ScalarApplicationId;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::TerminalModule;

    impl NativeExecutionScope {
        pub(super) fn storage_extent_demand(
            &self,
            module: &TerminalModule,
            layouts: &terminal_installation::AdmittedTargetLayouts,
        ) -> Result<BTreeSet<ScalarApplicationId>, String> {
            let mut declarations = module
                .structural_types
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect::<BTreeMap<_, _>>();
            for resolved in layouts.resolved_records() {
                declarations.insert(resolved.id, resolved);
            }
            let mut roots = BTreeSet::new();
            for machine in module
                .machines
                .iter()
                .filter(|machine| self.contains_machine(machine.id))
            {
                roots.extend(
                    machine
                        .structural_parameters
                        .iter()
                        .chain(machine.block_structural_parameters())
                        .map(|parameter| parameter.structural_type),
                );
                roots.extend(
                    machine
                        .result
                        .structural()
                        .map(|result| result.structural_type),
                );
                for place in &machine.structural_places {
                    roots.extend(place.kind.produced_structural_type());
                }
                for block in &machine.blocks {
                    for operation in &block.operations {
                        roots.extend(
                            operation
                                .result
                                .structural()
                                .map(|result| result.structural_type),
                        );
                        for storage in operation.kind.storage_uses() {
                            terminal_semantics::storage_operand_dependencies(
                                machine,
                                &declarations,
                                storage,
                                &mut roots,
                            )
                            .map_err(|reason| {
                                format!("native storage demand for {:?}: {reason}", operation.id)
                            })?;
                        }
                    }
                    for projection in block.terminator.storage_uses() {
                        terminal_semantics::storage_operand_dependencies(
                            machine,
                            &declarations,
                            projection.into(),
                            &mut roots,
                        )
                        .map_err(|reason| {
                            format!(
                                "native terminator storage demand for {:?}: {reason}",
                                machine.id
                            )
                        })?;
                    }
                }
            }
            for boundary in module
                .boundary_machines
                .iter()
                .filter(|boundary| self.contains_boundary(boundary.id))
            {
                roots.extend(
                    boundary
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.structural_type),
                );
                if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
                    roots.insert(result.structural_type);
                }
            }
            calling_conventions::terminal_layout::StructuralLayouts::new(declarations.into_values())
                .map_err(|error| format!("native storage declarations: {error:?}"))?
                .required_scalar_extents(roots)
                .map_err(|error| format!("native storage extent demand: {error:?}"))
        }
    }
}

#[derive(Debug)]
pub enum NativeExecutionScopeError {
    TargetFamily(String),
    InstalledInterrupt(String),
    MissingMachine(MachineId),
    MissingBoundary(BoundaryMachineId),
    MissingSelectedCheckedProvider(BoundaryMachineId),
    CompetingSelectedCheckedProviders(BoundaryMachineId),
}

impl std::fmt::Display for NativeExecutionScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetFamily(error) | Self::InstalledInterrupt(error) => {
                formatter.write_str(error)
            }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionScope {
    target_selections: terminal_installation::AdmittedTargetMachineSelections,
    entry: MachineId,
    selected_provider_closure: [u8; 32],
    source_provider_selection: effects::SelectedProviderPlanFacts,
    source_checked_entry: Option<terminal_psi::CheckedProgramEntryTerminalReceipt>,
    local_adapters: Vec<SelectedProviderAdapter>,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    callbacks: Vec<backend_plan::TerminalCallbackOccurrenceProposal>,
    machines: Vec<MachineId>,
    boundaries: Vec<BoundaryMachineId>,
    boundaries_set: std::collections::BTreeSet<BoundaryMachineId>,
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
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        selected: (
            &[SelectedProviderAdapter],
            &effects::SelectedProviderPlanFacts,
        ),
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        callbacks: &[backend_plan::TerminalCallbackOccurrenceProposal],
    ) -> Result<Self, NativeExecutionScopeError> {
        let module = artifact.module();
        if targets.terminal_psi() != artifact.identity() {
            return Err(NativeExecutionScopeError::TargetFamily(
                "native execution scope target selections name another Terminal artifact".into(),
            ));
        }
        let (selected, selected_plans) = selected;
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
        };
        let installed_helpers =
            provider_selection::CallbackProviderDeclarations::installed_helper_roots(module);
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
            target_selections: targets.clone(),
            entry,
            selected_provider_closure: *selected_plans.identity_digest().as_bytes(),
            source_provider_selection: selected_plans.clone(),
            source_checked_entry: None,
            local_adapters: selected.to_vec(),
            entry_call: entry_call.cloned(),
            callbacks: callbacks.to_vec(),
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
            for block in &body.blocks {
                for operation in &block.operations {
                    if let OperationKind::BoundaryCall { .. } = operation.kind {
                        if let Ok(position) = installed_helpers
                            .binary_search_by_key(&operation.id, |(operation, _)| *operation)
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

    fn retain_boundary(
        &mut self,
        module: &TerminalModule,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
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
        if let Some(terminal_psi::TargetMachineCallable::Machine(member)) =
            terminal_installation::admitted_boundary_member(module, Some(targets), boundary)
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
            return Ok(());
        }
        let mut matches = candidates.iter().filter(|(_, candidate)| {
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

    pub fn require_source_input(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        selection: &effects::SelectedProviderPlanFacts,
        adapters: &[SelectedProviderAdapter],
    ) -> Result<(), &'static str> {
        if artifact.identity() != self.target_selections.terminal_psi()
            || entry != self.entry
            || targets != &self.target_selections
            || selection != &self.source_provider_selection
            || adapters != self.local_adapters
        {
            return Err("native preparation differs from its checked proposal inputs");
        }
        Ok(())
    }

    pub fn callbacks(&self) -> &[backend_plan::TerminalCallbackOccurrenceProposal] {
        &self.callbacks
    }

    pub fn full_module_scalar_demand(
        module: &TerminalModule,
        entry: MachineId,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        layouts: &terminal_installation::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut machines = module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>();
        machines.sort_unstable();
        let mut boundaries = module
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id)
            .collect::<Vec<_>>();
        boundaries.sort_unstable();
        let full = Self {
            target_selections: targets.clone(),
            entry,
            selected_provider_closure: [0; 32],
            source_provider_selection: effects::SelectedProviderPlanFacts::default(),
            source_checked_entry: None,
            local_adapters: Vec::new(),
            entry_call: None,
            callbacks: Vec::new(),
            machines,
            boundaries_set: boundaries.iter().copied().collect(),
            boundaries,
            selected_candidate_positions: (0..module.provider_candidates.len()).collect(),
        };
        full.scalar_application_demand(module, targets, layouts)
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn target_selections(&self) -> &terminal_installation::AdmittedTargetMachineSelections {
        &self.target_selections
    }

    pub fn source_provider_selection(&self) -> &effects::SelectedProviderPlanFacts {
        &self.source_provider_selection
    }

    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        &self.local_adapters
    }
    pub fn entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        self.entry_call.as_ref()
    }
    pub const fn selected_provider_closure(&self) -> [u8; 32] {
        self.selected_provider_closure
    }

    pub(crate) fn bind_checked_source_entry(
        &mut self,
        entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
    ) {
        self.source_checked_entry = Some(entry.clone());
    }

    pub(crate) fn require_proposal_context(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        proposal: &crate::TerminalNativeRealizationProposal,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        self.require_source_selection(&proposal.providers().selection)?;
        if self.target_selections.terminal_psi() != artifact.identity()
            || self.target_selections.target() != proposal.target_profile().target_name()
            || self.source_checked_entry.as_ref() != Some(proposal.program_entry().checked_entry())
            || self.callbacks != proposal.occurrences().callbacks
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "native execution belongs to another checked proposal context",
            )]);
        }
        Ok(())
    }

    pub(crate) fn require_source_selection(
        &self,
        selection: &effects::SelectedProviderPlanFacts,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if &self.source_provider_selection != selection {
            return Err(vec![diagnostics::Diagnostic::error(
                "native preparation belongs to another authored provider selection",
            )]);
        }
        Ok(())
    }

    pub(crate) fn bind_completed_provider_selection(
        &mut self,
        selection: &effects::SelectedProviderPlanFacts,
    ) {
        self.selected_provider_closure = *selection.identity_digest().as_bytes();
    }

    pub(crate) fn require_input_context(
        &self,
        plan: &abstract_operations::AbstractOperationPlan,
        targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if self.target_selections.terminal_psi() != plan.psi
            || self.entry != plan.entry
            || targets != Some(&self.target_selections)
            || self.selected_provider_closure != selected_provider_closure
        {
            return Err(
                "native execution scope belongs to another artifact, entry, target or provider selection",
            );
        }
        Ok(())
    }

    pub fn machines(&self) -> &[MachineId] {
        &self.machines
    }

    pub fn contains_machine(&self, machine: MachineId) -> bool {
        self.machines.binary_search(&machine).is_ok()
    }

    pub fn boundary_application_demands(
        &self,
        module: &TerminalModule,
    ) -> Result<Vec<terminal_psi::TerminalBoundaryApplicationDemand>, &'static str> {
        if module
            .boundary_application_demands
            .windows(2)
            .any(|rows| rows[0].terminal_operation().get() >= rows[1].terminal_operation().get())
        {
            return Err("Terminal boundary demand catalog is not strictly ordered");
        }
        let mut owners = module
            .machines
            .iter()
            .flat_map(|machine| machine.operations().map(move |op| (op.id, machine.id)))
            .collect::<Vec<_>>();
        owners.sort_unstable_by_key(|(operation, _)| operation.get());
        let mut demands = Vec::new();
        for demand in &module.boundary_application_demands {
            let operation = demand.terminal_operation();
            let position = owners.partition_point(|(found, _)| found.get() < operation.get());
            let Some(&(found, machine)) = owners.get(position) else {
                return Err("Terminal boundary demand has no operation owner");
            };
            if found != operation
                || owners
                    .get(position + 1)
                    .is_some_and(|(next, _)| *next == operation)
            {
                return Err("Terminal boundary demand has no unique operation owner");
            }
            if self.contains_machine(machine) {
                demands.push(demand.clone());
            }
        }
        Ok(demands)
    }

    pub fn scalar_value_demand(
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

    pub fn scalar_application_demand(
        &self,
        module: &TerminalModule,
        targets: &terminal_installation::AdmittedTargetMachineSelections,
        layouts: &terminal_installation::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut applications = self.storage_extent_demand(module, layouts)?;
        applications.extend(self.scalar_value_demand(module, targets)?);
        Ok(applications.into_iter().collect())
    }

    pub fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.boundaries.binary_search(&boundary).is_ok()
    }

    pub fn contains_candidate_position(&self, position: usize) -> bool {
        self.selected_candidate_positions
            .binary_search(&position)
            .is_ok()
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    };
    fn machine_id(value: u64) -> MachineId {
        MachineId::new(value).unwrap()
    }

    fn scalar_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    fn machine(identity: u64, kind: OperationKind) -> TerminalMachine {
        let result = ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: scalar_type(),
            qualifications: Default::default(),
        };
        TerminalMachine {
            id: machine_id(identity),
            attachment: None,
            nominal_cleanup_subject: None,
            structural_places: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: vec![Operation::unsuspended(
                    OperationId::new(1).unwrap(),
                    OperationResult::Scalar(result),
                    kind,
                )],
                terminator: Terminator::Return {
                    edge: EdgeId::new(1).unwrap(),
                    value: result.id,
                    cleanup_actions: Vec::new(),
                },
            }],
            ranked_scc: None,
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            contract: terminal_psi::MachineContract {
                declaration: None,
                parameters: Vec::new(),
                parameter_order: Vec::new(),
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                content_entry_claims: Vec::new(),
                result: TerminalMachineResult::Scalar(result),
                declared_service_reach: None,
                closed_reach_application: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
                ..MachineContract::empty(ContractId::new(identity).unwrap())
            },
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        }
    }

    fn call(callee: u64) -> OperationKind {
        OperationKind::Call {
            callee: machine_id(callee),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    }

    fn graph() -> TerminalModule {
        let mut module = TerminalModule::for_entry(machine_id(1));
        let application = module
            .scalar_applications
            .insert(terminal_psi::ScalarApplication::Evaluate {
                entry: terminal_psi::TargetMachineCallable::Machine(machine_id(1)),
                scalar_type: scalar_type(),
            })
            .unwrap();
        module.machines = vec![
            machine(
                1,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(7),
                },
            ),
            machine(2, call(3)),
            machine(3, OperationKind::ReadScalarApplication { application }),
        ];
        module
    }

    fn scope(
        module: TerminalModule,
        target: &str,
    ) -> Result<NativeExecutionScope, NativeExecutionScopeError> {
        let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
        let artifact =
            terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
        let reloaded =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
        let targets =
            terminal_installation::admit_target_machine_selections(reloaded.view(), target);
        let entry = targets
            .entry(
                reloaded.view().identity(),
                reloaded.semantic_module(),
                reloaded.semantic_module().entry.unwrap(),
            )
            .map_err(|reason| NativeExecutionScopeError::TargetFamily(reason.to_string()))?;
        NativeExecutionScope::derive(
            reloaded.view(),
            entry,
            &targets,
            None,
            (&[], &effects::SelectedProviderPlanFacts::default()),
            &[],
            &[],
        )
    }

    #[test]
    fn selected_scope_omits_dormant_scalar_work_and_retains_calls_from_all_blocks() {
        assert_eq!(
            scope(graph(), "macos_arm64").unwrap().machines(),
            &[machine_id(1)]
        );
        let mut called = graph();
        called.machines[0].blocks[0].operations[0].kind = call(2);
        assert_eq!(
            scope(called, "macos_arm64").unwrap().machines(),
            &[machine_id(1), machine_id(2), machine_id(3)]
        );
        let mut blocks = graph();
        let mut block = blocks.machines[0].blocks[0].clone();
        block.id = BlockId::new(2).unwrap();
        block.operations[0].id = OperationId::new(2).unwrap();
        block.operations[0].result = OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(2).unwrap(),
            scalar_type: scalar_type(),
            qualifications: Default::default(),
        });
        block.operations[0].kind = call(2);
        block.terminator = Terminator::Return {
            edge: EdgeId::new(2).unwrap(),
            value: ValueId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        };
        blocks.machines[0].blocks.push(block);
        assert_eq!(
            scope(blocks, "macos_arm64").unwrap().machines(),
            &[machine_id(1), machine_id(2), machine_id(3)]
        );
    }

    #[test]
    fn selected_scope_preserves_exact_target_member_choice() {
        let mut module = graph();
        let callable = |value| terminal_psi::TargetMachineCallable::Machine(machine_id(value));
        module
            .target_machine_families
            .push(terminal_psi::TargetMachineFamily {
                canonical: callable(1),
                head_contract: [1; 32],
                members: vec![
                    terminal_psi::TargetMachineFamilyMember {
                        target: "macos_arm64".into(),
                        callable: callable(1),
                        supply: terminal_psi::TargetMachineSupply::CheckedBody,
                    },
                    terminal_psi::TargetMachineFamilyMember {
                        target: "windows_x86_64".into(),
                        callable: callable(2),
                        supply: terminal_psi::TargetMachineSupply::CheckedBody,
                    },
                ],
            });
        assert_eq!(
            scope(module.clone(), "macos_arm64").unwrap().machines(),
            &[machine_id(1)]
        );
        module.entry = Some(callable(2));
        assert!(scope(module.clone(), "macos_arm64").is_err());
        assert_eq!(
            scope(module, "windows_x86_64").unwrap().machines(),
            &[machine_id(2), machine_id(3)]
        );
    }
}
