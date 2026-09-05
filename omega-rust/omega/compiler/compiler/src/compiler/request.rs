use crate::compiler::{ArtifactEmissionPolicy, CompileOptions, OptimizationRollback};
use crate::pipeline::PackageCompilationInputs;
use diagnostics::Diagnostic;

mod multi_target;
mod targets;
pub use multi_target::{
    ExactTargetCompileOutcome, MultiTargetCompileOutcomes, MultiTargetCompileRequest,
};
pub use targets::ExplicitTargetSet;

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
/// This Rust-host object is neither D18's logical standalone request nor D25's
/// canonical `OCREQ` frame, and its memory layout carries no bootstrap
/// authority. The eventual adapter must independently reconstruct the sealed
/// `OmegaCompilationSubject`/`OmegaInvocation` bytes before comparing this
/// implementation with either Alpha-tape compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    pub(crate) options: CompileOptions,
    pub(crate) requested_product: RequestedCompileProduct,
    pub(crate) artifact_policy: ArtifactEmissionPolicy,
    pub(crate) terminal_admission_profile: proof_admission::AdmissionProfile,
    pub(crate) terminal_authority_permission_policy:
        native_realization::TerminalAuthorityPermissionPolicy,
    pub(crate) accepted_trust_admissions: Vec<trust_model::TrustAdmission>,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
    pub(crate) optimization_rollback: OptimizationRollback,
}

/// Request whose cross-field product constraints have been admitted before
/// any source acquisition or reporting filesystem effect.
#[derive(Debug)]
pub(super) struct ValidatedCompileRequest(CompileRequest);

impl CompileRequest {
    /// Creates a checking request. Callers requesting another product must
    /// name it explicitly.
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            requested_product: RequestedCompileProduct::Check,
            artifact_policy: ArtifactEmissionPolicy::Full,
            terminal_admission_profile: proof_admission::AdmissionProfile::default(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
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
    pub fn with_admission_profile(mut self, profile: proof_admission::AdmissionProfile) -> Self {
        self.terminal_admission_profile = profile;
        self
    }

    /// Supply the receiving authority's exact service-schema/requirement
    /// permissions for native realization. The default is deny-by-absence.
    pub fn with_terminal_authority_permission_policy(
        mut self,
        policy: native_realization::TerminalAuthorityPermissionPolicy,
    ) -> Self {
        self.terminal_authority_permission_policy = policy;
        self
    }

    /// Supply the complete, exact admission set selected by owner policy.
    /// Compilation compares this in-memory set with independently
    /// reconstructed obligations and performs no policy discovery.
    pub fn with_accepted_trust_admissions(
        mut self,
        admissions: Vec<trust_model::TrustAdmission>,
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

    pub(super) fn validate_for_execution(
        mut self,
    ) -> Result<ValidatedCompileRequest, Vec<Diagnostic>> {
        if !self.optimization_rollback.is_empty()
            && self.requested_product != RequestedCompileProduct::NativeArtifact
        {
            let names = self
                .optimization_rollback
                .requested_disabled()
                .as_slice()
                .iter()
                .map(|optimization| format!("`{}`", optimization.build_case_name()))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(vec![Diagnostic::error(format!(
                "optimization rollback {names} requires NativeArtifact production; {:?} does not enter native optimizer realization",
                self.requested_product
            ))]);
        }
        if self.requested_product == RequestedCompileProduct::NativeArtifact
            && self.options.target_name.is_none()
        {
            self.options.target_name = Some(target::TargetProfile::host().target_name().to_owned());
        }
        if let Some(requested) = self.options.target_name.as_deref() {
            let profile = target::TargetProfile::from_omega_target_name(Some(requested))
                .map_err(|diagnostic| vec![diagnostic])?;
            self.options.target_name = Some(profile.target_name().to_owned());
        }
        Ok(ValidatedCompileRequest(self))
    }
}

impl ValidatedCompileRequest {
    pub(super) const fn options(&self) -> &CompileOptions {
        &self.0.options
    }

    pub(super) const fn requested_product(&self) -> RequestedCompileProduct {
        self.0.requested_product
    }

    pub(super) const fn artifact_policy(&self) -> ArtifactEmissionPolicy {
        self.0.artifact_policy
    }

    pub(super) fn accepted_trust_admissions(&self) -> &[trust_model::TrustAdmission] {
        &self.0.accepted_trust_admissions
    }

    pub(super) const fn package_inputs(&self) -> Option<&PackageCompilationInputs> {
        self.0.package_inputs.as_ref()
    }

    pub(super) fn into_inner(self) -> CompileRequest {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optimization_core::Optimization;

    fn request(product: RequestedCompileProduct, rollback: OptimizationRollback) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: "missing.omg".into(),
            build_dir: None,
            target_name: None,
        })
        .with_requested_product(product)
        .with_optimization_rollback(rollback)
    }

