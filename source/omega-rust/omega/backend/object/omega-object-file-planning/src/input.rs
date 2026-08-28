use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::MachineFunctionIdentity;
use omega_layout::LayoutPlan;
use omega_machine_bytes::EncodedMachinePlan;
use omega_target::NativeTarget;
use omega_target_operations::TargetDataPlan;
use psi_symbols::SymbolHandle;

pub struct ObjectPlanningInput<'plan> {
    pub target: NativeTarget,
    pub host_abi: &'plan HostAbiPlan,
    pub layouts: &'plan LayoutPlan,
    pub entry_machine_symbol: SymbolHandle,
    pub entry_machine_name: &'plan str,
    pub entry_function_identity: MachineFunctionIdentity,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub data: &'plan TargetDataPlan,
    pub runtime_frame_size: usize,
    pub runtime_frame_alignment: usize,
}
