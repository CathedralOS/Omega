use omega_object_file::{ObjectPlan, RelocationPlan};
use omega_target::NativeTarget;

pub struct ExecutableImageInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
    /// PE optional-header Subsystem (console 3, gui 2, efi_application 10);
    /// non-PE image formats ignore it.
    pub subsystem: u16,
}
