//! Read-only native wrapper issued for one authenticated core activation call.
use semantic_vocabulary::MachineId;
use target_vocabulary::NormalizedForeignCallBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFixedActivationCall<Origin> {
    binding: NormalizedForeignCallBinding,
    bytes: Vec<u8>,
    worker_call: Option<(usize, MachineId)>,
    caller_stack_bytes: u64,
    origin: std::marker::PhantomData<fn(Origin) -> Origin>,
}

impl<Origin> EncodedFixedActivationCall<Origin> {
    pub fn from_encoder(_origin: Origin, binding: NormalizedForeignCallBinding, bytes: Vec<u8>,
        worker_call: Option<(usize,MachineId)>, caller_stack_bytes:u64) -> Self {
        Self {binding,bytes,worker_call,caller_stack_bytes,origin:std::marker::PhantomData}
    }
    pub fn binding(&self) -> &NormalizedForeignCallBinding { &self.binding }
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    /// Offset of the rel32 field in the wrapper image and the regular worker it targets.
    pub fn worker_call(&self) -> Option<(usize,MachineId)> { self.worker_call }
    pub fn caller_stack_bytes(&self) -> u64 { self.caller_stack_bytes }
}
