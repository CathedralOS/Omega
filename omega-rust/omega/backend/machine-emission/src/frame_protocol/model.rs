use crate::frame_protocol::TargetFrameLayoutIdentity;
pub use machine_code::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy,
};
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFrameProtocolEncodingReceipt {
    identity: TargetFrameProtocolEncodingIdentity,
    frame_layout: TargetFrameLayoutIdentity,
    target: NativeTarget,
    policy: TargetFrameProtocolEncodingPolicy,
    function_count: usize,
    byte_count: usize,
    nonempty_prologue_count: usize,
    nonempty_epilogue_count: usize,
}

impl TargetFrameProtocolEncodingReceipt {
    pub const fn identity(self) -> TargetFrameProtocolEncodingIdentity {
        self.identity
    }
    pub const fn frame_layout(self) -> TargetFrameLayoutIdentity {
        self.frame_layout
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn policy(self) -> TargetFrameProtocolEncodingPolicy {
        self.policy
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn byte_count(self) -> usize {
        self.byte_count
    }
    pub const fn nonempty_prologue_count(self) -> usize {
        self.nonempty_prologue_count
    }
    pub const fn nonempty_epilogue_count(self) -> usize {
        self.nonempty_epilogue_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetFrameProtocolEncoding {
    pub(in crate::frame_protocol) plan: std::sync::Arc<TargetFrameProtocolEncodingPlan>,
    pub(in crate::frame_protocol) receipt: TargetFrameProtocolEncodingReceipt,
}

impl ValidatedTargetFrameProtocolEncoding {
    pub fn plan(&self) -> &TargetFrameProtocolEncodingPlan {
        &self.plan
    }
    pub fn shared_plan(&self) -> std::sync::Arc<TargetFrameProtocolEncodingPlan> {
        std::sync::Arc::clone(&self.plan)
    }
    pub const fn receipt(&self) -> TargetFrameProtocolEncodingReceipt {
        self.receipt
    }
}

pub(super) fn seal(plan: &TargetFrameProtocolEncodingPlan) -> TargetFrameProtocolEncodingReceipt {
    TargetFrameProtocolEncodingReceipt {
        identity: super::target_frame_protocol_encoding_identity(plan),
        frame_layout: plan.frame_layout,
        target: plan.target,
        policy: plan.policy,
        function_count: plan.functions.len(),
        byte_count: plan.bytes.len(),
        nonempty_prologue_count: plan
            .functions
            .iter()
            .filter(|row| row.prologue.length != 0)
            .count(),
        nonempty_epilogue_count: plan
            .functions
            .iter()
            .filter(|row| row.epilogue.length != 0)
            .count(),
    }
}
