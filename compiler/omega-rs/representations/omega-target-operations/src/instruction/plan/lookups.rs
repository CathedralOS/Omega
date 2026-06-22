use crate::instruction::plan::TargetOperationPlan;
use crate::{HostOperationKey, TargetHostBinding};

impl TargetOperationPlan {
    pub fn host_binding(&self, operation_key: HostOperationKey) -> Option<&TargetHostBinding> {
        self.code
            .host_bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
            .map(|(_, binding)| binding)
    }
}
