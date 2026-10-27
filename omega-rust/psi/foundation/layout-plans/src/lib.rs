#![forbid(unsafe_code)]

//! Normalized programmable-layout plans and symbolic materialization.
//!
//! Layout policies describe geometry. A materializer consumes validated
//! geometry plus compiler-issued symbolic values; source programs never
//! receive numeric code addresses or an arbitrary byte-patching primitive.
//!
//! Start at `symbolic_materialization.rs`: the plan derivation that joins
//! validated geometry from `layout_reports` with compiler-issued identities
//! from `symbolic_values` under `placement` constraints. `materialization`
//! applies the derived plans and owns the field values, field identities and
//! stored-integer write predicates they consume. These are portable
//! descriptions and semantic validity only: Omega consumers derive, lower,
//! validate, and execute their own writer programs from the same vocabulary,
//! and the interpreter consumes the portable operations through its own path.

/// Generates one `*_numbered` constructor from its positional sibling. The
/// positional constructor always runs first, complete with its own
/// validation, so stamping a compiler-retained stable member identity can
/// never produce a value the sibling would have refused. The spelling stays
/// diagnostic presentation; the identity is what selects placements.
macro_rules! numbered_constructor {
    ($(#[$attribute:meta])* $name:ident(field $(, $argument:ident : $type:ty)*) -> $base:ident) => {
        $(#[$attribute])*
        pub fn $name(
            field: impl Into<String>,
            member_identity: u64,
            $($argument: $type),*
        ) -> Result<Self, MaterializationDiagnostic> {
            let mut value = Self::$base(field, $($argument),*)?;
            value.member_identity = Some(member_identity);
            Ok(value)
        }
    };
}

/// `numbered_constructor` for the field carriers whose positional sibling is
/// infallible: the identity is stamped through the same construct-then-set
/// shape, so every numbered spelling of one carrier agrees on where the
/// identity enters.
macro_rules! numbered_variant {
    ($(#[$attribute:meta])* $name:ident(field $(, $argument:ident : $type:ty)*) -> $base:ident) => {
        $(#[$attribute])*
        pub fn $name(field: impl Into<String>, member_identity: u64, $($argument: $type),*) -> Self {
            Self {
                member_identity: Some(member_identity),
                ..Self::$base(field, $($argument),*)
            }
        }
    };
}

mod layout_reports;
mod materialization;
mod placement;
mod symbolic_materialization;
mod symbolic_values;

pub use layout_reports::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalNestedRecordSumOccurrenceLayoutReport,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalNestedRecordSumPathsLayoutReport,
    ConventionalRecordSumChildHop, ConventionalRecordSumChildInterior,
    ConventionalRecordSumChildLayoutReport, ConventionalRecursiveRecordSumPathsLayoutReport,
    ConventionalSumArrayFieldLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, IntegerInterpretation, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, NativeLayoutPlanReport,
    PrivateCallbackLayoutDemandReport, conventional_sum_layout_reports_match_for_replay,
    layout_plan_reports_match_for_replay, normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint, normalized_native_layout_plan_report_fingerprint,
};
pub use materialization::field_codec::{
    decode_aggregate_field, decode_aggregate_field_into, stage_scalar_field, update_scalar_field,
};
pub use materialization::field_values::{
    AggregateFieldSchema, AggregateFieldValue, ScalarFieldSchema, ScalarFieldValue,
};
pub use materialization::stored_integer_writes::{
    apply_write, validate_fragment, validate_stored_integer_fit, validate_stored_integer_fit_shape,
    validate_write, validate_write_source_value,
};
pub use materialization::{
    MaterializationDiagnostic, SymbolicMaterializationPlan, decode_scalar_layout,
    materialize_aggregate_layout_into, materialize_scalar_layout_into, stage_aggregate_layout,
};
pub use placement::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, MachineRegimeId,
    MaterializationAction, MaterializationContext, MaterializationWrite, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, PlacementSite, StoredIntegerFit,
};
pub use symbolic_materialization::{
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
};
pub use symbolic_values::{
    DataSymbolId, EntryStubId, RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldInterior,
    SymbolicFieldInteriorLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
};

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
