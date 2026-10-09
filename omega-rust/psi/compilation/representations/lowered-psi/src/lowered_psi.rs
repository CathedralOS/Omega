//! Current, unsealed Psi product before selected optimization and publication.

mod source_custody;
pub use source_custody::{
    CallbackTerminalLoweringReceipt, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSelectedIntegerComparisonOperandOrder,
    LoweredSelectedIntegerComparisonOperation, LoweredSourceCallOccurrence, LoweredSourceCallSite,
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
    pub selected_ieee_float_comparison_occurrences:
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub semantic_module: TerminalModule,
    /// Replaceable presentation metadata. The public producer always fills
    /// this after semantic identities are final; private builders leave it
    /// empty until that finalization step.
    pub debug_map: Option<TerminalDebugMap>,
    /// Ephemeral checked-source-to-Terminal call joins. These rows are not
    /// encoded into Terminal Psi. Psi projects portable callback applications
    /// before publication and retains these joins only for optimization and
    /// explicit source-correspondence audits.
    pub source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    /// Ephemeral exact joins from selected checked integer comparison uses to
    /// the Terminal operations they produced, the integer counterpart of
    /// `selected_ieee_float_comparison_occurrences`. Operation crash contracts
    /// are joined through these rows before the source companions are gone;
    /// Publication discards them; native settlement reads canonical demands.
    pub selected_integer_comparison_occurrences: Vec<LoweredSelectedIntegerComparisonOccurrence>,
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
