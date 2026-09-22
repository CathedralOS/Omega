//! Optimizer module role: stage group. legalized operations control flow.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod saturating;
pub use saturating::{SaturatingCarrier, SaturatingOperation};
mod scalar_graph;
pub use scalar_graph::{
    LegalizedDynamicParameterCall, LegalizedNormalizedForeignCall, LegalizedScalarArgument,
    LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedStructuralCaseSource, LegalizedValueDefinition,
};
mod structural_case;
pub use structural_case::{LegalizedStructuralCasePayload, LegalizedStructuralCaseSuccessor};
