use omega_isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperEncodingRequest,
};
use omega_program_entry_plan::OptimizedProgramStorageSemanticWrapperPlan;

#[derive(Debug)]
#[must_use = "target wrapper encoding custody must be retained through continuation resolution"]
pub struct StagedOptimizedProgramStorageSemanticWrapperEncoding {
    pub(crate) source: OptimizedProgramStorageSemanticWrapperPlan,
    pub(crate) request: X86_64SemanticUnitWrapperEncodingRequest,
    pub(crate) template: ValidatedX86_64SemanticUnitWrapperTemplate,
}

impl StagedOptimizedProgramStorageSemanticWrapperEncoding {
    pub const fn source(&self) -> &OptimizedProgramStorageSemanticWrapperPlan {
        &self.source
    }

    pub const fn request(&self) -> X86_64SemanticUnitWrapperEncodingRequest {
        self.request
    }

    pub const fn template(&self) -> &ValidatedX86_64SemanticUnitWrapperTemplate {
        &self.template
    }
}
