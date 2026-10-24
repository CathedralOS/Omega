//! A dispatch retains one subject computation and its covered authored prefix.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    ExpressionNode, PrimitiveType,
};
use crate::values::operator_is_builtin;
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar_expression_type;
use checked_trees::{
    CheckedScalarComputationStructuralArgument, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::expression::{MatchPattern, TableMatchExpression};

impl Builder<'_, '_> {
    /// Shared operand primitive when one use is an admitted selected
    /// comparison. IEEE float requirements carry their canonical operation
    /// identity; integer boundary comparisons carry their authored spelling.
    /// Neither classification realizes provider semantics here.
    pub(super) fn selected_comparison_primitive(
        &self,
        operator_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
    ) -> Option<PrimitiveType> {
        self.operators
            .selected_float_comparison(self.program, operator_use)
            .map(|(_, primitive)| primitive)
            .or_else(|| {
                self.operators
                    .selected_integer_comparison(self.program, operator_use)
                    .map(|(_, primitive)| primitive)
            })
    }

    pub(super) fn comparison_use(
        &self,
        expression: ExpressionHandle,
        occurrence: checked_trees::CheckedOperatorOccurrence,
    ) -> Option<arena::Handle<checked_trees::CheckedOperatorUseFact>> {
        let mut matching = self.operators.uses.iter().filter_map(|(handle, selected)| {
            (selected.expression == expression && selected.occurrence == occurrence
                && matches!(selected.origin, checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, state_symbol, statement_index, .. }
                    if machine_symbol == self.machine && state_symbol == self.state && statement_index == self.statement_index)
                && self.selected_comparison_primitive(handle).is_some()).then_some(handle)
        });
        let selected = matching.next()?;
        matching.next().is_none().then_some(selected)
    }
    pub(super) fn dispatch(
        &mut self,
        source_expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        result_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        // Result transport is independent of pattern comparison. Each arm must
        // produce the exact requested scalar carrier through ordinary expression
        // lowering; selecting f32/f64 values neither compares nor converts them.
        // Subject admission below still requires its supported comparison meaning.
        // Anonymous comparisons have exact compile-time meaning without a
        // machine-width subject. Retain the selected arm's ordinary computation;
        // source custody rederives this selection from the unchanged Match root.
        if let Some(selected) =
            validation::select_anonymous_numeric_match_arm(self.program, dispatch, |expression| {
                operator_is_builtin(self.operators, expression)
            })
        {
            return self.expression(selected, result_type);
        }
        let subject = self.dispatch_subject(dispatch)?;
        if matches!(
            subject,
            checked_trees::CheckedDispatchSubject::Structural { .. }
        ) {
            return self.structural_subject_dispatch(
                source_expression,
                dispatch,
                subject,
                result_type,
            );
        }
        let checked_trees::CheckedDispatchSubject::Scalar(subject) = subject else {
            return None;
        };
        let subject_type = self.plans.nodes.get(subject).primitive_type;
        let authored = self.program.expression_table.match_arms(dispatch.arms);
        let mut arms = Vec::new();
        let mut boolean_coverage = [false; 2];
        let mut covered = false;
        for (ordinal, arm) in authored.iter().enumerate() {
            let source_arm = arena::Handle::from_parts(
                dispatch
                    .arms
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).ok()?)?,
                dispatch.arms.start().generation(),
            );
            let equality_use = if matches!(arm.pattern, MatchPattern::Value(_))
                && matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
            {
                self.comparison_use(
                    source_expression,
                    checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                )?
            } else {
                arena::Handle::invalid()
            };
            let pattern = match arm.pattern {
                MatchPattern::Destructure(_) => return None,
                MatchPattern::Wildcard => {
                    covered = true;
                    CheckedScalarDispatchPattern::Wildcard
                }
                MatchPattern::Value(expression) => {
                    if subject_type == PrimitiveType::Bool
                        && let ExpressionNode::Boolean(value) =
                            self.program.expression_table.expression(expression)
                    {
                        boolean_coverage[usize::from(*value)] = true;
                        covered = boolean_coverage.iter().all(|value| *value);
                    }
                    CheckedScalarDispatchPattern::Value(self.expression(expression, subject_type)?)
                }
            };
            let value = self.expression(arm.value, result_type)?;
            arms.push(CheckedScalarDispatchArm {
                equality_use,
                source_arm,
                pattern,
                value,
            });
            if covered {
                break;
            }
        }
        if !covered {
            return None;
        }
        let arms = self.plans.dispatch_arms.insert_many(arms);
        Some(self.insert(
            result_type,
            CheckedScalarComputationKind::Dispatch {
                source_expression,
                subject: checked_trees::CheckedDispatchSubject::Scalar(subject),
                arms,
            },
        ))
    }

    /// Retain one ordinary evaluated value, independent of its authored place.
    pub(super) fn dispatch_subject(
        &mut self,
        dispatch: &TableMatchExpression,
    ) -> Option<checked_trees::CheckedDispatchSubject> {
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(machine, self.state)?;
        if let Some(reference) =
            validation::structural_match_subject_type(self.program, machine, state, dispatch)
        {
            // Dispatch always needs an evaluated subject node, including an
            // existing place. Argument retention deliberately returns absence
            // when direct storage transport needs no temporary; that is not a
            // refusal to observe the same completed place in a match.
            let value = self.structural_value(dispatch.subject, reference)?;
            self.retained_argument_owner(value, reference)?;
            self.retain_structural_value_root(value, reference)?;
            let alias = if dispatch.capture.is_valid() {
                self.program
                    .expression_table
                    .match_subject(dispatch.capture)
                    .value_anchor
            } else {
                ExpressionHandle::invalid()
            };
            return Some(checked_trees::CheckedDispatchSubject::Structural {
                value,
                alias,
                place: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression: if alias.is_valid() {
                            alias
                        } else {
                            dispatch.subject
                        },
                    },
                    path: Vec::new(),
                    type_identity: self.program.normalized_type_identity(reference),
                    access: CheckedStructuralAccess::SharedBorrow,
                },
            });
        }
        let subject = if let Some(subject_type) =
            validation::match_subject_primitive_type(self.program, dispatch)
        {
            if subject_type != PrimitiveType::Bool
                && !is_integer(subject_type)
                && !matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            self.expression(dispatch.subject, subject_type)?
        } else if let Some(operand) = self.integer_operand(dispatch.subject, None)
            && scalar_expression_type(&operand.value).is_some_and(is_integer)
        {
            self.materialize_integer(operand)?
        } else {
            self.expression(dispatch.subject, PrimitiveType::Bool)?
        };
        Some(checked_trees::CheckedDispatchSubject::Scalar(subject))
    }

    fn structural_subject_dispatch(
        &mut self,
        source_expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        subject: checked_trees::CheckedDispatchSubject,
        result_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let authored = self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .to_vec();
        let mut arms = Vec::new();
        for (ordinal, arm) in authored.iter().enumerate() {
            let source_arm = dispatch.arms.handle_at(u32::try_from(ordinal).ok()?);
            let pattern = self.structural_dispatch_pattern(
                source_expression,
                dispatch,
                &subject,
                arm.pattern,
            )?;
            let value = self.expression(arm.value, result_type)?;
            arms.push(CheckedScalarDispatchArm {
                equality_use: arena::Handle::invalid(),
                source_arm,
                pattern,
                value,
            });
            if matches!(arm.pattern, MatchPattern::Wildcard) {
                break;
            }
        }
        let arms = self.plans.dispatch_arms.insert_many(arms);
        Some(self.insert(
            result_type,
            CheckedScalarComputationKind::Dispatch {
                source_expression,
                subject,
                arms,
            },
        ))
    }

    /// Structural predicates use the same checked Boolean computation as
    /// authored conjunctions. Normalization owns their tag/field/guard order;
    /// this consumer does not reconstruct a second conditional schedule.
    pub(super) fn structural_dispatch_pattern(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        subject: &checked_trees::CheckedDispatchSubject,
        pattern: MatchPattern,
    ) -> Option<CheckedScalarDispatchPattern> {
        if matches!(pattern, MatchPattern::Wildcard) {
            return Some(CheckedScalarDispatchPattern::Wildcard);
        }
        let checked_trees::CheckedDispatchSubject::Structural {
            place,
            value: receiver,
            ..
        } = subject
        else {
            return None;
        };
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(machine, self.state)?;
        let case = validation::structural_match_pattern_case(
            self.program,
            machine,
            state,
            dispatch,
            pattern,
        )?;
        let selected = match pattern {
            MatchPattern::Destructure(pattern) => {
                return Some(CheckedScalarDispatchPattern::Predicate(
                    self.expression(pattern.condition, PrimitiveType::Bool)?,
                ));
            }
            MatchPattern::Value(_) => {
                if !case.is_valid() {
                    return None;
                }
                self.publish_case_membership(
                    expression,
                    ExpressionHandle::invalid(),
                    *receiver,
                    CheckedScalarComputationStructuralArgument::Place(place.clone()),
                    case,
                )
            }
            MatchPattern::Wildcard => return None,
        };
        Some(CheckedScalarDispatchPattern::Predicate(selected))
    }
}
