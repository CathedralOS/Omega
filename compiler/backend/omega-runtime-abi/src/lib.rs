use omega_target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAbiPlan {
    pub pointer_size: usize,
    pub pointer_alignment: usize,
}

impl Default for RuntimeAbiPlan {
    fn default() -> Self {
        build_runtime_abi_plan(NativeTarget::host())
    }
}

impl RuntimeAbiPlan {
    pub const fn string_descriptor_size(self) -> usize {
        self.pointer_size.saturating_mul(2)
    }

    pub const fn slice_descriptor_size(self) -> usize {
        self.pointer_size.saturating_mul(2)
    }
}

pub const fn build_runtime_abi_plan(target: NativeTarget) -> RuntimeAbiPlan {
    RuntimeAbiPlan {
        pointer_size: target.pointer_size,
        pointer_alignment: target.pointer_alignment,
    }
}
