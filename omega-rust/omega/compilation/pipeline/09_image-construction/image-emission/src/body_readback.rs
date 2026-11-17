//! Byte readback inputs retained by an emitted body.
//!
//! These are encoded bytes, register/frame descriptions and exact source
//! obligations. They contain no selected instructions or compiler stages.
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeBodyReadback {
    pub cpu: machine_code::cpu_origins::PhysicalCpuOrigins,
    pub faults: Result<machine_code::fault_origins::PhysicalFaultOrigins, String>,
    pub physical: register_model::ValidatedPhysicalRegisterModel,
    pub encoding: Arc<machine_code::SelectedFormEncoding>,
    pub layout: Arc<machine_code::ResolvedMachineLayout>,
    pub frame: Arc<machine_code::TargetFrameLayoutPlan>,
    pub protocol: Arc<machine_code::TargetFrameProtocolEncodingPlan>,
    pub fragments: machine_code::FunctionFragmentEmissionPlan,
    pub branches: Vec<machine_code::frame_branch_origins::FunctionFrameBranchOrigins>,
}

impl NativeBodyReadback {
    pub fn capture(container: &object_file::StagedOptimizedRelocationFreeObjectContainer) -> Self {
        let framed = container.source().source();
        let current = framed.source().source();
        let program = current.program();
        Self {
            cpu: current.facts().cpu.clone(),
            faults: current.facts().faults.clone(),
            physical: current.register_environment().physical().clone(),
            encoding: Arc::clone(&program.encoding),
            layout: Arc::clone(&program.layout),
            frame: Arc::clone(&program.frame),
            protocol: Arc::clone(&program.protocol),
            fragments: framed.fragments().clone(),
            branches: framed.branch_origins().to_vec(),
        }
    }
}
