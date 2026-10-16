//! Native type layout: sizes, alignment, field paths, and sum materialization.
//!
//! Start at `layout_plan.rs`: `LayoutPlan` and the `TypeLayout`,
//! `TypeLayoutDescriptor`, and field/data layout vocabulary it
//! carries. `builder.rs` turns a typed reference into a `TypeLayout`
//! whose descriptor records packed fields, bit-field fragments, and case
//! payloads. `callback_layout` retains and checks private callback field paths and
//! their target geometry independently of source-parameter association;
//! `boundary_values` constructs bounded native signature graphs and selected
//! opaque carrier occurrences from already-typed boundary applications.
//! Layout never authorizes an access; it only fixes
//! where a value's bytes live. It consumes typed declarations and installed layout
//! plans, never check facts; callers must not manufacture a checked program to
//! request geometry before checking.

mod boundary_values;
mod builder;
mod callback_layout;
mod layout_plan;
mod packing;
mod sizing;

pub use boundary_values::{
    BOUNDARY_VALUE_FIELD_CAPACITY, BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryOpaqueRepresentationUse,
    BoundaryTypeBinding, BoundaryValueLayout, append_boundary_value_shape,
    substitute_boundary_type_reference,
};
pub use builder::{build_layout_plan, build_layout_plan_for_roots, layout_type_reference};
pub use callback_layout::{PrivateCallbackInlineField, PrivateCallbackLayout};
pub(crate) use layout_plan::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, ENUM_TAG_BYTES, FieldLayout,
    RepeatedFieldLayout, StoredIntegerLayout, TypeLayoutDescriptor, VariantLayout,
};
pub use layout_plan::{
    LayoutPlan, TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand, TypeLayout,
};
