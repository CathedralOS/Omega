use omega_object_file::{ObjectPlan, RelocationPlan};
use omega_target::NativeTarget;

pub struct ExecutableImageInput<'a> {
    pub target: NativeTarget,
    /// Structurally validated callback-placement identity retained by backend
    /// planning for the final image/certificate publication join.
    pub callback_placement_identity_fingerprint: u64,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub encoded_machine_code: &'a omega_machine_bytes::EncodedMachineCode,
    pub encoded_machine_semantics: &'a omega_machine_bytes::EncodedMachineSemanticSummary,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
    /// PE optional-header Subsystem (console 3, gui 2, efi_application 10);
    /// non-PE image formats ignore it.
    pub subsystem: u16,
}
