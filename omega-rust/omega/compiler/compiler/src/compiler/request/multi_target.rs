use super::targets::ExplicitTargetSet;
use super::{CompileRequest, ValidatedCompileRequest};
use crate::compiler::report::CompileReport;
use diagnostics::Diagnostic;
use target::TargetProfile;

/// One explicit multi-target compiler invocation.
///
/// The target set is the sole source of child target identity. The request
/// factory receives each canonical profile and must return a targetless request;
/// this constructor installs that profile exactly once. Target-specific package
/// inputs, build roots, admission policy, and lowering authority may otherwise
/// differ between children.
#[derive(Debug)]
pub struct MultiTargetCompileRequest {
    target_set: ExplicitTargetSet,
    children: Vec<CompileRequest>,
}

impl MultiTargetCompileRequest {
    pub fn from_target_set<F>(
        target_set: ExplicitTargetSet,
        mut request_for_target: F,
    ) -> Result<Self, Vec<Diagnostic>>
    where
        F: FnMut(TargetProfile) -> CompileRequest,
    {
        let mut children = Vec::with_capacity(target_set.profiles().len());
        let mut diagnostics = Vec::new();
        for profile in target_set.profiles().iter().copied() {
            let mut request = request_for_target(profile);
            if let Some(duplicate) = request.options.target_name.as_deref() {
                diagnostics.push(Diagnostic::error(format!(
                    "multi-target child for `{}` also declared target `{duplicate}`; the explicit target set is the sole target source",
                    profile.target_name(),
                )));
            }
            request.options.target_name = Some(profile.target_name().to_owned());
            children.push(request);
        }
        if diagnostics.is_empty() {
            Ok(Self {
                target_set,
                children,
            })
        } else {
            Err(diagnostics)
        }
    }

    pub const fn target_set(&self) -> &ExplicitTargetSet {
        &self.target_set
    }

    pub(in crate::compiler) fn validate_batch_for_execution(
        self,
    ) -> Result<ValidatedMultiTargetCompileRequest, Vec<Diagnostic>> {
        let mut children = Vec::with_capacity(self.children.len());
        let mut diagnostics = Vec::new();
        for request in self.children {
            match request.validate_for_execution() {
                Ok(request) => children.push(request),
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let first = children
            .first()
            .expect("ExplicitTargetSet always materializes at least one child");
        let root_path = &first.options().root_path;
        let requested_product = first.requested_product();
        let artifact_policy = first.artifact_policy();
        let package_source_inputs = first
            .package_inputs()
            .map(package_compilation::PackageCompilationInputs::source_inputs);
        let mut build_directories = Vec::<(TargetProfile, std::path::PathBuf)>::new();
        for (profile, child) in self.target_set.profiles().iter().copied().zip(&children) {
            if child.options().root_path != *root_path {
                diagnostics.push(Diagnostic::error(format!(
                    "multi-target child `{}` uses a different compilation root",
                    profile.target_name(),
                )));
            }
            if child.requested_product() != requested_product {
                diagnostics.push(Diagnostic::error(format!(
                    "multi-target child `{}` requests {:?} instead of the batch product {requested_product:?}",
                    profile.target_name(),
                    child.requested_product(),
                )));
            }
            if child.artifact_policy() != artifact_policy {
                diagnostics.push(Diagnostic::error(format!(
                    "multi-target child `{}` uses {:?} artifact emission instead of the batch policy {artifact_policy:?}",
                    profile.target_name(),
                    child.artifact_policy(),
                )));
            }
            if child
                .package_inputs()
                .map(package_compilation::PackageCompilationInputs::source_inputs)
                != package_source_inputs
            {
                diagnostics.push(Diagnostic::error(format!(
                    "multi-target child `{}` does not match the batch package source inputs",
                    profile.target_name(),
                )));
            }
            let build_directory = child.options().build_dir();
            if let Some((other, _)) = build_directories
                .iter()
                .find(|(_, existing)| *existing == build_directory)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "multi-target children `{}` and `{}` name the same build directory `{}`; each child must name separate staging",
                    other.target_name(),
                    profile.target_name(),
                    build_directory.display(),
                )));
            } else {
                build_directories.push((profile, build_directory));
            }
        }
        if diagnostics.is_empty() {
            Ok(ValidatedMultiTargetCompileRequest {
                target_set: self.target_set,
                children,
            })
        } else {
            Err(diagnostics)
        }
    }
}

