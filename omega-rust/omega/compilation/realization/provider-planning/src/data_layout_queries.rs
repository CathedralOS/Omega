/*
We supply selected target geometry (`TargetLayoutQueries` and
`TargetLayoutMeasurements`) for Psi's source layout-query evaluation. Psi owns
query formation, deferral, and literal installation, while this adapter supplies
the selected target's layout answer and prepares one private target-family view
per pass without scheduling evaluation or rewriting source.

The answer walks typed declarations, so it lives here alongside source
correspondence rather than in the source-free backend `layout` crate. Invariant
programs borrow the caller's current program instead of copying its arenas,
while family-laid programs clone a single private layout view closed under the
selected target profile so source alternatives remain intact for Terminal
production.

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

use build_time_evaluation::{LayoutQueryMeasurements, LayoutQueryProvider};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::expression::DataLayoutQueryRequest;
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;

// A boundary-opaque data type is laid out as the carrier the authoritative
// build selected for it. Layout needs only that pairing, so callers project it
// from their selection records here; the conformance application, lifecycle
// and commitments of a selection stay with the source-side owners that check
// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueCarrierSelection {
    pub opaque: symbols::SymbolHandle,
    pub carrier_type: checked_trees::typed_trees::types::TypeReferenceHandle,
}

pub struct TargetLayoutQueries<'selection> {
    pub target: target::TargetProfile,
    pub applications: Option<&'selection [build_time_evaluation::LayoutApplicationTargetInput]>,
    pub opaque_representations: &'selection [OpaqueCarrierSelection],
}

pub struct TargetLayoutMeasurements<'selection> {
    target: target::TargetProfile,
    opaque_representations: &'selection [OpaqueCarrierSelection],
    layout_program: Option<TypedTrees>,
}

impl<'selection> LayoutQueryProvider for TargetLayoutQueries<'selection> {
    type Measurements = TargetLayoutMeasurements<'selection>;

    fn layout_target_inputs(&self) -> Vec<layout_plans::LayoutTargetFacts> {
        match self.applications {
            Some(inputs) => {
                let mut targets = Vec::new();
                for input in inputs {
                    if !targets.contains(&input.target) {
                        targets.push(input.target.clone());
                    }
                }
                targets
            }
            None => self.target.layout_target_facts().into_iter().collect(),
        }
    }

    fn layout_application_inputs(
        &self,
    ) -> Option<&[build_time_evaluation::LayoutApplicationTargetInput]> {
        self.applications
    }

    fn prepare(&self, program: &TypedTrees) -> Self::Measurements {
        let target = self.target;
        let layout_program = program
            .plan_laid_layouts
            .iter()
            .any(|plan| {
                matches!(
                    plan.realizations,
                    checked_trees::typed_trees::PlanLaidRealizations::TargetInputs(_)
                )
            })
            .then(|| {
                let mut closed = program.clone();
                for (plan, authored) in closed
                    .plan_laid_layouts
                    .iter_mut()
                    .zip(&program.plan_laid_layouts)
                {
                    if let Some(geometry) =
                        authored.geometry_for_target(program, Some(target.target_name()))
                    {
                        plan.realizations =
                            checked_trees::typed_trees::PlanLaidRealizations::Invariant(
                                geometry.clone(),
                            );
                    }
                }
                closed
            });

        TargetLayoutMeasurements {
            target,
            opaque_representations: self.opaque_representations,
            layout_program,
        }
    }
}

impl LayoutQueryMeasurements for TargetLayoutMeasurements<'_> {
    fn measure(
        &self,
        program: &TypedTrees,
        query: &DataLayoutQueryRequest,
    ) -> Result<Option<usize>, Diagnostic> {
        let layout = try_layout_type_reference(
            self.layout_program.as_ref().unwrap_or(program),
            self.target.native_target(),
            self.opaque_representations,
            query.operand.type_reference,
        )?;
        Ok(layout.map(|layout| match query.kind {
            DataLayoutQueryKind::Size => layout.size,
            DataLayoutQueryKind::Alignment => layout.alignment,
        }))
    }
}

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
