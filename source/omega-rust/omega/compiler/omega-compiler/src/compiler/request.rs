use crate::compiler::{ArtifactEmissionPolicy, CompileOptions, OptimizationRollback};
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
    pub(crate) accepted_trust_admissions: Vec<omega_trust_model::TrustAdmission>,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
    pub(crate) optimization_rollback: OptimizationRollback,
}

impl CompileRequest {
    /// Creates a checking request. Callers requesting another product must
    /// name it explicitly.
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            requested_product: RequestedCompileProduct::Check,
            artifact_policy: ArtifactEmissionPolicy::Full,
            terminal_admission_profile: psi_proof_admission::AdmissionProfile::default(),
            accepted_trust_admissions: Vec::new(),
            package_inputs: None,
            optimization_rollback: OptimizationRollback::default(),
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

    /// Supply the complete, exact admission set selected by owner policy.
    /// Compilation compares this in-memory set with independently
    /// reconstructed obligations and performs no policy discovery.
    pub fn with_accepted_trust_admissions(
        mut self,
        admissions: Vec<omega_trust_model::TrustAdmission>,
    ) -> Self {
        self.accepted_trust_admissions = admissions;
        self
    }

    pub fn with_package_inputs(mut self, package_inputs: PackageCompilationInputs) -> Self {
        self.package_inputs = Some(package_inputs);
        self
    }

    pub fn with_optimization_rollback(
        mut self,
        optimization_rollback: OptimizationRollback,
    ) -> Self {
        self.optimization_rollback = optimization_rollback;
        self
    }

    pub const fn options(&self) -> &CompileOptions {
        &self.options
    }

    pub const fn requested_product(&self) -> RequestedCompileProduct {
        self.requested_product
    }

    pub const fn optimization_rollback(&self) -> &OptimizationRollback {
        &self.optimization_rollback
    }
}
