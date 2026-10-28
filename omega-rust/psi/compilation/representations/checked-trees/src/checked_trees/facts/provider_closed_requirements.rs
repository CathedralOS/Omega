//! The closed boundary requirement each concrete provider refines.
//!
//! A provider conformance names an authored machine or trait requirement; the
//! provider's specialization receipt selects the exact closed application of
//! that declaration it satisfies. Checking resolves that join once over the
//! finalized program, and lowering's provider catalog reads the row instead
//! of re-entering the resolution. Named maps retain their selecting declaration:
//! sharing a realization body does not let one map borrow another map's checked
//! requirement application. The receiving trait application and ordered public
//! region arguments travel with that join; neither equal runtime signatures
//! nor a later search may reconstruct them. Recording the association does not
//! widen provider execution admission.

use arena::{Arena, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::type_identity::{LifetimeParameterIdentity, NormalizedTypeIdentity};

/// `provider` satisfies the closed instance `closed_requirement` of the
/// generic requirement `template` its conformance names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderClosedRequirement {
    pub provider: SymbolHandle,
    /// Named implementation, or the machine owning a direct satisfaction edge.
    /// This is selection custody, separate from the executable provider body.
    pub selecting_declaration: SymbolHandle,
    pub template: SymbolHandle,
    pub closed_requirement: SymbolHandle,
    /// Complete receiving trait application already retained by the typed
    /// requirement instance. Zero denotes a top-level requirement with no
    /// trait application, never an application wildcard.
    pub receiving_application: NormalizedTypeIdentity,
    /// Receiving coordinates in the selecting declaration's public region
    /// binder order. Callable invocation binders and loans are separate axes.
    pub receiving_regions: HandleSpan<LifetimeParameterIdentity>,
}

/// One checked owner for requirement rows and their durable region arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderClosedRequirements {
    rows: Arena<CheckedProviderClosedRequirement>,
    regions: Arena<LifetimeParameterIdentity>,
}

impl CheckedProviderClosedRequirements {
    pub fn insert(
        &mut self,
        mut row: CheckedProviderClosedRequirement,
        receiving_regions: impl IntoIterator<Item = LifetimeParameterIdentity>,
    ) {
        row.receiving_regions = self.regions.insert_many(receiving_regions);
        self.rows.insert(row);
    }

    pub fn iter(&self) -> impl Iterator<Item = &CheckedProviderClosedRequirement> {
        self.rows.iter().map(|(_, row)| row)
    }

    pub fn receiving_regions(
        &self,
        row: &CheckedProviderClosedRequirement,
    ) -> Option<&[LifetimeParameterIdentity]> {
        self.regions.span(row.receiving_regions)
    }

    /// Product pruning removes associations without moving surviving spans.
    pub fn retain(&mut self, mut retain: impl FnMut(&CheckedProviderClosedRequirement) -> bool) {
        let removed = self
            .rows
            .iter()
            .filter_map(|(handle, row)| (!retain(row)).then_some(handle))
            .collect::<Vec<_>>();
        for handle in removed {
            self.rows.free(handle);
        }
    }
}
