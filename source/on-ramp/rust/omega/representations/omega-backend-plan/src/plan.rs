use crate::{
    BackendArtifactRoots, BackendPlanPhaseTiming, BoundNominalCallbackPlacement,
    CallbackPrivateRelocationDemand, CallbackRegistrarArgumentBinding,
    CallbackRegistrarAssignedOperandBinding, CallbackRegistrarPhysicalDestination,
    CallbackThunkPlan,
};
use omega_abstract_operations::{AbstractDataPlan, AbstractOperationPlan};
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_calling_conventions::{BoundaryEntryPlan, HostAbiPlan};
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_layout::LayoutPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_dispatch::StateDispatchPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::{NativeTarget, TargetProfile};
use omega_target_operations::{InstructionPlan, TargetDataPlan};
use psi_arena::Arena;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendPlan {
    /// Exact deployment policy identity. This must not be reconstructed from
    /// `target`: profiles such as Windows and UEFI deliberately share one
    /// native architecture/object tuple while owning different root policy.
    pub target_profile: TargetProfile,
    pub target: NativeTarget,
    pub artifacts: BackendArtifactRoots,
    pub host_abi: Arc<HostAbiPlan>,
    pub host_calls: Arc<HostCallPlan>,
    pub state_calls: Arc<StateCallPlan>,
    pub alias_flow: AliasFlowPlan,
    pub state_storage: Arc<StateStoragePlan>,
    pub state_values: StateValuePlan,
    pub abstract_data: AbstractDataPlan,
    pub data: TargetDataPlan,
    pub abstract_operations: AbstractOperationPlan,
    pub target_operations: InstructionPlan,
    pub assigned_target_operations: AssignedTargetOperationPlan,
    pub control_flow: Arc<ControlFlowPlan>,
    pub runtime_flow: Arc<RuntimeFlowPlan>,
    pub state_dispatch: Arc<StateDispatchPlan>,
    pub state_guards: Arc<StateGuardPlan>,
    pub runtime_bodies: Arc<RuntimeDispatchBodyPlan>,
    pub runtime_branching_calls: RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: RuntimeDispatchLoopPlan,
    pub runtime_storage: RuntimeStoragePlan,
    pub runtime_text: RuntimeTextPlan,
    pub layouts: Arc<LayoutPlan>,
    pub entry_key: StateKey,
    /// Exact source-evaluated plan selected by an explicit target-owned entry
    /// slot. Legacy name-discovered entries retain `None` until corpus
    /// migration removes that compatibility path.
    pub entry_boundary_plan: Option<BoundaryEntryPlan>,
    /// Exact target-owned recipes for callback thunks, already joined to the
    /// checked nominal-use sites that require them. Later lowering consumes
    /// these rows directly and must not reconstruct ABI placement.
    pub callback_placements: Arc<[BoundNominalCallbackPlacement]>,
    /// Private inbound-function identities resolved to exact control-flow
    /// entries. This is the address-free input to future multi-entry target
    /// instruction and object-symbol lowering.
    pub callback_thunks: Arc<[CallbackThunkPlan]>,
    /// Complete ordered address-free joins from target-closed private
    /// materialization rows to emitted callback thunk identities.
    pub callback_private_relocations: Arc<[CallbackPrivateRelocationDemand]>,
    /// Complete address-free joins from each private relocation demand to the
    /// exact outbound registrar occurrence and native destination root.
    pub callback_registrar_arguments: Arc<[CallbackRegistrarArgumentBinding]>,
    /// Complete ABI-relative joins from callback registrar arguments to their
    /// exact outbound parameter placement and target-closed field geometry.
    pub callback_registrar_destinations: Arc<[CallbackRegistrarPhysicalDestination]>,
    /// Complete exact joins from callback registrar destinations to selected
    /// and assigned outbound host-call operands.
    pub callback_registrar_assigned_operands: Arc<[CallbackRegistrarAssignedOperandBinding]>,
    /// Per-DISPATCH-INDEX receiver storage base (indexed by the runtime-flow
    /// state's arena index): `Some(base)` when the dispatch case's clone
    /// context was minted by a call through a CONTAINED receiver whose true
    /// offset resolved (per-instance dispatch); `None` = the by-type walk
    /// stays authoritative. Computed ONCE in the pipeline builder; consumed
    /// by guard-operand layout, selection, and the contained-receiver fence
    /// -- one prediction, no lockstep copies.
    pub receiver_bases: Vec<Option<usize>>,
    /// Dispatch-index -> call-context id (the runtime-flow state's context).
    /// Slot lookups that relax state matching must stay inside one context: a
    /// machine expanded at two call sites has two slot regions under the same
    /// (machine, state) symbols, and crossing contexts reads the other call's
    /// frame (the repeated dir-walk guard miscompile).
    pub state_contexts: Vec<u32>,
    pub phase_timings: Arena<BackendPlanPhaseTiming>,
}

impl BackendPlan {
    pub fn entry_machine_name(&self) -> &str {
        self.control_flow
            .machine_by_symbol(self.entry_key.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("")
    }

    pub fn entry_state_name(&self) -> &str {
        self.control_flow
            .state_by_key(self.entry_key)
            .map(|state| state.name.as_str())
            .unwrap_or("")
    }
}

impl Deref for BackendPlan {
    type Target = BackendArtifactRoots;

    fn deref(&self) -> &Self::Target {
        &self.artifacts
    }
}

impl DerefMut for BackendPlan {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.artifacts
    }
}
