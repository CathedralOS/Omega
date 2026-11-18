//! Current, framed native code and the records needed to interpret its bytes.

use crate::{
    FunctionFragmentEmissionStatistics, FunctionFragmentFrameApplication, ResolvedMachineLayout,
    SelectedFormEncoding, TargetFrameLayoutPlan, TargetFrameProtocolEncodingPlan,
    frame_branch_origins::FunctionFrameBranchOrigins,
};
use register_homes::AllocationPublicationFacts;
use register_model::ValidatedPhysicalRegisterModel;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBodyReadback {
    pub physical: ValidatedPhysicalRegisterModel,
    pub encoding: Arc<SelectedFormEncoding>,
    pub layout: Arc<ResolvedMachineLayout>,
    pub frame: Arc<TargetFrameLayoutPlan>,
    pub protocol: Arc<TargetFrameProtocolEncodingPlan>,
    pub frame_application: Arc<FunctionFragmentFrameApplication>,
    pub branches: Arc<[FunctionFrameBranchOrigins]>,
    pub allocation: Arc<AllocationPublicationFacts>,
    pub fragment_statistics: FunctionFragmentEmissionStatistics,
}
