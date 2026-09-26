//! Native type layout: sizes, alignment, field paths, and sum materialization.
//!
//! Start at `layout_plan.rs`: `LayoutPlan` and the `TypeLayout`,
//! `TypeLayoutDescriptor`, and field/data/machine layout vocabulary it
//! carries. `builder.rs` turns a checked type reference into a `TypeLayout`
//! whose descriptor records packed fields, bit-field fragments, and case
//! payloads; `field_paths` resolves a field path to its offset;
//! `sum_materialization` projects conventional sum layouts for the
//! layout-plans foundation. Layout never authorizes an access; it only fixes
//! where a value's bytes live.

mod builder;
mod field_paths;
mod layout_plan;
mod packing;
mod sizing;
mod sum_materialization;

pub use builder::{build_layout_plan, layout_type_reference};
pub use field_paths::{field_data_layout_fields, field_machine_layout, field_path_offset};
pub use layout_plan::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, ENUM_TAG_BYTES, FieldLayout,
    LayoutPlan, MachineLayout, RepeatedFieldLayout, StoredIntegerLayout,
    TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
    TargetClosedTwoHopPrivateCallbackPath, TypeLayout, TypeLayoutDescriptor, VariantLayout,
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
