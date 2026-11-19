// Explicit consumer-policy bindings for ordinary-package declarations that
// realize compiler-recognized semantic roles (`Console`, `ProcessExit`,
// `FilesystemHostService`, `TimeHostService`, and per-target `ProgramEntry`).
// Constructing `AcceptedSemanticBinding` records the policy authority supplied
// to compilation without inferring provenance from package or source names; the
// compiler still rejoins package owner, declaration path, normalized schema,
// provider plan, and terminal-authority permissions before admitting use.

use crate::provider_plan::ServiceSchemaDigest;
use semantic_vocabulary::PackageKeyIdentity;

pub use host_provider_inputs::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, program_entry_semantic_binding_role,
};

#[doc(hidden)]
pub fn accepted_service_schema_digest(
    role: AcceptedSemanticBindingRole,
    schema: &crate::provider_plan::ServiceSchema,
) -> ServiceSchemaDigest {
    if !matches!(
        role,
        AcceptedSemanticBindingRole::UefiX64ProgramEntry
            | AcceptedSemanticBindingRole::MacosArm64ProgramEntry
            | AcceptedSemanticBindingRole::MacosX64ProgramEntry
            | AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry
            | AcceptedSemanticBindingRole::LinuxArm64ProgramEntry
            | AcceptedSemanticBindingRole::WindowsX64ProgramEntry
    ) {
        return schema.identity_digest();
    }
    schema.declaration_identity_digest()
}

use crate::provider_plan::EvaluatedNativeBinding;
use declaration_contracts::selection::NativeBindingProducer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingProduction {
    pub producer: NativeBindingProducer,
    pub evaluated: EvaluatedNativeBinding,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageNativeBindingOrigin {
    pub primary: NativeBindingProduction,
    pub equivalents: Vec<NativeBindingProduction>,
}
