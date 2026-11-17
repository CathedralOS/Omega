//! The published program and the settled inputs for executable planning.
//!
//! This handoff contains no source tree, source publication job, provider
//! inventory or accepted Build source. Inspection and package review retain
//! their source through CheckedCompilation; executable planning cannot reopen it.

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PublishedCompilation {
    pub(crate) root_grants: Arc<[native_program_planning::trust::TrustAdmission]>,
    pub(crate) execution: native_program_planning::SelectedExecution,
    pub(crate) package_inputs: crate::ExecutablePackageInputs,
    pub(crate) program: Arc<crate::PublishedProgram>,
    pub(crate) application: target::NativeApplicationMetadata,
    pub(crate) behavior_exclusions: Vec<effects::AuthoredBehaviorExclusion>,
    pub(crate) x86_deployment_claim:
        package_compilation::build_execution::build_value::X86DeploymentClaim,
}

impl PublishedCompilation {
    /// The obligations admitted for this target's exact selected providers.
    pub fn root_grants(&self) -> &[native_program_planning::trust::TrustAdmission] {
        &self.root_grants
    }

    pub fn execution(&self) -> &native_program_planning::SelectedExecution {
        &self.execution
    }

    pub fn package_inputs(&self) -> &crate::ExecutablePackageInputs {
        &self.package_inputs
    }

    pub fn program(&self) -> &Arc<crate::PublishedProgram> {
        &self.program
    }

    pub fn native_application_metadata(&self) -> target::NativeApplicationMetadata {
        self.application.clone()
    }

    pub fn behavior_exclusions(&self) -> &[effects::AuthoredBehaviorExclusion] {
        &self.behavior_exclusions
    }

    pub fn x86_deployment_claim(
        &self,
    ) -> package_compilation::build_execution::build_value::X86DeploymentClaim {
        self.x86_deployment_claim
    }
}