pub(in crate::compiler) struct ValidatedMultiTargetCompileRequest {
    target_set: ExplicitTargetSet,
    children: Vec<ValidatedCompileRequest>,
}

impl ValidatedMultiTargetCompileRequest {
    pub(in crate::compiler) fn into_parts(
        self,
    ) -> (ExplicitTargetSet, Vec<ValidatedCompileRequest>) {
        (self.target_set, self.children)
    }
}

/// One exact child's ordinary standalone compiler result.
///
/// This is an orchestration result, not semantic evidence that the target is
/// supported, tested, audited, or part of any complete deployment matrix.
#[derive(Debug)]
pub struct ExactTargetCompileOutcome {
    target: TargetProfile,
    result: Result<CompileReport, Vec<Diagnostic>>,
}

impl ExactTargetCompileOutcome {
    pub(in crate::compiler) fn new(
        target: TargetProfile,
        result: Result<CompileReport, Vec<Diagnostic>>,
    ) -> Self {
        Self { target, result }
    }

    pub const fn target_profile(&self) -> TargetProfile {
        self.target
    }

    pub fn report(&self) -> Option<&CompileReport> {
        self.result.as_ref().ok()
    }

    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        self.result.as_ref().err().map(Vec::as_slice)
    }

    pub const fn succeeded(&self) -> bool {
        self.result.is_ok()
    }

    pub fn into_result(self) -> Result<CompileReport, Vec<Diagnostic>> {
        self.result
    }
}

/// Canonically ordered outcomes for exactly one caller-supplied target set.
///
/// No durable batch manifest is implied. Each successful report and each
/// diagnostic vector is the ordinary result of its exact standalone child.
#[derive(Debug)]
pub struct MultiTargetCompileOutcomes {
    outcomes: Box<[ExactTargetCompileOutcome]>,
    #[cfg(test)]
    prepared_terminal_native_input_count: usize,
}

impl MultiTargetCompileOutcomes {
    pub(in crate::compiler) fn new(
        target_set: ExplicitTargetSet,
        outcomes: Vec<ExactTargetCompileOutcome>,
    ) -> Self {
        debug_assert_eq!(target_set.profiles().len(), outcomes.len());
        debug_assert!(
            target_set
                .profiles()
                .iter()
                .zip(&outcomes)
                .all(|(profile, outcome)| *profile == outcome.target),
        );
        Self {
            outcomes: outcomes.into_boxed_slice(),
            #[cfg(test)]
            prepared_terminal_native_input_count: 0,
        }
    }

    pub(in crate::compiler) fn with_prepared_terminal_native_input_count(
        self,
        count: usize,
    ) -> Self {
        #[cfg(test)]
        {
            let mut outcomes = self;
            outcomes.prepared_terminal_native_input_count = count;
            outcomes
        }
        #[cfg(not(test))]
        {
            let _ = count;
            self
        }
    }

    #[cfg(test)]
    pub(in crate::compiler) const fn prepared_terminal_native_input_count(&self) -> usize {
        self.prepared_terminal_native_input_count
    }

    pub const fn outcomes(&self) -> &[ExactTargetCompileOutcome] {
        &self.outcomes
    }

