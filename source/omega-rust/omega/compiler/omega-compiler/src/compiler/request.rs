use crate::compiler::{ArtifactEmissionPolicy, CompileOptions};
use crate::pipeline::PackageCompilationInputs;

/// The semantic product requested from the production compiler pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedCompileProduct {
    Check,
    TerminalArtifact,
    NativeArtifact,
}

/// One typed production compiler invocation.
///
/// This request owns production policy and input; [`super::Compiler`] only
/// coordinates the resulting phase transitions. Publication is a subsequent
/// operation on a retained native product, not a requested compiler mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    pub(crate) options: CompileOptions,
    pub(crate) requested_product: RequestedCompileProduct,
    pub(crate) artifact_policy: ArtifactEmissionPolicy,
    pub(crate) terminal_admission_profile: psi_proof_admission::AdmissionProfile,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
}

impl CompileRequest {
    /// Creates a checking request.
    ///
    /// Product selection is never inferred from compatibility options such as
    /// `write_output`; callers requesting another product must name it.
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            requested_product: RequestedCompileProduct::Check,
            artifact_policy: ArtifactEmissionPolicy::Full,
            terminal_admission_profile: psi_proof_admission::AdmissionProfile::default(),
            package_inputs: None,
        }
    }

    pub fn with_requested_product(mut self, requested_product: RequestedCompileProduct) -> Self {
        self.requested_product = requested_product;
        self
    }

    pub fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }

    /// Select the exact owner-accepted admission set used while verifying a
    /// canonical Terminal artifact. The default is empty and therefore
    /// rejects every admission-authorized obligation.
    pub fn with_admission_profile(
        mut self,
        profile: psi_proof_admission::AdmissionProfile,
    ) -> Self {
        self.terminal_admission_profile = profile;
        self
    }

    pub fn with_package_inputs(mut self, package_inputs: PackageCompilationInputs) -> Self {
        self.package_inputs = Some(package_inputs);
        self
    }

    pub const fn options(&self) -> &CompileOptions {
        &self.options
    }

    pub const fn requested_product(&self) -> RequestedCompileProduct {
        self.requested_product
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn request_product_is_not_inferred_from_write_output() {
        let request = CompileRequest::new(CompileOptions {
            root_path: PathBuf::from("main.omg"),
            build_dir: None,
            target_name: None,
            write_output: true,
        });

        assert_eq!(request.requested_product(), RequestedCompileProduct::Check);
    }
}
