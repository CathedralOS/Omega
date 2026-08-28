use psi_diagnostics::Diagnostic;

use super::checked_entry::compile_to_checked_for_terminal;
use super::{
    CheckedCompilation, CompileOptions, CompileReport, PackageCompilationInputs,
    TerminalComponentDeploymentInputOwner, TerminalComponentDriverError,
    TerminalComponentProviderSettlement, TerminalComponentStagingInputs,
    stage_acquire_and_deploy_terminal_component_output,
};

/// One typed ordinary-frontend handoff to an external terminal deployment
/// owner.
///
/// The request deliberately contains no installation factory. Its admission
/// profile and provider settlements remain borrowed from their external
/// owners, while `deployment_owner` is the sole source of installation,
/// provider-occurrence, progress, and profile-decision authority.
#[derive(Debug)]
#[must_use = "terminal compile requests retain external deployment authority"]
pub struct TerminalComponentCompileRequest<'evidence, Owner> {
    options: CompileOptions,
    package_inputs: Option<PackageCompilationInputs>,
    profile: &'evidence psi_proof_admission::AdmissionProfile,
    settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
    deployment_owner: Owner,
}

impl<'evidence, Owner> TerminalComponentCompileRequest<'evidence, Owner> {
    pub fn new(
        options: CompileOptions,
        profile: &'evidence psi_proof_admission::AdmissionProfile,
        settlements: Vec<TerminalComponentProviderSettlement<'evidence>>,
        deployment_owner: Owner,
    ) -> Self {
        Self {
            options,
            package_inputs: None,
            profile,
            settlements,
            deployment_owner,
        }
    }

    pub fn with_package_inputs(mut self, package_inputs: PackageCompilationInputs) -> Self {
        self.package_inputs = Some(package_inputs);
        self
    }

    pub const fn options(&self) -> &CompileOptions {
        &self.options
    }

    pub const fn package_inputs(&self) -> Option<&PackageCompilationInputs> {
        self.package_inputs.as_ref()
    }

    pub const fn profile(&self) -> &psi_proof_admission::AdmissionProfile {
        self.profile
    }

    pub fn settlements(&self) -> &[TerminalComponentProviderSettlement<'evidence>] {
        &self.settlements
    }

    pub const fn deployment_owner(&self) -> &Owner {
        &self.deployment_owner
    }

    fn bind_checked(
        self,
        checked: CheckedCompilation,
    ) -> Result<
        BoundTerminalComponentCompileRequest<'evidence, Owner>,
        TerminalComponentCompileRequestBindingError<'evidence, Owner>,
    > {
        let Self {
            options,
            package_inputs,
            profile,
            settlements,
            deployment_owner,
        } = self;
        match TerminalComponentStagingInputs::from_checked(&checked, profile, settlements) {
            Ok(staging_inputs) => Ok(BoundTerminalComponentCompileRequest {
                options,
                package_inputs,
                checked,
                staging_inputs,
                deployment_owner,
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let (_, profile, settlements) = error.into_parts();
                Err(TerminalComponentCompileRequestBindingError {
                    diagnostic,
                    checked,
                    request: Self {
                        options,
                        package_inputs,
                        profile,
                        settlements,
                        deployment_owner,
                    },
                })
            }
        }
    }
}

/// One request whose checked target/subsystem binding has completed without
/// consuming or rearranging any external staging or deployment custody.
struct BoundTerminalComponentCompileRequest<'evidence, Owner> {
    options: CompileOptions,
    package_inputs: Option<PackageCompilationInputs>,
    checked: CheckedCompilation,
    staging_inputs: TerminalComponentStagingInputs<'evidence>,
    deployment_owner: Owner,
}

/// A failed request-owned checked binding. The exact checked result and the
/// complete original request remain paired for inspection or corrected retry.
struct TerminalComponentCompileRequestBindingError<'evidence, Owner> {
    diagnostic: Diagnostic,
    checked: CheckedCompilation,
    request: TerminalComponentCompileRequest<'evidence, Owner>,
}

