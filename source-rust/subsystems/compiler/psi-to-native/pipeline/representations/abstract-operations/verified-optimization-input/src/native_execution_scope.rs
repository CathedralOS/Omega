/*
  Keep every function that the selected entry, providers and callbacks can run.
  The checked proposal supplies their complete inputs; this owner follows
  machine, boundary, cleanup and interrupt edges without changing the module.
  Later stages retain that same selection with the lowered program.
*/
mod sequence_bounds;

use provider_admission::provider_installation::SelectedProviderAdapter;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use sequence_bounds::closed_owned_sequence_bounds;
use target_vocabulary::publication::{EmittedCallbackOccurrence, EmittedExecutionScope};
use terminal_psi::{OperationKind, TerminalModule, Terminator};
struct ScalarDemand<'scope> {
    emitted: Option<&'scope EmittedExecutionScope>,
    owned_sequence_bounds: &'scope [(semantic_vocabulary::StructuralTypeId, u64)],
}

impl ScalarDemand<'_> {
    fn contains_machine(&self, machine: MachineId) -> bool {
        self.emitted
            .is_none_or(|scope| scope.contains_machine(machine))
    }

    fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.emitted
            .is_none_or(|scope| scope.contains_boundary(boundary))
    }

    fn scalar_value_demand(
        &self,
        module: &TerminalModule,
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
            let terminal_psi::TargetMachineCallable::Machine(machine) = entry else {
                continue;
            };
            let machine = *machine;
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

    fn scalar_application_demand(
        &self,
        module: &TerminalModule,
        layouts: &psi_target_layouts::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut applications = self.storage_extent_demand(module, layouts)?;
        applications.extend(self.scalar_value_demand(module)?);
        Ok(applications.into_iter().collect())
    }
}

mod storage {
    use super::ScalarDemand;
    use semantic_vocabulary::ScalarApplicationId;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::TerminalModule;

    impl ScalarDemand<'_> {
        pub(super) fn storage_extent_demand(
            &self,
            module: &TerminalModule,
            layouts: &psi_target_layouts::AdmittedTargetLayouts,
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
                        for (storage, _) in operation.kind.storage_uses() {
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
            calling_conventions::terminal_layout::StructuralLayouts::with_owned_sequence_bounds(
                declarations.into_values(),
                self.owned_sequence_bounds,
            )
            .map_err(|error| format!("native storage declarations: {error:?}"))?
            .required_scalar_extents(roots)
            .map_err(|error| format!("native storage extent demand: {error:?}"))
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionScope {
    target_context: psi_target_layouts::TerminalTargetContext,
    entry: MachineId,
    selected_provider_closure: [u8; 32],
    source_native_bindings: Option<[u8; 32]>,
    source_provider_selection: effects::SelectedProviderPlanFacts,
    source_checked_entry: Option<terminal_psi::CheckedProgramEntryTerminalReceipt>,
    local_adapters: Vec<SelectedProviderAdapter>,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    callbacks: Vec<provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal>,
    demands: boundary_applications::TerminalBoundaryApplicationDemands,
    pub(super) emitted: std::sync::Arc<EmittedExecutionScope>,
    owned_sequence_bounds: Vec<(semantic_vocabulary::StructuralTypeId, u64)>,
    selected_candidate_positions: Vec<usize>,
}

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

impl NativeExecutionScope {
    pub(crate) fn derive(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        targets: &psi_target_layouts::TerminalTargetContext,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        selected: (
            &[SelectedProviderAdapter],
            &effects::SelectedProviderPlanFacts,
        ),
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
        callbacks: &[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal],
    ) -> Result<Self, NativeExecutionScopeError> {
        let module = artifact.module();
        if targets.terminal_psi() != artifact.identity() {
            return Err(NativeExecutionScopeError::TargetContext(
                "native execution scope target selections name another Terminal artifact".into(),
            ));
        }
        let (selected, selected_plans) = selected;
        let mut function_positions = module
            .machines
            .iter()
            .enumerate()
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
                .dynamic_dispatch
                .tables
                .iter()
                .flat_map(|table| table.rows.iter().flatten())
                .copied(),
        );
        let mut machines = Vec::new();
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
                                .callables
                                .descriptor(*descriptor)
                                .ok_or_else(|| invalid("selected descriptor is absent"))?;
                            if operation.result.scalar().map(|value| value.scalar_type)
                                != Some(semantic_vocabulary::ScalarType::Callable(
                                    selected.contract,
                                ))
                                || module.callables.contract(selected.contract).is_none()
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
                        assumptions,
                        &function_positions,
                        boundary,
                        &mut pending,
                    )?;
                }
            }
        }
        let emitted = std::sync::Arc::new(EmittedExecutionScope::new(
            machines,
            index.reached.into_iter().collect(),
            callbacks
                .iter()
                .zip(reached_callbacks)
                .filter_map(|(callback, reached)| {
                    reached.then_some(EmittedCallbackOccurrence {
                        application_index: callback.application_index(),
                        terminal_operation: callback.terminal_operation(),
                    })
                })
                .collect(),
        ));
        index.selected_candidate_positions.sort_unstable();
        let owned_sequence_bounds =
            closed_owned_sequence_bounds(module, |machine| emitted.contains_machine(machine))
                .map_err(|reason| NativeExecutionScopeError::TargetContext(reason.into()))?;
        let demands = module
            .boundary_application_demands_for_machines(emitted.machines())
            .and_then(|rows| {
                boundary_applications::TerminalBoundaryApplicationDemands::new(
                    artifact.identity(),
                    rows,
                )
            })
            .map_err(|reason| NativeExecutionScopeError::TargetContext(reason.into()))?;
        Ok(Self {
            demands,
            target_context: targets.clone(),
            entry,
            selected_provider_closure: *selected_plans.identity_digest().as_bytes(),
            source_provider_selection: selected_plans.clone(),
            source_native_bindings: None,
            source_checked_entry: None,
            local_adapters: selected.to_vec(),
            entry_call: entry_call.cloned(),
            callbacks: callbacks.to_vec(),
            emitted,
            owned_sequence_bounds,
            selected_candidate_positions: index.selected_candidate_positions,
        })
    }
}

