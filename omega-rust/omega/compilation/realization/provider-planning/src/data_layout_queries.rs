//! Selected target geometry for Psi's source layout-query evaluation.
//!
//! Psi owns query formation, deferral and literal installation. This adapter
//! supplies the selected target's layout answer and prepares one private
//! target-family view per pass. It does not schedule evaluation or rewrite
//! source.
//!
//! The answer walks typed declarations, so it lives here with the other source
//! correspondence rather than in the backend `layout` crate, which stays
//! source-free. Start at `layout_plan.rs` for the `LayoutPlan` vocabulary;
//! `builder.rs` turns a typed reference into a `TypeLayout` whose descriptor
//! records packed fields, bit-field fragments and case payloads;
//! `callback_layout` retains and checks private callback field paths and their
//! target geometry. Pointer and descriptor words come from `layout`. Nothing
//! here consumes check facts; callers must not manufacture a checked program
//! to request geometry before checking.

mod builder;
mod callback_layout;
mod layout_plan;
mod packing;
mod sizing;

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

/// Target/representation inputs stay outside Psi's evaluation sequence.
pub struct TargetLayoutQueries<'selection> {
    pub target: target::TargetProfile,
    pub applications: Option<&'selection [build_time_evaluation::LayoutApplicationTargetInput]>,
    pub opaque_representations:
        &'selection [representation_selections::OpaqueRepresentationSelection],
}

/// Invocation-local backend view; invariant programs borrow the caller's
/// current program instead of copying its arenas.
pub struct TargetLayoutMeasurements<'selection> {
    target: target::TargetProfile,
    opaque_representations:
        &'selection [representation_selections::OpaqueRepresentationSelection],
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
        // The physical backend consumes invariant geometry. A family needs one
        // private layout view closed under this exact profile; source alternatives
        // remain intact for Terminal production. All queries borrow that same view.
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