/// A rejected typed terminal compile handoff.
///
/// Frontend and staging-input binding rejection retain the complete original
/// request. Binding rejection additionally retains the checked result so a
/// caller can inspect or reuse the exact frontend owner without rerunning it.
/// Once staging begins, the established driver error retains staging or linear
/// deployment custody; options and package inputs remain beside it.
#[derive(Debug)]
pub enum TerminalComponentCompileError<'evidence, Owner>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    Frontend {
        diagnostics: Vec<Diagnostic>,
        request: TerminalComponentCompileRequest<'evidence, Owner>,
    },
    StagingInputBinding {
        diagnostic: Diagnostic,
        checked: CheckedCompilation,
        request: TerminalComponentCompileRequest<'evidence, Owner>,
    },
    Driver {
        error: Box<TerminalComponentDriverError<'evidence, Owner>>,
        options: CompileOptions,
        package_inputs: Option<PackageCompilationInputs>,
    },
}

impl<Owner> std::fmt::Display for TerminalComponentCompileError<'_, Owner>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frontend { diagnostics, .. } => diagnostics
                .first()
                .map_or("terminal component frontend failed", |diagnostic| {
                    diagnostic.message.as_str()
                })
                .fmt(formatter),
            Self::StagingInputBinding { diagnostic, .. } => diagnostic.fmt(formatter),
            Self::Driver { error, .. } => error.fmt(formatter),
        }
    }
}

impl<Owner> std::error::Error for TerminalComponentCompileError<'_, Owner> where
    Owner: TerminalComponentDeploymentInputOwner
{
}

