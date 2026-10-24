/// Aggregate ceilings for the complete lock, never reset for a target or child.
/// Input remains borrowed; owned storage counts requested recovery allocations
/// and verification scratch, not allocator overhead or already owned input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PackageLockRecoveryLimits {
    pub maximum_bytes: usize,
    pub maximum_owned_bytes: usize,
    pub maximum_targets: usize,
    pub maximum_packages: usize,
    pub maximum_dependency_requests: usize,
    pub maximum_policy_elements: usize,
    pub maximum_identity_nodes: usize,
    pub maximum_decisions: usize,
}

impl Default for PackageLockRecoveryLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: 128 * 1024 * 1024,
            maximum_owned_bytes: 256 * 1024 * 1024,
            maximum_targets: 32,
            maximum_packages: 16 * 1024,
            maximum_dependency_requests: 256 * 1024,
            maximum_policy_elements: 1024 * 1024,
            // Every identity node consumes lock text, at most two productions
            // per byte (a nominal name opens one scope for its atom and one for
            // its owner), so the node ceiling follows the byte ceiling. A
            // 17-package application's membership exceeded a fixed 1,048,576.
            maximum_identity_nodes: 2 * 128 * 1024 * 1024,
            maximum_decisions: 65_536,
        }
    }
}

impl PackageLockRecoveryLimits {
    pub(super) fn source_limits(
        self,
    ) -> crate::resolution::graph::CanonicalSourceClosureSubjectLimits {
        crate::resolution::graph::CanonicalSourceClosureSubjectLimits {
            maximum_record_bytes: 64 * 1024 * 1024,
            maximum_packages: self.maximum_packages,
            maximum_dependency_requests: self.maximum_dependency_requests,
            maximum_identity_bytes: 1024 * 1024,
            maximum_request_bytes: 1024 * 1024,
        }
    }

    crate::bounded_limits!(
        pub(super) fn;
        maximum_bytes,
        maximum_owned_bytes,
        maximum_targets,
        maximum_packages,
        maximum_dependency_requests,
        maximum_policy_elements,
        maximum_identity_nodes,
        maximum_decisions,
    );
}
