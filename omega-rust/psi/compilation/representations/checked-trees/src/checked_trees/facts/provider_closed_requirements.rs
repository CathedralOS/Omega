//! The closed boundary requirement each concrete provider refines.
//!
//! A provider conformance names a generic top-level requirement template; the
//! provider's specialization receipt selects the exact closed application of
//! that template it satisfies. Checking resolves that join once over the
//! finalized program, and lowering's provider catalog reads the row instead
//! of re-entering the resolution.

use symbols::SymbolHandle;

/// `provider` satisfies the closed instance `closed_requirement` of the
/// generic requirement `template` its conformance names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedProviderClosedRequirement {
    pub provider: SymbolHandle,
    pub template: SymbolHandle,
    pub closed_requirement: SymbolHandle,
}
