/*
Answer Psi's source layout queries (`size_of`/`align_of`) with the selected
target's geometry (`TargetLayoutQueries`, `TargetLayoutMeasurements`).

Psi owns query formation, deferral and literal installation, and asks its
caller for the numbers through `LayoutQueryProvider`. Build admission is that
caller, so the adapter lives here: it implements Psi's evaluation traits and
asks provider planning's layout builder (`try_layout_type_reference`) for each
answer. It prepares one private target-family view per pass without scheduling
evaluation or rewriting source.

Invariant programs borrow the caller's current program instead of copying its
arenas, while family-laid programs clone a single private layout view closed
under the selected target profile so source alternatives remain intact for
Terminal production. Nothing here consumes check facts; callers must never
manufacture a checked program to request geometry before checking.
*/

use build_time_evaluation::{LayoutQueryMeasurements, LayoutQueryProvider};
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;
use provider_planning::data_layout_queries::try_layout_type_reference;
use typed_trees::TypedTrees;
use typed_trees::expression::DataLayoutQueryRequest;

pub(crate) struct TargetLayoutQueries<'selection> {
    pub(crate) target: target::TargetProfile,
    pub(crate) applications:
        Option<&'selection [build_time_evaluation::LayoutApplicationTargetInput]>,
    pub(crate) opaque_representations:
        &'selection [provider_planning::data_layout_queries::OpaqueCarrierSelection],
}

pub(crate) struct TargetLayoutMeasurements<'selection> {
    target: target::TargetProfile,
    opaque_representations:
        &'selection [provider_planning::data_layout_queries::OpaqueCarrierSelection],
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
                    typed_trees::PlanLaidRealizations::TargetInputs(_)
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
                            typed_trees::PlanLaidRealizations::Invariant(geometry.clone());
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
