//! The closed boundary requirement each concrete provider refines.
//!
//! A provider conformance names an authored machine or trait requirement; the
//! provider's specialization receipt selects the exact closed application of
//! that declaration it satisfies. Checking resolves that join once over the
//! finalized program, and lowering's provider catalog reads the row instead
//! of re-entering the resolution. Named maps retain their selecting declaration:
//! sharing a realization body does not let one map borrow another map's checked
//! requirement application.

use symbols::SymbolHandle;

/// `provider` satisfies the closed instance `closed_requirement` of the
/// generic requirement `template` its conformance names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedProviderClosedRequirement {
    pub provider: SymbolHandle,
    /// Named implementation, or the machine owning a direct satisfaction edge.
    /// This is selection custody, separate from the executable provider body.
    pub selecting_declaration: SymbolHandle,
    pub template: SymbolHandle,
    pub closed_requirement: SymbolHandle,
}
