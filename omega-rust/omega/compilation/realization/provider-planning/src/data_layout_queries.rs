//! Selected target geometry for Psi's source layout-query evaluation.
//!
//! Psi owns query formation, deferral and literal installation. This adapter
//! supplies the backend layout answer and prepares one private target-family
//! view per pass. It does not schedule evaluation or rewrite source.

use build_time_evaluation::{LayoutQueryMeasurements, LayoutQueryProvider};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::expression::DataLayoutQueryRequest;
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;

/// Target/representation inputs stay outside Psi's evaluation sequence.
pub struct TargetLayoutQueries<'selection> {
    pub target: target::TargetProfile,
    pub opaque_representations:
        &'selection [representation_planning::OpaqueRepresentationSelection],
}

/// Invocation-local backend view; invariant programs borrow the caller's
/// current program instead of copying its arenas.
pub struct TargetLayoutMeasurements<'selection> {
    target: target::TargetProfile,
    opaque_representations: &'selection [representation_planning::OpaqueRepresentationSelection],
    layout_program: Option<TypedTrees>,
}

impl<'selection> LayoutQueryProvider for TargetLayoutQueries<'selection> {
    type Measurements = TargetLayoutMeasurements<'selection>;

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
                    checked_trees::typed_trees::PlanLaidRealizations::TargetFamily(_)
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
        let layout = layout::try_layout_type_reference(
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
