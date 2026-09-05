//! Current, unsealed Psi product before selected optimization and publication.

mod source_custody;
pub use source_custody::*;
use terminal_psi::{ProofBundle, TerminalDebugMap, TerminalModule};

/// Semantic module and separate replaceable proof artifact produced by the
/// Psi frontend producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPsi {
    pub semantic_module: TerminalModule,
    pub proof_bundle: ProofBundle,
    /// Replaceable presentation metadata. The public producer always fills
    /// this after semantic identities are final; private builders leave it
    /// empty until that finalization step.
    pub debug_map: Option<TerminalDebugMap>,
    /// Ephemeral checked-source-to-Terminal call joins. These rows are not
    /// encoded into Terminal Psi; the Omega product consumes them while both
    /// representations are available and retains only target-owned evidence.
    pub source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    /// Ephemeral exact joins from selected checked IEEE FMA uses to the
    /// target-neutral Terminal operations they produced. Source and selected-
    /// plan handles remain outside the canonical Terminal artifact; Omega must
    /// consume these rows while both representations are alive.
    pub selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
}