impl ScopeBoundaryIndex<'_> {
    fn retain_boundary(
        &mut self,
        module: &TerminalModule,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        assumptions: &[(BoundaryMachineId, BoundaryMachineId)],
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
                entry_call,
                assumptions,
                function_positions,
                *supplied,
                pending,
            );
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

impl NativeExecutionScope {
    pub fn require_source_input(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        targets: &psi_target_layouts::TerminalTargetContext,
        selection: &effects::SelectedProviderPlanFacts,
        adapters: &[SelectedProviderAdapter],
    ) -> Result<(), &'static str> {
        if artifact.identity() != self.target_context.terminal_psi()
            || entry != self.entry
            || targets != &self.target_context
            || selection != &self.source_provider_selection
            || adapters != self.local_adapters
        {
            return Err("native preparation differs from its checked proposal inputs");
        }
        Ok(())
    }

    pub fn callbacks(
        &self,
    ) -> &[provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal] {
        &self.callbacks
    }

    /// Derive bounds from all machine bodies when lowering the complete input.
    pub fn full_module_owned_sequence_bounds(
        module: &TerminalModule,
    ) -> Result<Vec<(semantic_vocabulary::StructuralTypeId, u64)>, &'static str> {
        closed_owned_sequence_bounds(module, |_| true)
    }

    pub fn full_module_scalar_demand(
        module: &TerminalModule,
        layouts: &psi_target_layouts::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let bounds = Self::full_module_owned_sequence_bounds(module).map_err(str::to_owned)?;
        ScalarDemand {
            emitted: None,
            owned_sequence_bounds: &bounds,
        }
        .scalar_application_demand(module, layouts)
    }

    pub fn owned_sequence_bounds(&self) -> &[(semantic_vocabulary::StructuralTypeId, u64)] {
        &self.owned_sequence_bounds
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn target_context(&self) -> &psi_target_layouts::TerminalTargetContext {
        &self.target_context
    }

    pub(crate) fn source_native_bindings(&self) -> Option<[u8; 32]> {
        self.source_native_bindings
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

    pub(crate) fn bind_native_supply(
        &mut self,
        native: &provider_admission::RetainedNativeBindings,
    ) {
        self.source_native_bindings = Some(native.identity_digest());
    }

    pub(crate) fn require_proposal_context(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        proposal: &crate::TerminalNativeRealizationProposal,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        self.require_source_selection(&proposal.providers().selection)?;
        if self.target_context.terminal_psi() != artifact.identity()
            || self.target_context.target() != proposal.target_profile().target_name()
            || self.source_checked_entry.as_ref() != Some(proposal.program_entry().checked_entry())
            || self.callbacks != proposal.occurrences().callbacks
            || self.source_native_bindings
                != Some(proposal.providers().native_bindings.identity_digest())
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
        targets: Option<&psi_target_layouts::TerminalTargetContext>,
        selected_provider_closure: [u8; 32],
    ) -> Result<(), &'static str> {
        if self.target_context.terminal_psi() != plan.psi
            || self.entry != plan.entry
            || targets != Some(&self.target_context)
            || self.selected_provider_closure != selected_provider_closure
        {
            return Err(
                "native execution scope belongs to another artifact, entry, target or provider selection",
            );
        }
        Ok(())
    }

    pub fn machines(&self) -> &[MachineId] {
        self.emitted.machines()
    }

    pub fn contains_machine(&self, machine: MachineId) -> bool {
        self.emitted.contains_machine(machine)
    }

    pub fn boundary_application_demands(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationDemands {
        &self.demands
    }

    fn scalar_demand(&self) -> ScalarDemand<'_> {
        ScalarDemand {
            emitted: Some(&self.emitted),
            owned_sequence_bounds: &self.owned_sequence_bounds,
        }
    }

    pub fn scalar_value_demand(
        &self,
        module: &TerminalModule,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        self.scalar_demand().scalar_value_demand(module)
    }

    pub fn scalar_application_demand(
        &self,
        module: &TerminalModule,
        layouts: &psi_target_layouts::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        self.scalar_demand()
            .scalar_application_demand(module, layouts)
    }

    pub fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.emitted.contains_boundary(boundary)
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
            structural_places: Vec::new(),
            call_formations: Vec::new(),
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
                selected_application: None,
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
        let targets = psi_target_layouts::terminal_target_context(reloaded.view(), target);
        let entry = targets
            .entry(
                reloaded.view().identity(),
                reloaded.semantic_module(),
                reloaded.semantic_module().entry.unwrap(),
            )
            .map_err(|reason| NativeExecutionScopeError::TargetContext(reason.to_string()))?;
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
    fn sequence_bounds_read_only_selected_constructors() {
        let mut module = TerminalModule::for_entry(machine_id(1));
        module.machines = vec![machine(
            2,
            OperationKind::EstablishStructuralArray { elements: vec![] },
        )];
        // The fixture has a scalar result, so including this machine must reject it.
        assert_eq!(
            closed_owned_sequence_bounds(&module, |id| id == machine_id(1)),
            Ok(vec![])
        );
        assert_eq!(
            closed_owned_sequence_bounds(&module, |_| true),
            Err("sequence construction has no structural result")
        );
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
    fn boundary_demands_require_one_owner_across_reached_and_dormant_machines() {
        let mut module = graph();
        let operation = OperationId::new(77).unwrap();
        let mut identities = module.identities.clone().into_builder();
        let identity = identities.intern("Fixture::effect");
        module.identities = identities.build();
        let demand = terminal_psi::TerminalBoundaryApplicationDemand::new(
            operation,
            terminal_psi::BoundaryOperatorRequirement::new(
                terminal_psi::NominalDeclarationIdentity::new(identity),
                "effect".into(),
                None,
            )
            .unwrap(),
            terminal_psi::BoundaryApplication::Empty,
        );
        module.boundary_application_demands.push(demand.clone());
        let reached = [machine_id(1)];
        assert_eq!(
            module.boundary_application_demands_for_machines(&reached),
            Err("Terminal boundary demand has no operation owner"),
        );
        module.machines[1].blocks[0].operations[0].id = operation;
        assert_eq!(
            module
                .boundary_application_demands_for_machines(&reached)
                .unwrap(),
            [],
        );
        module.machines[0].blocks[0].operations[0].id = operation;
        assert_eq!(
            module.boundary_application_demands_for_machines(&reached),
            Err("Terminal boundary demand has no unique operation owner"),
        );
        module.machines[1].blocks[0].operations[0].id = OperationId::new(2).unwrap();
        assert_eq!(
            module
                .boundary_application_demands_for_machines(&reached)
                .unwrap(),
            [demand],
        );
    }

    #[test]
    fn emission_records_a_callback_only_when_its_registrar_runs() {
        use calling_conventions::{
            CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
        };
        use terminal_psi::{
            CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalCallbackApplication,
        };

        let mut module = graph();
        let mut identities = module.identities.clone().into_builder();
        let name = identities.intern("Fixture::register");
        let callback = identities.intern("Fixture::callback");
        let unit = identities.intern("()");
        module.identities = identities.build();
        let declaration = CheckedMachineDeclaration {
            machine: NominalDeclarationIdentity::new(callback),
            state: NominalDeclarationIdentity::new(callback),
            semantic_declaration: callback,
            parameter_types: Vec::new(),
            callable: callback,
            result_type: unit,
            contract: [9; 32],
            published_may_suspend: false,
            published_may_block: false,
            specialization: None,
        };
        let boundary = BoundaryMachineId::new(1).unwrap();
        module
            .boundary_machines
            .push(terminal_psi::BoundaryMachineDeclaration {
                id: boundary,
                identity: name,
                owner: terminal_psi::NominalDeclarationIdentity::new(name),
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                kind: terminal_psi::BoundaryRequirementKind::Machine,
                completion: terminal_psi::BoundaryCompletion::NormalReturn,
                unconditional_termination: true,
                declaration: None,
                numeric_intrinsic: None,
                attachment: None,
                membership_ensures: Vec::new(),
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                parameter_order: Vec::new(),
                scalar_parameters: Vec::new(),
                crash_routes: Vec::new(),
                structural_parameters: Vec::new(),
                result: terminal_psi::BoundaryMachineResult::Scalar(scalar_type().into()),
                requires: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                result_reference_regions: Vec::new(),
                result_lifetimes: None,
                reference_outputs: Vec::new(),
                fixed_service_reach: Vec::new(),
                service_reach_is_installation_bound: false,
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
            });
        let inbound_boundary = BoundaryMachineId::new(2).unwrap();
        let mut inbound_requirement = module.boundary_machines[0].clone();
        inbound_requirement.id = inbound_boundary;
        inbound_requirement.identity = callback;
        inbound_requirement.result = terminal_psi::BoundaryMachineResult::Unit;
        module.boundary_machines.push(inbound_requirement);
        let registrar = OperationId::new(2).unwrap();
        let demand = terminal_psi::TerminalBoundaryApplicationDemand::new(
            registrar,
            terminal_psi::BoundaryOperatorRequirement::new(
                NominalDeclarationIdentity::new(name),
                "Fixture::register".into(),
                None,
            )
            .unwrap(),
            terminal_psi::BoundaryApplication::Empty,
        );
        module.boundary_application_demands.push(demand.clone());
        module.machines[1].blocks[0].operations[0].id = registrar;
        module.machines[1].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        };
        module.machines[2].blocks[0].operations.clear();
        module.machines[2].blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(3).unwrap(),
            cleanup_actions: Vec::new(),
        };
        module.machines[2].contract.result = TerminalMachineResult::Unit;
        module.machines[2].contract.declaration = Some(declaration.clone());
        module
            .callback_applications
            .push(TerminalCallbackApplication {
                application_index: 0,
                registrar_machine: machine_id(2),
                registrar,
                static_machine_ordinal: 0,
                requirement: "Fixture::callback".into(),
                calling_owner_application: "Fixture".into(),
                boundary: Some(inbound_boundary),
                published_contract: declaration.contract,
                entry_admitted: true,
                body: declaration,
            });
        let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
        let artifact = std::sync::Arc::new(
            terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap(),
        );
        let thunk =
            provider_correspondence::callbacks::TerminalCallbackThunkArtifact::for_application(
                artifact.clone(),
                0,
            )
            .unwrap();
        let identity = function_identity::MachineFunctionIdentity::callback_thunk(
            artifact.view().identity(),
            0,
        )
        .unwrap();
        let inbound = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap();
        let occurrences = [provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal::new(
            0,
            0,
            registrar,
            identity,
            thunk,
            inbound.plan().clone(),
            Vec::new(),
        )];
        let targets =
            psi_target_layouts::terminal_target_context(artifact.view(), "windows_x86_64");
        for (entry, reached) in [(machine_id(1), false), (machine_id(2), true)] {
            let scope = NativeExecutionScope::derive(
                artifact.view(),
                entry,
                &targets,
                None,
                (&[], &effects::SelectedProviderPlanFacts::default()),
                &[],
                &occurrences,
            )
            .unwrap();
            assert_eq!(scope.callbacks(), &occurrences);
            let expected = if reached {
                std::slice::from_ref(&demand)
            } else {
                &[]
            };
            assert_eq!(scope.boundary_application_demands().rows(), expected);
            assert_eq!(
                scope.boundary_application_demands().terminal_psi(),
                artifact.view().identity()
            );
            assert_eq!(scope.contains_machine(machine_id(3)), reached);
            assert_eq!(scope.emitted.callbacks().len(), usize::from(reached));
            if reached {
                assert_eq!(scope.emitted.callbacks()[0].application_index, 0);
                assert_eq!(scope.emitted.callbacks()[0].terminal_operation, registrar);
            }
        }
    }

    #[test]
    fn target_context_does_not_select_source_bodies() {
        let module = graph();
        assert_eq!(
            scope(module.clone(), "macos_arm64").unwrap().machines(),
            scope(module, "windows_x86_64").unwrap().machines()
        );
    }
}