/// Run the ordinary checked frontend and hand its exact result to the existing
/// terminal staging/deployment driver.
///
/// This operation does not enter the legacy checked-tree backend or
/// `write_output`. It cannot complete production installation until its caller
/// supplies a concrete non-test [`TerminalComponentDeploymentInputOwner`].
pub fn compile_terminal_component_output<'evidence, Owner>(
    request: TerminalComponentCompileRequest<'evidence, Owner>,
) -> Result<CompileReport, Box<TerminalComponentCompileError<'evidence, Owner>>>
where
    Owner: TerminalComponentDeploymentInputOwner,
{
    let checked = match compile_to_checked_for_terminal(request.options(), request.package_inputs())
    {
        Ok(checked) => checked,
        Err(diagnostics) => {
            return Err(Box::new(TerminalComponentCompileError::Frontend {
                diagnostics,
                request,
            }));
        }
    };
    let BoundTerminalComponentCompileRequest {
        options,
        package_inputs,
        checked,
        staging_inputs,
        deployment_owner,
    } = match request.bind_checked(checked) {
        Ok(bound) => bound,
        Err(error) => {
            return Err(Box::new(
                TerminalComponentCompileError::StagingInputBinding {
                    diagnostic: error.diagnostic,
                    checked: error.checked,
                    request: error.request,
                },
            ));
        }
    };
    let source_file_count = checked.source_file_count();
    match stage_acquire_and_deploy_terminal_component_output(
        &options,
        source_file_count,
        &checked,
        staging_inputs,
        deployment_owner,
    ) {
        Ok(report) => Ok(report),
        Err(error) => Err(Box::new(TerminalComponentCompileError::Driver {
            error,
            options,
            package_inputs,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_installation_evidence::TerminalProviderExecutionEvidence;
    use omega_terminal_target_operations::{
        TerminalBoundaryRealization, TerminalLinuxExitGroupI32Realization,
    };

    #[derive(Debug)]
    struct TestProviderExecution;

    impl TerminalProviderExecutionEvidence for TestProviderExecution {
        fn requirement_identity(&self) -> &str {
            "Test::exit"
        }

        fn provider_plan(&self) -> u64 {
            11
        }

        fn provider_execution_identity(&self) -> u64 {
            12
        }

        fn provider_execution_fingerprint(&self) -> u64 {
            13
        }

        fn normalized_root_identity(&self) -> u64 {
            14
        }

        fn boundary_contract_fingerprint(&self) -> u64 {
            15
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TestDeploymentOwner(u64);

    fn canary_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../tests/canaries/pass/ownership/linear_transfer_and_consume/main.omg",
        )
    }

    fn options(root_path: std::path::PathBuf, target_name: Option<&str>) -> CompileOptions {
        CompileOptions {
            root_path,
            build_dir: Some("exact-terminal-build".into()),
            target_name: target_name.map(str::to_owned),
            write_output: false,
        }
    }

    fn settlement(execution: &TestProviderExecution) -> TerminalComponentProviderSettlement<'_> {
        TerminalComponentProviderSettlement {
            provider_execution: execution,
            realization: TerminalBoundaryRealization::LinuxExitGroupI32(
                TerminalLinuxExitGroupI32Realization,
            ),
        }
    }

    fn package_inputs(source_root: &std::path::Path) -> PackageCompilationInputs {
        let identity = psi_core::PackageKeyIdentity::from_digest([0x71; 32])
            .expect("nonzero package identity");
        PackageCompilationInputs::new(
            identity,
            vec![omega_package_compilation::PackageSourceBinding::new(
                identity,
                "terminal-owner-fixture",
                source_root.to_path_buf(),
            )],
            Vec::new(),
        )
        .expect("single-package fixture is a closed reconciled graph")
    }

    #[test]
    fn targetless_binding_preserves_the_exact_checked_result_and_request_custody() {
        let root = canary_root();
        let checked = crate::compile_to_checked(&root, None)
            .expect("targetless ownership canary should check");
        let expected_checked = checked.clone();
        let profile = psi_proof_admission::AdmissionProfile::default();
        let execution = TestProviderExecution;
        let expected_package_inputs = package_inputs(
            root.parent()
                .expect("ownership canary has a concrete source root"),
        );
        let expected_options = options(root, None);
        let request = TerminalComponentCompileRequest::new(
            expected_options.clone(),
            &profile,
            vec![settlement(&execution)],
            TestDeploymentOwner(91),
        )
        .with_package_inputs(expected_package_inputs.clone());

        let error = match request.bind_checked(checked) {
            Ok(_) => panic!("targetless checked result must reject staging-input binding"),
            Err(error) => error,
        };

        assert_eq!(
            error.diagnostic.message,
            "terminal component staging requires an exact native target selected by the owning checked result"
        );
        assert_eq!(error.checked, expected_checked);
        assert_eq!(error.request.options(), &expected_options);
        assert_eq!(
            error.request.package_inputs(),
            Some(&expected_package_inputs)
        );
        assert!(std::ptr::eq(error.request.profile(), &profile));
        assert_eq!(error.request.settlements().len(), 1);
        assert!(std::ptr::eq(
            error.request.settlements()[0].provider_execution,
            &execution as &dyn TerminalProviderExecutionEvidence,
        ));
        assert_eq!(
            error.request.settlements()[0].realization,
            TerminalBoundaryRealization::LinuxExitGroupI32(TerminalLinuxExitGroupI32Realization,)
        );
        assert_eq!(error.request.deployment_owner(), &TestDeploymentOwner(91));
    }

    #[test]
    fn selected_target_binding_conserves_the_complete_request_in_one_bound_owner() {
        let root = canary_root();
        let checked = crate::compile_to_checked(&root, Some("macos_arm64"))
            .expect("selected-target ownership canary should check");
        let expected_checked = checked.clone();
        let expected_target = checked
            .selected_native_target()
            .expect("selected-target checked result retains native target");
        let expected_subsystem = checked.subsystem();
        let profile = psi_proof_admission::AdmissionProfile::default();
        let execution = TestProviderExecution;
        let expected_package_inputs = package_inputs(
            root.parent()
                .expect("ownership canary has a concrete source root"),
        );
        let expected_options = options(root, Some("macos_arm64"));
        let request = TerminalComponentCompileRequest::new(
            expected_options.clone(),
            &profile,
            vec![settlement(&execution)],
            TestDeploymentOwner(92),
        )
        .with_package_inputs(expected_package_inputs.clone());

        let bound = match request.bind_checked(checked) {
            Ok(bound) => bound,
            Err(_) => panic!("selected-target checked result must bind staging inputs"),
        };

        assert_eq!(bound.options, expected_options);
        assert_eq!(bound.package_inputs, Some(expected_package_inputs));
        assert_eq!(bound.checked, expected_checked);
        assert_eq!(bound.staging_inputs.target(), expected_target);
        assert_eq!(bound.staging_inputs.subsystem(), expected_subsystem);
        assert!(std::ptr::eq(bound.staging_inputs.profile(), &profile));
        assert_eq!(bound.staging_inputs.settlements().len(), 1);
        assert!(std::ptr::eq(
            bound.staging_inputs.settlements()[0].provider_execution,
            &execution as &dyn TerminalProviderExecutionEvidence,
        ));
        assert_eq!(
            bound.staging_inputs.settlements()[0].realization,
            TerminalBoundaryRealization::LinuxExitGroupI32(TerminalLinuxExitGroupI32Realization,)
        );
        assert_eq!(bound.deployment_owner, TestDeploymentOwner(92));
    }
}
