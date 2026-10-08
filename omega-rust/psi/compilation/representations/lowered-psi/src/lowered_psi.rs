//! Current, unsealed Psi product before selected optimization and publication.

mod source_custody;
pub use source_custody::{
    CallbackTerminalLoweringReceipt, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSelectedIntegerComparisonOperandOrder,
    LoweredSelectedIntegerComparisonOperation, LoweredSourceCallOccurrence, LoweredSourceCallSite,
    LoweredStateBlock,
};
use terminal_psi::{TerminalDebugMap, TerminalModule};

/// Source joins that still accompany publication for native settlement. These
/// contain no semantic program storage and are not a portable product.
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
    /// encoded into Terminal Psi; the Omega product consumes them while both
    /// representations are available and retains only target-owned evidence.
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
    /// Transfer the semantic program to publication, retaining only source
    /// joins for consumers whose settlement still depends on checked input.
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
