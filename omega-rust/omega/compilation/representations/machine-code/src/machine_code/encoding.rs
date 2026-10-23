//! Current layout-independent instruction bytes, call templates and evidence.

mod identity;
mod rows;
#[cfg(test)]
mod tests;
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};
pub use rows::{
    DeferredControlEncodingReason, ResolvedPhysicalAddress, SelectedFormDecodedFootprint,
    SelectedFormDecodedFootprintHandle, SelectedFormEncodedByteSpan, SelectedFormEncodedRowStore,
    SelectedFormEncodingCounts, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};

/// Current encoded data and its exact content joins. Construction, cloning and
/// identity recomputation grant no admission, layout or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncoding {
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: PostAllocationMachineIdentity,
    pub post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    pub identity: SelectedFormEncodingIdentity,
    pub rows: Vec<SelectedFormEncodingRow>,
    /// The one byte image and footprint table every row above names. Rows
    /// hold spans and handles into it, never their own allocations.
    pub encoded: SelectedFormEncodedRowStore,
    /// Raw frame geometry is explicit replay input, never a frame admission token.
    pub frame: Option<crate::TargetFrameLayoutPlan>,
    pub counts: SelectedFormEncodingCounts,
}

impl SelectedFormEncoding {
    pub const fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.post_allocation_machine_optimization
    }

    pub const fn identity(&self) -> SelectedFormEncodingIdentity {
        self.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.rows
    }

    pub const fn encoded(&self) -> &SelectedFormEncodedRowStore {
        &self.encoded
    }

    pub const fn counts(&self) -> SelectedFormEncodingCounts {
        self.counts
    }

    /// Recompute content identity without granting encoding or publication authority.
    pub fn recomputed_identity(&self) -> SelectedFormEncodingIdentity {
        identity::encoding_identity(
            self.selected,
            self.machine,
            self.post_allocation_machine_optimization,
            &self.rows,
            &self.encoded,
            self.frame.as_ref(),
            self.counts,
        )
    }
}

/// An identity names retained bytes and rows. It does not grant encoding,
/// optimization, layout, or publication authority.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormEncodingIdentity([u8; 32]);

impl SelectedFormEncodingIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
