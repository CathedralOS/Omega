use omega_bounded_process::BoundedProcessPrepared;

/// One command already admitted by a real external-policy sandbox backend.
///
/// This is a per-invocation capability rather than a caller-implemented trait:
/// its private field makes self-attestation impossible. There is intentionally
/// no production constructor while Omega has no verified OS sandbox backend.
pub(crate) struct VerifiedExternalPolicySandboxInvocation {
    prepared: BoundedProcessPrepared,
}

impl VerifiedExternalPolicySandboxInvocation {
    pub(super) fn into_prepared(self) -> BoundedProcessPrepared {
        self.prepared
    }

    /// Unit tests exercise adapter mechanics without claiming host isolation.
    #[cfg(test)]
    pub(super) fn for_unsandboxed_test_only(prepared: BoundedProcessPrepared) -> Self {
        Self { prepared }
    }
}
