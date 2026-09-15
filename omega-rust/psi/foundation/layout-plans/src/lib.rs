#![forbid(unsafe_code)]

//! Normalized programmable-layout plans and symbolic materialization.
//!
//! Layout policies describe geometry. A materializer consumes validated
//! geometry plus compiler-issued symbolic values; source programs never
//! receive numeric code addresses or an arbitrary byte-patching primitive.
//!
//! Start at [`layout_reports`] for validated geometry, [`symbolic_values`]
//! for compiler-issued identities, and [`symbolic_materialization`] for the
//! plan derivation that joins them under [`placement`] constraints.

mod field_values;
mod layout_reports;
mod materialization;
mod materialization_field_identities;
mod placement;
mod post_handoff_writer;
mod stored_integer_writes;
mod symbolic_materialization;
mod symbolic_values;

pub use field_values::{
    AggregateFieldSchema, AggregateFieldValue, ScalarFieldSchema, ScalarFieldValue,
};
pub use layout_reports::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalNestedRecordSumOccurrenceLayoutReport,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalNestedRecordSumPathsLayoutReport,
    ConventionalRecordSumOccurrenceLayoutReport, ConventionalRecordSumPathsLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumCaseLayoutReport, ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, IntegerInterpretation, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, NativeLayoutPlanReport,
    PrivateCallbackLayoutDemandReport, conventional_sum_layout_reports_match_for_replay,
    layout_plan_reports_match_for_replay, normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint, normalized_native_layout_plan_report_fingerprint,
};
pub use materialization::{
    MaterializationDiagnostic, SymbolicMaterializationPlan, decode_scalar_layout,
    materialize_aggregate_layout_into, materialize_scalar_layout_into,
};
pub use placement::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, MachineRegimeId,
    MaterializationAction, MaterializationContext, MaterializationWrite, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, PlacementSite, StoredIntegerFit,
};
pub use post_handoff_writer::{
    GeneratedPostHandoffWriterFragmentPlan, GeneratedPostHandoffWriterStep,
    POST_HANDOFF_WRITER_CONTEXT_ABI_V1, POST_HANDOFF_WRITER_DESTINATION_OFFSET,
    POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH, POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET,
    PostHandoffWriterFitConstraint, PostHandoffWriterInvocationPlan, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterSourceSlot, PostHandoffWriterStep,
    post_handoff_writer_context_byte_len,
};
pub use symbolic_materialization::{
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
};
pub use symbolic_values::{
    DataSymbolId, EntryStubId, RelocationTarget, SymbolicFieldInnerLayout,
    SymbolicFieldInteriorLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
};

#[cfg(test)]
mod tests;
