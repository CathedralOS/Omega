use crate::compiler::{ArtifactEmissionPolicy, CompileOptions};
use crate::pipeline::{ExecutableTcbBuildPolicy, PackageCompilationInputs};

/// The semantic product requested from the production compiler pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedCompileProduct {
    Check,
    TerminalArtifact,
    NativeArtifact,
    InstalledOutput,
}

impl RequestedCompileProduct {
    pub(crate) const fn from_legacy_write_output(write_output: bool) -> Self {
        if write_output {
            Self::InstalledOutput
        } else {
            Self::Check
        }
    }
}

/// One typed production compiler invocation.
///
/// Test-only entry overrides and worker ceilings deliberately remain on the
/// separate harness request. This request owns production policy and input;
/// [`super::Compiler`] only coordinates the resulting phase transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    pub(crate) options: CompileOptions,
    pub(crate) requested_product: RequestedCompileProduct,
    pub(crate) executable_tcb_policy: ExecutableTcbBuildPolicy,
    pub(crate) artifact_policy: ArtifactEmissionPolicy,
    pub(crate) terminal_admission_profile: psi_proof_admission::AdmissionProfile,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
}

impl CompileRequest {
    pub fn new(options: CompileOptions) -> Self {
        let requested_product =
            RequestedCompileProduct::from_legacy_write_output(options.write_output);
        Self {
            options,
            requested_product,
            executable_tcb_policy: ExecutableTcbBuildPolicy::default(),
            artifact_policy: ArtifactEmissionPolicy::Full,
            terminal_admission_profile: psi_proof_admission::AdmissionProfile::default(),
            package_inputs: None,
        }
    }

    pub fn with_executable_tcb_policy(
        mut self,
        executable_tcb_policy: ExecutableTcbBuildPolicy,
    ) -> Self {
        self.executable_tcb_policy = executable_tcb_policy;
        self
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
    pub fn with_terminal_admission_profile(
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
