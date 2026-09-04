const ABSOLUTE_RECORD_BYTE_LIMIT: usize = 64 * 1024 * 1024;
const ABSOLUTE_PACKAGE_LIMIT: usize = 16 * 1024;
const ABSOLUTE_DEPENDENCY_REQUEST_LIMIT: usize = 256 * 1024;
const ABSOLUTE_IDENTITY_BYTE_LIMIT: usize = 1024 * 1024;
const ABSOLUTE_REQUEST_BYTE_LIMIT: usize = 1024 * 1024;

/// Resource ceilings for one canonical resolved-source question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubjectLimits {
    pub maximum_record_bytes: usize,
    pub maximum_packages: usize,
    pub maximum_dependency_requests: usize,
    pub maximum_identity_bytes: usize,
    pub maximum_request_bytes: usize,
}

impl Default for CanonicalSourceClosureSubjectLimits {
    fn default() -> Self {
        Self {
            maximum_record_bytes: ABSOLUTE_RECORD_BYTE_LIMIT,
            maximum_packages: 1024,
            maximum_dependency_requests: 16 * 1024,
            maximum_identity_bytes: 64 * 1024,
            maximum_request_bytes: 64 * 1024,
        }
    }
}

impl CanonicalSourceClosureSubjectLimits {
    pub(in super::super) fn compiler_bounded(self) -> Self {
        Self {
            maximum_record_bytes: self.maximum_record_bytes.min(ABSOLUTE_RECORD_BYTE_LIMIT),
            maximum_packages: self.maximum_packages.min(ABSOLUTE_PACKAGE_LIMIT),
            maximum_dependency_requests: self
                .maximum_dependency_requests
                .min(ABSOLUTE_DEPENDENCY_REQUEST_LIMIT),
            maximum_identity_bytes: self
                .maximum_identity_bytes
                .min(ABSOLUTE_IDENTITY_BYTE_LIMIT),
            maximum_request_bytes: self.maximum_request_bytes.min(ABSOLUTE_REQUEST_BYTE_LIMIT),
        }
    }
}