    pub fn into_outcomes(self) -> Box<[ExactTargetCompileOutcome]> {
        self.outcomes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{ArtifactEmissionPolicy, CompileOptions, RequestedCompileProduct};
    use package_compilation::{PackageCompilationInputs, PackageSourceBinding};

    fn package_identity(marker: u8) -> semantic_vocabulary::PackageKeyIdentity {
        semantic_vocabulary::PackageKeyIdentity::from_digest([marker; 32])
            .expect("nonzero package fixture identity")
    }

    fn package_inputs(canonical_name: &str) -> PackageCompilationInputs {
        PackageCompilationInputs::new_package(
            package_identity(1),
            vec![PackageSourceBinding::new(
                package_identity(1),
                canonical_name,
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            )],
            Vec::new(),
        )
        .expect("single-package source inputs should close")
    }

    fn request(root: &str, build_dir: &str, product: RequestedCompileProduct) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: root.into(),
            build_dir: Some(build_dir.into()),
            target_name: None,
        })
        .with_requested_product(product)
    }

    #[test]
    fn target_set_is_the_only_child_target_source() {
        let targets = ExplicitTargetSet::from_caller_names(["windows_x64", "linux_x64"])
            .expect("canonical target set");
        let batch = MultiTargetCompileRequest::from_target_set(targets, |profile| {
            request(
                "main.omg",
                profile.target_name(),
                RequestedCompileProduct::Check,
            )
        })
        .expect("targetless children should materialize");
        let validated = batch
            .validate_batch_for_execution()
            .expect("materialized children should validate");
        let (targets, children) = validated.into_parts();
        assert_eq!(
            targets.profiles(),
            &[TargetProfile::LinuxX64, TargetProfile::WindowsX64],
        );
        assert_eq!(
            children
                .iter()
                .map(|child| child.options().target_name.as_deref())
                .collect::<Vec<_>>(),
            [Some("linux_x86_64"), Some("windows_x86_64")],
        );

        let targets = ExplicitTargetSet::from_caller_names(["linux_x64"]).expect("one target set");
        let diagnostics = MultiTargetCompileRequest::from_target_set(targets, |_| {
            let mut child = request("main.omg", "linux-build", RequestedCompileProduct::Check);
            child.options.target_name = Some("windows_x64".to_owned());
            child
        })
        .expect_err("duplicated child target must reject");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("sole target source"));
    }

    #[test]
    fn batch_admission_requires_one_root_and_product_before_source_work() {
        let targets = ExplicitTargetSet::from_caller_names(["linux_x64", "windows_x64"])
            .expect("two target set");
        let batch = MultiTargetCompileRequest::from_target_set(targets, |profile| match profile {
            TargetProfile::LinuxX64 => {
                request("first.omg", "linux-build", RequestedCompileProduct::Check)
            }
            TargetProfile::WindowsX64 => request(
                "second.omg",
                "windows-build",
                RequestedCompileProduct::TerminalArtifact,
            ),
            _ => unreachable!("fixture target set is exact"),
        })
        .expect("target factory itself is valid");
        let diagnostics = batch
            .validate_batch_for_execution()
            .err()
            .expect("mixed roots and products must reject request admission");
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics[0]
                .message
                .contains("different compilation root")
        );
        assert!(
            diagnostics[1]
                .message
                .contains("instead of the batch product")
        );
    }

    #[test]
    fn batch_admission_requires_one_artifact_policy_and_distinct_declared_build_staging() {
        let targets = ExplicitTargetSet::from_caller_names(["linux_x64", "windows_x64"])
            .expect("two target set");
        let batch = MultiTargetCompileRequest::from_target_set(targets, |profile| {
            let child = request("main.omg", "shared-build", RequestedCompileProduct::Check);
            match profile {
                TargetProfile::LinuxX64 => child,
                TargetProfile::WindowsX64 => {
                    child.with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
                }
                _ => unreachable!("fixture target set is exact"),
            }
        })
        .expect("target factory itself is valid");
        let diagnostics = batch
            .validate_batch_for_execution()
            .err()
            .expect("mixed artifact policy and build staging must reject request admission");
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("artifact emission"));
        assert!(diagnostics[1].message.contains("separate staging"));
    }

    #[test]
    fn batch_admission_rejects_package_source_substitution() {
        let targets = ExplicitTargetSet::from_caller_names(["linux_x64", "windows_x64"])
            .expect("two target set");
        let batch = MultiTargetCompileRequest::from_target_set(targets, |profile| {
            request(
                "package/main.omg",
                profile.target_name(),
                RequestedCompileProduct::Check,
            )
            .with_package_inputs(package_inputs(match profile {
                TargetProfile::LinuxX64 => "original-package",
                TargetProfile::WindowsX64 => "substituted-package",
                _ => unreachable!("fixture target set is exact"),
            }))
        })
        .expect("target factory itself is valid");
        let diagnostics = batch
            .validate_batch_for_execution()
            .err()
            .expect("package source substitution must reject request admission");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("package source inputs"));
    }
}
