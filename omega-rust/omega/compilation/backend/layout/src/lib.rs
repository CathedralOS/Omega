//! Native type layout: sizes, alignment, field paths, and sum materialization.
//!
//! Start at `layout_plan.rs`: `LayoutPlan` and the `TypeLayout`,
//! `TypeLayoutDescriptor`, and field/data layout vocabulary it
//! carries. `builder.rs` turns a typed reference into a `TypeLayout`
//! whose descriptor records packed fields, bit-field fragments, and case
//! payloads. `callback_layout` retains and checks private callback field paths and
//! their target geometry independently of source-parameter association;
//! `boundary_values` constructs bounded native signature graphs and selected
//! opaque carrier occurrences from already-typed boundary applications;
//! `sum_materialization` projects conventional sum layouts for the
//! layout-plans foundation. Layout never authorizes an access; it only fixes
//! where a value's bytes live. It consumes typed declarations and installed layout
//! plans, never check facts; callers must not manufacture a checked program to
//! request geometry before checking.

mod boundary_values;
mod builder;
mod callback_layout;
mod layout_plan;
mod packing;
mod sizing;
mod sum_materialization;

pub use boundary_values::{
    BOUNDARY_VALUE_FIELD_CAPACITY, BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryOpaqueRepresentationUse,
    BoundaryTypeBinding, BoundaryValueLayout, append_boundary_value_shape,
    substitute_boundary_type_reference,
};
pub use builder::{build_layout_plan, layout_type_reference};
pub use callback_layout::{PrivateCallbackInlineField, PrivateCallbackLayout};
pub use layout_plan::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, ENUM_TAG_BYTES, FieldLayout,
    LayoutPlan, RepeatedFieldLayout, StoredIntegerLayout, TargetClosedPlanLaidDataLayoutIdentity,
    TargetClosedPrivateCallbackDemand, TargetClosedTwoHopPrivateCallbackPath, TypeLayout,
    TypeLayoutDescriptor, VariantLayout,
};
pub use sizing::primitive_layout;
pub use sum_materialization::{
    project_conventional_record_with_nested_sum_record_materialization_layout,
    project_conventional_record_with_nested_sum_records_materialization_layout,
    project_conventional_record_with_recursive_nested_sums_materialization_layout,
    project_conventional_record_with_sum_array_materialization_layout,
    project_conventional_record_with_sum_arrays_materialization_layout,
    project_conventional_record_with_sum_materialization_layout,
    project_conventional_sum_materialization_layout,
};
