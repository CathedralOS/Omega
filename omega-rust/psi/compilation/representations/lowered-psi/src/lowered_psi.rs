//! Current, unsealed Psi product before selected optimization and publication.

mod source_custody;
pub use source_custody::{
    LoweredSelectedComparisonOccurrence, LoweredSelectedComparisonOperandOrder,
    LoweredSourceCallOccurrence, LoweredSourceCallSite, LoweredSourceFrontierBindings,
    LoweredSourceLoanCapture, LoweredSourceScalarBinding, LoweredSourceStructuralBinding,
    LoweredStateBlock,
};
use terminal_psi::{TerminalDebugMap, TerminalModule};

/// Producer-private source joins used by explicit source-correspondence audits.
/// Publication discards these; native settlement reads canonical Terminal rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceCustody {
    pub source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
}

/// Semantic module and source companions produced by the Psi frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPsi {
    pub semantic_module: TerminalModule,
    /// Explicit application roots kept through optimization. Publication need
    /// not encode this producer demand: entry receipts name the exact retained
    /// declarations in the completed artifact, and confer no runtime authority.
    pub retained_roots: Vec<semantic_vocabulary::MachineId>,
    /// Replaceable presentation metadata. The public producer always fills
    /// this after semantic identities are final; private builders leave it
    /// empty until that finalization step.
    pub debug_map: Option<TerminalDebugMap>,
    /// Ephemeral checked-source-to-Terminal call joins. These rows are not
    /// encoded into Terminal Psi. Psi projects portable callback applications
    /// before publication and retains these joins only for optimization and
    /// explicit source-correspondence audits.
    pub source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    /// Ephemeral exact joins from selected checked comparison uses to the
    /// Terminal operations they produced, including where each emitted
    /// operation reads the authored operands. Operation crash contracts and
    /// boundary applications are joined through these rows before the source
    /// companions are gone; publication discards them; native settlement
    /// reads canonical demands.
    pub selected_comparison_occurrences: Vec<LoweredSelectedComparisonOccurrence>,
    /// Ephemeral joins from each emitted state graph state to its header
    /// block. Proof publication reads them; Omega does not.
    pub state_blocks: Vec<LoweredStateBlock>,
}

impl LoweredPsi {
    /// Separate the portable program from presentation and producer-private
    /// source joins. Publication discards the latter; an explicit audit may use them.
    pub fn into_parts(
        self,
    ) -> (
        TerminalModule,
        Option<TerminalDebugMap>,
        LoweredSourceCustody,
    ) {
        (
            self.semantic_module,
            self.debug_map,
            LoweredSourceCustody {
                source_call_occurrences: self.source_call_occurrences,
            },
        )
    }
}
