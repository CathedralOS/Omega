//! Resource ceilings for exact review-row comparison.

/// Canonical row bytes are compiler-bounded, but comparison clones changed
/// rows into orchestration state. These ceilings prevent a hostile graph from
/// multiplying that memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewOnlyCapabilityConflictLimits {
    pub(super) maximum_packages: usize,
    pub(super) maximum_rows: usize,
    pub(super) maximum_row_key_bytes: usize,
    pub(super) maximum_encoded_row_bytes: usize,
    pub(super) maximum_source_locations: usize,
    pub(super) maximum_source_location_path_bytes: usize,
    pub(super) maximum_conflicts: usize,
    pub(super) maximum_changed_row_bytes: usize,
    pub(super) maximum_changed_source_location_bytes: usize,
    pub(super) maximum_dependency_path_steps: usize,
}

impl ReviewOnlyCapabilityConflictLimits {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        maximum_packages: usize,
        maximum_rows: usize,
        maximum_row_key_bytes: usize,
        maximum_encoded_row_bytes: usize,
        maximum_source_locations: usize,
        maximum_source_location_path_bytes: usize,
        maximum_conflicts: usize,
        maximum_changed_row_bytes: usize,
        maximum_changed_source_location_bytes: usize,
        maximum_dependency_path_steps: usize,
    ) -> Self {
        Self {
            maximum_packages,
            maximum_rows,
            maximum_row_key_bytes,
            maximum_encoded_row_bytes,
            maximum_source_locations,
            maximum_source_location_path_bytes,
            maximum_conflicts,
            maximum_changed_row_bytes,
            maximum_changed_source_location_bytes,
            maximum_dependency_path_steps,
        }
    }

    pub const fn maximum_packages(self) -> usize {
        self.maximum_packages
    }
    pub const fn maximum_conflicts(self) -> usize {
        self.maximum_conflicts
    }
    pub const fn maximum_changed_row_bytes(self) -> usize {
        self.maximum_changed_row_bytes
    }
    pub const fn maximum_rows(self) -> usize {
        self.maximum_rows
    }
    pub const fn maximum_row_key_bytes(self) -> usize {
        self.maximum_row_key_bytes
    }
    pub const fn maximum_encoded_row_bytes(self) -> usize {
        self.maximum_encoded_row_bytes
    }
    pub const fn maximum_source_locations(self) -> usize {
        self.maximum_source_locations
    }
    pub const fn maximum_source_location_path_bytes(self) -> usize {
        self.maximum_source_location_path_bytes
    }
    pub const fn maximum_changed_source_location_bytes(self) -> usize {
        self.maximum_changed_source_location_bytes
    }
    pub const fn maximum_dependency_path_steps(self) -> usize {
        self.maximum_dependency_path_steps
    }
}

impl Default for ReviewOnlyCapabilityConflictLimits {
    fn default() -> Self {
        Self::new(
            4_096,
            131_072,
            16 * 1024 * 1024,
            32 * 1024 * 1024,
            262_144,
            16 * 1024 * 1024,
            65_536,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            1_024,
        )
    }
}
