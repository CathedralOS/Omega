//! Reach, storage geometry and scalar inputs bound to one native entry.
#![forbid(unsafe_code)]

pub mod builtins;
mod execution_preparation;
pub use execution_preparation::PreparedWorker;
mod entry_call;
pub mod entry_settlement;
mod native_execution_scope;

pub use entry_call::SelectedProgramEntryCall;
pub use native_execution_scope::{
    AssemblyCallingPlan, NativeExecutionScope, NativeRoot, NativeScopeBinding, NativeScopeSource,
    ScopedProviderSelection,
};
pub use provider_bindings::{
    SelectedProviderAdapter, SelectedProviderAdapters, retain_selected_provider_adapters,
};
mod provider_bindings;
mod scalar_demand;

pub use provider_bindings::ScopedProviderBindings;

use native_execution_reach::ExecutionReach;
use psi_target_layouts::{AdmittedTargetLayouts, TerminalTargetContext};
use scalar_demand::ScalarDemand;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarApplicationId};
use std::sync::Arc;
use terminal_psi::TerminalModule;
use terminal_semantics::closed_owned_sequence_bounds;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetBoundReach {
    target_context: TerminalTargetContext,
    entry: MachineId,
    emitted: Arc<ExecutionReach>,
    layouts: Arc<AdmittedTargetLayouts>,
    scalar_applications: Arc<[ScalarApplicationId]>,
    owned_sequence_bounds: Vec<(semantic_vocabulary::StructuralTypeId, u64)>,
    execution_owners: Vec<runtime_abi::ExecutionOwnerHome>,
    steps: Vec<(
        BoundaryMachineId,
        terminal_semantics::calls::owned_activation::StepShape,
    )>,
    demands: terminal_psi::TerminalBoundaryApplicationDemands,
    selected_candidate_positions: Vec<usize>,
}

impl TargetBoundReach {
    /// Bind the traversal to this program and entry. Private entries inherit their
    /// deployment's layouts and derive scalar demand from their own reach.
    pub fn bind(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        targets: TerminalTargetContext,
        profile: target::TargetProfile,
        entry: MachineId,
        emitted: ExecutionReach,
        mut selected_candidate_positions: Vec<usize>,
        prepared: &[PreparedWorker],
        parent: Option<&Self>,
    ) -> Result<Self, String> {
        if targets.terminal_psi() != artifact.identity()
            || targets.target() != profile.target_name()
        {
            return Err("native scope belongs to another canonical artifact or target".into());
        }
        let module = artifact.module();
        if !emitted.contains_machine(entry) {
            return Err("native execution scope omitted its selected entry".into());
        }
        selected_candidate_positions.sort_unstable();
        if selected_candidate_positions
            .last()
            .is_some_and(|position| *position >= module.interfaces.provider_candidates.len())
        {
            return Err(
                "selected native provider candidate is outside the canonical catalog".into(),
            );
        }
        let owned_sequence_bounds =
            closed_owned_sequence_bounds(module, |machine| emitted.contains_machine(machine))?;
        let demands = terminal_psi::TerminalBoundaryApplicationDemands::new(
            artifact.identity(),
            module.boundary_application_demands_for_machines(emitted.machines())?,
        )?;
        let layouts = if let Some(parent) = parent {
            if parent.target_context != targets {
                return Err(
                    "callable layouts belong to another canonical artifact or target".into(),
                );
            }
            Arc::clone(&parent.layouts)
        } else {
            profile.admit_address_representation()?;
            let facts = profile
                .layout_target_facts()
                .ok_or("selected native target has no representation facts")?;
            let layouts = psi_target_layouts::admit_target_layouts(artifact, &targets, &facts)
                .map_err(|error| error.to_string())?;
            Arc::new(layouts)
        };
        let (execution_owners, steps) = execution_preparation::owner_homes(
            module,
            &emitted,
            profile.native_target(),
            prepared,
            parent,
        )?;
        let scalar_applications = ScalarDemand {
            emitted: Some(&emitted),
            storage: runtime_abi::NativeValueStorage {
                sequences: &owned_sequence_bounds,
                execution_owners: &execution_owners,
            },
        }
        .scalar_application_demand(module, &layouts)?;
        Ok(Self {
            target_context: targets,
            entry,
            emitted: Arc::new(emitted),
            layouts,
            scalar_applications: scalar_applications.into(),
            owned_sequence_bounds,
            execution_owners,
            steps,
            demands,
            selected_candidate_positions,
        })
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
            storage: runtime_abi::NativeValueStorage {
                sequences: &bounds,
                ..Default::default()
            },
        }
        .scalar_application_demand(module, layouts)
    }

    pub fn owned_sequence_bounds(&self) -> &[(semantic_vocabulary::StructuralTypeId, u64)] {
        &self.owned_sequence_bounds
    }

    pub fn value_storage(&self) -> runtime_abi::NativeValueStorage<'_> {
        runtime_abi::NativeValueStorage {
            sequences: &self.owned_sequence_bounds,
            execution_owners: &self.execution_owners,
        }
    }

    pub fn step(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&terminal_semantics::calls::owned_activation::StepShape> {
        self.steps
            .binary_search_by_key(&boundary, |(id, _)| *id)
            .ok()
            .map(|i| &self.steps[i].1)
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn target_context(&self) -> &psi_target_layouts::TerminalTargetContext {
        &self.target_context
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

    /// All entries in this deployment use the same admitted geometry.
    pub fn layouts(&self) -> &AdmittedTargetLayouts {
        &self.layouts
    }

    /// Only storage extents and scalar values needed by this entry’s reach.
    pub fn scalar_application_demand(&self) -> &[ScalarApplicationId] {
        &self.scalar_applications
    }

    pub const fn emitted_reach(&self) -> &Arc<ExecutionReach> {
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
