//! Declaration calls consume admitted whole entry inputs. This joins their
//! copy receipt to the ordinary storage argument planner; it manufactures no
//! statement occurrence or aggregate from the scalar read roster. A shared
//! call may lend this initialized, borrow-free entry home until its pure,
//! total scalar invocation returns. That loan's authority is the complete
//! application input, not a fictitious statement-access capture.

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
        let plan = scope.plan;
        let coordinate = plan.coordinates.get(input.actual);
        let root_path = plan.path_segments.span(coordinate.path)?;
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
        let geometry = validation::structural_coordinate(
            self.program,
            self.authored_parameters,
            coordinate.parameter,
            root_path,
        )?;
        let bounds = plan.entry_indexes.span(coordinate.entry_indexes)?;
        let mut selectors = Vec::new();
        for (path_step, projection) in geometry.projections.iter().enumerate() {
            let validation::ScalarCoordinateProjection::Element { selector, .. } = projection
            else {
                continue;
            };
            // Constant fixed-array coordinates already lower as FixedIndex.
            // Other selectors carry their own entry-bound identity, not an
            // authored argument occurrence in the declaration's namespace.
            let checked =
                self.entry_index_selector(coordinate.parameter, path_step, projection, bounds)?;
            let checked_trees::CheckedEntryIndexSelector::Expression(expression) = selector else {
                return None;
            };
            let primitive = crate::values::scalar::expression_facts::scalar_expression_type(
                &checked.expression,
            )?;
            let value = self.insert(
                primitive,
                checked_trees::CheckedScalarComputationKind::Value(checked.expression),
            );
            self.plans.nodes.get_mut(value).authored_root = *expression;
            selectors.push((*expression, value));
        }
        selectors.extend(self.projection_selector_evaluations(argument, argument)?);
        let mut storage_target = target.clone();
        let access = match validation::reference_shell(self.program, target.type_reference) {
            Some(shell) => {
                let typed_trees::expression::ExpressionNode::Borrow(borrow) =
                    self.program.expression_table.expression(argument)
                else {
                    return None;
                };
                if shell.access != language_semantics::ReferenceAccess::Shared
                    || borrow.access != shell.access
                {
                    return None;
                }
                storage_target.type_reference = shell.referee;
                checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            None => {
                if matches!(
                    self.program.expression_table.expression(argument),
                    typed_trees::expression::ExpressionNode::Borrow(_)
                ) {
                    return None;
                }
                checked_trees::CheckedStructuralAccess::Owned
            }
        };
        // The retained whole input proves initialized owned backing and Copy,
        // and contains no references or linear descendants. Its declaration
        // calls are already admitted pure/total scalar observations, so this
        // shared window ends before body effects or another cycle begins.
        let mut argument_plan = crate::execution::terminal_unit::calls::owned_argument(
            self.program,
            &mut self.shapes,
            self.machine,
            state,
            0,
            &actual,
            &storage_target,
            Some(&|expression| {
                selectors.iter().find_map(|(source, value)| {
                    (*source == expression).then_some(
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                    )
                })
            }),
        )?;
        argument_plan.access = access;
        let evaluations = self
            .plans
            .operands
            .insert_many(selectors.iter().map(|(_, value)| *value));
        argument_plan.retain_projection_evaluations(evaluations, &self.plans.operands)?;
        Some(argument_plan)
    }
}
