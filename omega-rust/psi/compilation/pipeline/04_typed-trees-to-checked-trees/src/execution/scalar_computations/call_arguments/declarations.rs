//! Declaration calls consume admitted whole entry inputs. This joins their
//! copy receipt to the ordinary owned argument planner; it manufactures no
//! statement occurrence, loan, or aggregate from the scalar read roster.

use super::super::{Builder, ExpressionHandle};

impl Builder<'_, '_> {
    pub(super) fn declaration_structural_argument(
        &mut self,
        application: checked_trees::CheckedRankApplicationFormationHandle,
        call: arena::Handle<checked_trees::CheckedDeclarationCall>,
        argument: ExpressionHandle,
        target: &typed_trees::signature::StateParameter,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let scope = self.declaration_scope.as_ref()?;
        if scope.application != application {
            return None;
        }
        let (formation, call) = checked_trees::CheckedCallOrigin::Declaration { application, call }
            .declaration(scope.plan, self.machine, self.state)
            .ok()?;
        let input = formation.owned_copy?;
        let declaration = self
            .program
            .measures()
            .iter()
            .find(|declaration| declaration.symbol == formation.application.measure)?;
        if declaration.parameter.as_ref()?.symbol != input.formal
            || !self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .contains(&argument)
        {
            return None;
        }
        let authored = validation::canonical_place_from_expression(self.program, argument)?;
        if authored.root.as_symbol() != Some(input.formal) {
            return None;
        }
        let coordinate = scope.plan.coordinates.get(input.actual);
        let root_path = scope.plan.path_segments.span(coordinate.path)?;
        let actual = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(coordinate.parameter),
            segments: root_path
                .iter()
                .chain(&authored.segments)
                .cloned()
                .collect(),
        };
        if !validation::reference_result_custody::source_value_is_copied(
            self.program,
            input.actual_type,
        ) {
            return None;
        }
        let (_, state) = validation::find_state(self.program, self.state)?;
        let parameter = self
            .program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == coordinate.parameter)?;
        let actual_type = validation::project_type_reference_from_segments(
            self.program,
            parameter.type_reference,
            root_path,
        )?;
        if !validation::declared_input_type_matches(self.program, actual_type, input.actual_type) {
            return None;
        }
        let selectors = self.projection_selector_evaluations(argument, argument)?;
        let mut argument_plan = crate::execution::terminal_unit::calls::owned_argument(
            self.program,
            &mut self.shapes,
            self.machine,
            state,
            0,
            &actual,
            target,
            Some(&|expression| {
                selectors.iter().find_map(|(source, value)| {
                    (*source == expression).then_some(
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                    )
                })
            }),
        )?;
        argument_plan.retain_projection_evaluations(
            self.projection_selector_span(argument, &selectors)?,
            &self.plans.operands,
        )?;
        Some(argument_plan)
    }
}
