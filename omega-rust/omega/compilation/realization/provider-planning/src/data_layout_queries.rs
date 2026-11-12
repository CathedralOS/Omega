/*
We answer selected target geometry for typed declarations: layout plans, type
layouts and private callback layouts. Psi owns source layout-query formation,
deferral, and literal installation; Build admission adapts those queries to
`try_layout_type_reference` (`build_settings::admission::target_layout_queries`)
so this crate supplies the answer without scheduling evaluation or rewriting
source.

The answer walks typed declarations, so it lives here alongside source
correspondence rather than in the source-free backend `layout` crate.

We also fold source primitive geometry (`sizing`) over the backend's pointer
and descriptor words directly into this module entrypoint. Nothing here consumes
check facts; callers must never manufacture a checked program to request
geometry before checking.
*/

mod builder;
mod callback_layout;
mod layout_plan;
mod packing;

pub use builder::{
    build_layout_plan, build_layout_plan_for_roots, layout_type_reference,
    try_layout_type_reference,
};
pub use callback_layout::{PrivateCallbackInlineField, PrivateCallbackLayout};
pub(crate) use layout::LayoutTarget;
pub(crate) use layout_plan::{
    BitFieldFragment, BitFieldLayout, DataLayout, DataShape, ENUM_TAG_BYTES, FieldLayout,
    RepeatedFieldLayout, StoredIntegerLayout, TypeLayoutDescriptor, VariantLayout, type_layout,
};
pub use layout_plan::{
    LayoutPlan, TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand, TypeLayout,
};

pub(crate) mod sizing {
    use crate::data_layout_queries::{TypeLayout, type_layout};
    use checked_trees::typed_trees::types::PrimitiveType;
    pub(crate) use layout::{fat_descriptor_layout, pointer_layout};
    use target::NativeTarget;

    fn primitive_layout(
        pointer_size: usize,
        pointer_alignment: usize,
        primitive_type: PrimitiveType,
    ) -> TypeLayout {
        match primitive_type {
            PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::U8 => type_layout(1, 1),
            PrimitiveType::I16 | PrimitiveType::U16 => type_layout(2, 2),
            PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => type_layout(4, 4),
            PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 => type_layout(8, 8),
            PrimitiveType::Addr | PrimitiveType::Selector => {
                type_layout(pointer_size, pointer_alignment)
            }
        }
    }

    pub(super) fn primitive_type_layout(
        target: NativeTarget,
        primitive_type: PrimitiveType,
    ) -> TypeLayout {
        primitive_layout(
            target.pointer_size,
            target.pointer_alignment,
            primitive_type,
        )
    }
}