    #[test]
    fn request_admission_accepts_empty_rollback_for_every_product() {
        for product in [
            RequestedCompileProduct::Check,
            RequestedCompileProduct::TerminalArtifact,
            RequestedCompileProduct::NativeArtifact,
        ] {
            let admitted = request(product, OptimizationRollback::default())
                .validate_for_execution()
                .expect("empty rollback is valid for every product");
            assert_eq!(admitted.requested_product(), product);
        }
    }

    #[test]
    fn request_admission_rejects_non_native_rollback_with_stable_diagnostic() {
        let rollback = OptimizationRollback::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        for product in [
            RequestedCompileProduct::Check,
            RequestedCompileProduct::TerminalArtifact,
        ] {
            let diagnostics = request(product, rollback.clone())
                .validate_for_execution()
                .expect_err("non-native rollback must reject during request admission");
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(
                diagnostics[0].message,
                format!(
                    "optimization rollback `ControlFlowCleanup`, `CopyPropagation` requires NativeArtifact production; {product:?} does not enter native optimizer realization"
                )
            );
        }
        assert!(
            request(RequestedCompileProduct::NativeArtifact, rollback)
                .validate_for_execution()
                .is_ok()
        );
    }

    #[test]
    fn only_native_product_resolves_an_absent_target_to_host() {
        for product in [
            RequestedCompileProduct::Check,
            RequestedCompileProduct::TerminalArtifact,
        ] {
            let admitted = request(product, OptimizationRollback::default())
                .validate_for_execution()
                .expect("target-neutral request remains valid");
            assert_eq!(admitted.options().target_name, None);
        }

        let admitted = request(
            RequestedCompileProduct::NativeArtifact,
            OptimizationRollback::default(),
        )
        .validate_for_execution()
        .expect("native request resolves Host convenience");
        assert_eq!(
            admitted.options().target_name.as_deref(),
            Some(target::TargetProfile::host().target_name())
        );
    }

    #[test]
    fn explicit_canonical_target_is_preserved_by_request_admission() {
        let mut request = request(
            RequestedCompileProduct::NativeArtifact,
            OptimizationRollback::default(),
        );
        request.options.target_name = Some("linux_arm64".to_owned());
        let admitted = request
            .validate_for_execution()
            .expect("explicit target remains valid until exact profile parsing");
        assert_eq!(
            admitted.options().target_name.as_deref(),
            Some("linux_arm64")
        );
    }

    #[test]
    fn legacy_cli_target_alias_normalizes_during_request_admission() {
        let mut request = request(
            RequestedCompileProduct::NativeArtifact,
            OptimizationRollback::default(),
        );
        request.options.target_name = Some("windows_x64".to_owned());
        let admitted = request
            .validate_for_execution()
            .expect("legacy CLI alias should remain an accepted input");
        assert_eq!(
            admitted.options().target_name.as_deref(),
            Some("windows_x86_64")
        );
    }
}
