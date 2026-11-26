//! Reconcile the selected entry's physical operations with the resources that
//! the receiving executor will establish before invoking it.
use crate::AdmittedNativeProviders;
use crate::terminal_authority_policy::private_linux_syscall_authority;
use diagnostics::Diagnostic;
use runtime_abi::{DescriptorOperation, descriptor_system_call};
use semantic_vocabulary::{
    PortableFilesystemAuthorityFacet, TerminalAuthorityClass as Class, TerminalAuthorityDisposition,
};
use target::TargetProfile;
use terminal_authority_receipts::{TerminalAuthorityClosureLeaf, TerminalMechanismIdentity};

#[derive(Debug, Clone)]
pub struct IsolatedExecutionGrant {
    classes: TerminalAuthorityDisposition,
    target: TargetProfile,
    private_filesystem: bool,
}

impl IsolatedExecutionGrant {
    /// The runner captures stdout and stderr and contains process termination.
    pub fn captured_process(
        target: TargetProfile,
        finite_input: bool,
        private_filesystem: bool,
    ) -> Self {
        let mut classes = vec![Class::ProcessOutput, Class::ProcessTermination];
        if finite_input || private_filesystem {
            classes.push(Class::ProcessInput);
        }
        if private_filesystem {
            classes.extend(
                PortableFilesystemAuthorityFacet::ALL
                    .into_iter()
                    .map(Class::from),
            );
        }
        Self {
            classes: TerminalAuthorityDisposition::from_classes(classes),
            target,
            private_filesystem,
        }
    }

    fn check(&self, leaf: &TerminalAuthorityClosureLeaf, subject: &str) -> Result<(), Diagnostic> {
        let scoped = match leaf.mechanism() {
            TerminalMechanismIdentity::Syscall(call)
                if call.target() == self.target && !call.checked_argument_contract().is_zero() =>
            {
                if self.private_filesystem {
                    private_linux_syscall_authority(call)
                } else {
                    // The bounded child inherits only fd 0, 1 and 2. Its admitted closure
                    // has no means to open another descriptor. Psi installs that same table.
                    descriptor_system_call(call.target(), i64::from(call.number())).map(
                        |(_, operation)| {
                            TerminalAuthorityDisposition::from_classes([match operation {
                                DescriptorOperation::Read => Class::ProcessInput,
                                DescriptorOperation::Write => Class::ProcessOutput,
                            }])
                        },
                    )
                }
            }
            _ => None,
        };
        if leaf.exercised().is_none() && scoped.is_none() {
            return Err(Diagnostic::error(format!(
                "isolated execution cannot grant unclassified physical authority to `{subject}`"
            )));
        }
        // Both the installed provider and the receiving descriptor adapter can
        // require authority. Each must fit the resources supplied to this run.
        if let Some(missing) = leaf
            .exercised()
            .into_iter()
            .chain(scoped.as_ref())
            .find_map(|required| self.classes.first_missing_class(required))
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_authority_receipts::{
        CheckedSyscallArgumentContractIdentity, SyscallTerminalMechanismIdentity,
        TerminalAuthorityClosureLeafOrigin,
    };

    #[test]
    fn a_selected_provider_and_its_descriptor_adapter_each_require_their_grant() {
        let target = TargetProfile::LinuxX64;
        let mut identities = terminal_psi::TerminalIdentityTable::default().into_builder();
        let requirement = identities.intern("Bytes::read");
        let call = SyscallTerminalMechanismIdentity::new(
            target,
            0,
            CheckedSyscallArgumentContractIdentity::from_digest([3; 32]),
        );
        let leaf = |class| {
            TerminalAuthorityClosureLeaf::new(
                provider_plans::ServiceSchemaDigest::from_digest([1; 32]),
                requirement,
                requirement,
                provider_plans::ProviderPlanDigest::from_digest([2; 32]),
                call.into(),
                TerminalAuthorityClosureLeafOrigin::SelectedBoundary,
                Some(TerminalAuthorityDisposition::from_classes([class])),
                None,
            )
            .unwrap()
        };
        let output = leaf(Class::ProcessOutput);
        let without_input = IsolatedExecutionGrant::captured_process(target, false, false);
        assert!(
            without_input
                .check(&output, "Bytes::read")
                .unwrap_err()
                .message
                .contains("ProcessInput")
        );
        let with_input = IsolatedExecutionGrant::captured_process(target, true, false);
        with_input.check(&output, "Bytes::read").unwrap();
        let filesystem = leaf(Class::FilesystemContentRead);
        assert!(
            with_input
                .check(&filesystem, "Bytes::read")
                .unwrap_err()
                .message
                .contains("FilesystemContentRead")
        );
    }
}
