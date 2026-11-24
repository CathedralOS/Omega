//! Reconcile the selected entry's physical operations with the resources that
//! the receiving executor will establish before invoking it.
use crate::AdmittedNativeProviders;
use crate::terminal_authority_policy::private_linux_syscall_authority;
use diagnostics::Diagnostic;
use semantic_vocabulary::{
    PortableFilesystemAuthorityFacet, TerminalAuthorityClass as Class, TerminalAuthorityDisposition,
};
use target::TargetProfile;
use terminal_authority_receipts::{TerminalAuthorityClosureLeaf, TerminalMechanismIdentity};

#[derive(Debug, Clone)]
pub struct IsolatedExecutionGrant {
    classes: TerminalAuthorityDisposition,
    private_filesystem: Option<TargetProfile>,
}

impl IsolatedExecutionGrant {
    /// The runner captures stdout and stderr and contains process termination.
    pub fn captured_process() -> Self {
        Self {
            classes: TerminalAuthorityDisposition::from_classes([
                Class::ProcessOutput,
                Class::ProcessTermination,
            ]),
            private_filesystem: None,
        }
    }

    /// Every accessible path resolves inside this invocation's empty namespace.
    /// The three inherited descriptors are finite input and captured output pipes.
    pub fn private_linux_filesystem(target: TargetProfile) -> Option<Self> {
        matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64).then(|| Self {
            classes: TerminalAuthorityDisposition::from_classes(
                PortableFilesystemAuthorityFacet::ALL
                    .into_iter()
                    .map(Class::from)
                    .chain([
                        Class::ProcessInput,
                        Class::ProcessOutput,
                        Class::ProcessTermination,
                    ]),
            ),
            private_filesystem: Some(target),
        })
    }

    fn check(&self, leaf: &TerminalAuthorityClosureLeaf, subject: &str) -> Result<(), Diagnostic> {
        let scoped = match leaf.mechanism() {
            TerminalMechanismIdentity::Syscall(call)
                if self.private_filesystem == Some(call.target()) =>
            {
                private_linux_syscall_authority(call)
            }
            _ => None,
        };
        let exercised = leaf.exercised().or(scoped.as_ref()).ok_or_else(|| {
            Diagnostic::error(format!(
                "isolated execution cannot grant unclassified physical authority to `{subject}`"
            ))
        })?;
        if let Some(missing) = self.classes.first_missing_class(exercised) {
            return Err(Diagnostic::error(format!(
                "isolated execution has no {missing:?} grant for `{subject}`"
            )));
        }
        if scoped.is_none()
            && !matches!(
                leaf.mechanism(),
                TerminalMechanismIdentity::CompilerIntrinsic(_)
                    | TerminalMechanismIdentity::CheckedPhysical(_)
            )
        {
            return Err(Diagnostic::error(format!(
                "isolated execution has no scoped host adapter for `{subject}`"
            )));
        }
        Ok(())
    }
}

impl AdmittedNativeProviders<'_> {
    pub fn check_isolated_execution_grant(
        &self,
        grant: &IsolatedExecutionGrant,
    ) -> Result<(), Vec<Diagnostic>> {
        if !self.installed_interrupts.scopes().is_empty() {
            return Err(vec![Diagnostic::error(
                "isolated execution has no grant for host interrupt installation",
            )]);
        }
        if self.installation.as_ref().is_some_and(|installation| {
            !installation
                .terminal_selection()
                .assumed_bindings()
                .is_empty()
        }) {
            return Err(vec![Diagnostic::error(
                "isolated execution cannot admit an assumed provider binding",
            )]);
        }
        let review = self.terminal_authority_closure_review();
        for leaf in review.leaves() {
            let subject = review
                .identities()
                .text(leaf.subject_identity())
                .unwrap_or_default();
            grant.check(leaf, subject).map_err(|error| vec![error])?;
        }
        Ok(())
    }
}
