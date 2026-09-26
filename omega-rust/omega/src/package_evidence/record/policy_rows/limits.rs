/// One projection's aggregate limits; callers subtract usage across packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyRowLimits {
    pub maximum_rows: usize,
    pub maximum_owned_bytes: usize,
    pub maximum_sequence_elements: usize,
    pub maximum_depth: usize,
    pub maximum_key_bytes: usize,
    pub maximum_canonical_bytes: usize,
    pub maximum_text_bytes: usize,
}

impl Default for PackagePolicyRowLimits {
    fn default() -> Self {
        Self {
            maximum_rows: 65_536,
            maximum_owned_bytes: 128 * 1024 * 1024,
            maximum_sequence_elements: 1024 * 1024,
            maximum_depth: 128,
            maximum_key_bytes: 1024 * 1024,
            maximum_canonical_bytes: 4 * 1024 * 1024,
            maximum_text_bytes: 32 * 1024 * 1024,
        }
    }
}

impl PackagePolicyRowLimits {
    pub(crate) fn bounded(self) -> Self {
        let hard = Self::default();
        Self {
            maximum_rows: self.maximum_rows.min(hard.maximum_rows),
            maximum_owned_bytes: self.maximum_owned_bytes.min(hard.maximum_owned_bytes),
            maximum_sequence_elements: self
                .maximum_sequence_elements
                .min(hard.maximum_sequence_elements),
            maximum_depth: self.maximum_depth.min(hard.maximum_depth),
            maximum_key_bytes: self.maximum_key_bytes.min(hard.maximum_key_bytes),
            maximum_canonical_bytes: self
                .maximum_canonical_bytes
                .min(hard.maximum_canonical_bytes),
            maximum_text_bytes: self.maximum_text_bytes.min(hard.maximum_text_bytes),
        }
    }
}

/// Requested storage includes the exact row table and all retained buffers.
/// Elements include both allocation-free sizing and actual emission traversals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackagePolicyRowUsage {
    pub(crate) rows: usize,
    pub(crate) owned_bytes: usize,
    pub(crate) sequence_elements: usize,
}

impl PackagePolicyRowUsage {
    pub const fn rows(self) -> usize {
        self.rows
    }
    pub const fn owned_bytes(self) -> usize {
        self.owned_bytes
    }
    pub const fn sequence_elements(self) -> usize {
        self.sequence_elements
    }
}
