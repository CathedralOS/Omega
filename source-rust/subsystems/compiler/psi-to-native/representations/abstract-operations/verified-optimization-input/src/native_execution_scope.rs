//! The target-selected reach and the checked proposal which issued it travel together through lowering.

use provider_admission::provider_installation::SelectedProviderAdapter;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use target_vocabulary::publication::EmittedExecutionScope;
use terminal_psi::{OperationKind, TerminalModule};
use terminal_semantics::closed_owned_sequence_bounds;
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
        let catalog = &module.scalars.scalar_applications;
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
                .types.structural_types
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
                .interfaces.boundary_machines
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionScope {
    target_context: psi_target_layouts::TerminalTargetContext,
    entry: MachineId,
    selected_provider_closure: [u8; 32],
    source_native_bindings: Option<[u8; 32]>,
    source_provider_selection: selected_provider_plans::SelectedProviderPlanFacts,
    source_checked_entry: Option<psi_publication_records::CheckedProgramEntryTerminalReceipt>,
    local_adapters: Vec<SelectedProviderAdapter>,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    callbacks: Vec<provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal>,
    pub(super) emitted: std::sync::Arc<EmittedExecutionScope>,
    owned_sequence_bounds: Vec<(semantic_vocabulary::StructuralTypeId, u64)>,
    demands: terminal_psi::TerminalBoundaryApplicationDemands,
    selected_candidate_positions: Vec<usize>,
}

/// Identity of the artifact, native entry and provider selection that a reach must retain.
pub struct NativeScopeBinding<'a> {
    artifact: terminal_codec::TerminalArtifactView<'a>,
    entry: MachineId,
    targets: psi_target_layouts::TerminalTargetContext,
    entry_call: Option<crate::SelectedProgramEntryCall>,
    proposal: &'a crate::TerminalNativeRealizationProposal,
    parent: Option<&'a crate::ScopedProviderSelection>,
}

impl<'a> NativeScopeBinding<'a> {
    pub fn checked(
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        proposal: &'a crate::TerminalNativeRealizationProposal,
        targets: psi_target_layouts::TerminalTargetContext,
        root: terminal_psi::TargetMachineCallable,
        parent: Option<&'a crate::ScopedProviderSelection>,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        use diagnostics::Diagnostic;
        let program = proposal.program_entry();
        let view = artifact.view();
        if program.checked_entry().terminal_psi_identity() != view.identity() {
            return Err(vec![Diagnostic::error(
                "native proposal belongs to another canonical artifact",
            )]);
        }
        if targets.terminal_psi() != view.identity()
            || targets.target() != proposal.target_profile().target_name()
        {
            return Err(vec![Diagnostic::error(
                "native scope belongs to another canonical artifact or target",
            )]);
        }
        let entry_call = if let Some(parent) = parent {
            parent
                .authorization()
                .require_artifact(artifact)
                .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
            parent.scope().require_proposal_context(view, proposal)?;
            None
        } else {
            if root != proposal.target_program_entry() {
                return Err(vec![Diagnostic::error(
                    "native root scope differs from the proposal's selected program entry",
                )]);
            }
            crate::entry_settlement::bind_selected_program_entry_call(
                artifact,
                program,
                &proposal.providers().selection,
                &proposal.providers().intrinsic_executions,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?
        };
        let entry = targets
            .entry(view.identity(), artifact.semantic_module(), root)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        Ok(Self {
            artifact: view,
            entry,
            targets,
            entry_call,
            proposal,
            parent,
        })
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }
    pub fn entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        self.entry_call.as_ref()
    }
    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        self.parent
            .map_or(&self.proposal.providers().checked_adapters, |parent| {
                parent.scope().local_adapters()
            })
    }

    /// Seal one stage-01 traversal against the program and deployment that requested it.
    pub fn bind_target_reach(
        self,
        emitted: EmittedExecutionScope,
        selected_candidate_positions: Vec<usize>,
    ) -> Result<NativeExecutionScope, &'static str> {
        let local_adapters = self.local_adapters().to_vec();
        let module = self.artifact.module();
        if !emitted.contains_machine(self.entry) {
            return Err("native execution scope omitted its selected entry");
        }
        let owned_sequence_bounds =
            closed_owned_sequence_bounds(module, |machine| emitted.contains_machine(machine))?;
        let demands = terminal_psi::TerminalBoundaryApplicationDemands::new(
            self.artifact.identity(),
            module.boundary_application_demands_for_machines(emitted.machines())?,
        )?;
        let proposal = self.proposal;
        let selection = &proposal.providers().selection;
        Ok(NativeExecutionScope {
            target_context: self.targets,
            entry: self.entry,
            source_provider_selection: selection.clone(),
            selected_provider_closure: *self
                .parent
                .map(crate::ScopedProviderSelection::selection)
                .unwrap_or(selection)
                .identity_digest()
                .as_bytes(),
            source_checked_entry: Some(proposal.program_entry().checked_entry().clone()),
            source_native_bindings: Some(proposal.providers().native_bindings.identity_digest()),
            entry_call: self.entry_call,
            local_adapters,
            callbacks: proposal.occurrences().callbacks.clone(),
            emitted: std::sync::Arc::new(emitted),
            owned_sequence_bounds,
            demands,
            selected_candidate_positions,
        })
    }
}

impl NativeExecutionScope {
    pub fn require_source_input(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        entry: MachineId,
        targets: &psi_target_layouts::TerminalTargetContext,
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
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

    pub(crate) fn checked_entry(&self) -> Option<&psi_publication_records::CheckedProgramEntryTerminalReceipt> {
        self.source_checked_entry.as_ref()
    }

    pub(crate) fn source_native_bindings(&self) -> Option<[u8; 32]> {
        self.source_native_bindings
    }

    pub fn source_provider_selection(&self) -> &selected_provider_plans::SelectedProviderPlanFacts {
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
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
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
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
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
    ) -> &terminal_psi::TerminalBoundaryApplicationDemands {
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

    pub const fn emitted_reach(&self) -> &std::sync::Arc<EmittedExecutionScope> {
        &self.emitted
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
