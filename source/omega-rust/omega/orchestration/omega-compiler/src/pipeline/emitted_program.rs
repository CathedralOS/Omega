/// Complete legacy-backend payload awaiting output publication.
///
/// This is an internal handoff between emission and publication. It is not a
/// compiler report and must disappear with the StateGraph backend.
#[derive(Debug)]
pub(crate) struct EmittedProgram {
    pub(crate) target: omega_target::NativeTarget,
    pub(crate) subsystem: u16,
    pub(crate) planned_text_bytes: usize,
    pub(crate) callback_placement_identity_fingerprint: u64,
    pub(crate) object: omega_object_file::ObjectPlan,
    pub(crate) relocations: omega_object_file::RelocationPlan,
    pub(crate) encoded_machine_code: omega_machine_bytes::EncodedMachineCode,
    pub(crate) encoded_machine_semantics: omega_machine_bytes::EncodedMachineSemanticSummary,
    pub(crate) text_bytes: Vec<u8>,
    pub(crate) data_bytes: Vec<u8>,
}
