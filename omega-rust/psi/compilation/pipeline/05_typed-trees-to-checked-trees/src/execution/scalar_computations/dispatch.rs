//! A dispatch retains one subject computation and its covered authored prefix.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, PrimitiveType,
};
use crate::flow::expressions;
use crate::values::operator_is_builtin;
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar_expression_type;
use checked_trees::{
    CheckedScalarComputationStructuralArgument, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
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
        // The application owns its complete feasible-arm set. Keep all tested
        // patterns, but do not request operations from impossible arm values.
        let admitted_arms = self.declaration_scope.as_ref().and_then(|scope| {
            let application = scope.plan.application_formations.get(scope.application);
            let arms = application
                .application
                .dispatches
                .handles()
                .filter_map(|handle| {
                    let arm = scope.plan.application_dispatches.get(handle);
                    (arm.expression == source_expression).then_some(arm.source_arm)
                })
                .collect::<Vec<_>>();
            (!arms.is_empty()).then_some(arms)
        });
        let subject = self.dispatch_subject(dispatch)?;
        if let Some(admitted_arms) = admitted_arms {
            return self.declaration_dispatch(
                source_expression,
                dispatch,
                subject,
                result_type,
                &admitted_arms,
            );
        }
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
        let checked_trees::CheckedDispatchSubject::Scalar(subject_handle) = subject else {
            return None;
        };
        let subject_type = self.plans.nodes.get(subject_handle).primitive_type;
        // An arm whose value no path reaches, such as `_` after `true` in
        // `match true { ... }`, has no call or value occurrence to plan: the
        // occurrence collectors skip it. Plan only the reachable arms; the
        // last of them covers every input that reaches it.
        let reachable = dispatch
            .arms
            .handles()
            .zip(self.program.expression_table.match_arms(dispatch.arms))
            .filter(|(_, arm)| !self.program.static_type_context_is_unreachable(arm.value))
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        if reachable.len() < dispatch.arms.len()
            && !matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
        {
            return self.declaration_dispatch(
                source_expression,
                dispatch,
                subject,
                result_type,
                &reachable,
            );
        }
        let subject = subject_handle;
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

    /// Consume the checked path union without importing branch assumptions or
    /// forming an unreachable child. Failed pattern evaluations still precede
    /// the next feasible pattern; the final feasible arm covers the admitted
    /// input, and evaluates its own pattern before producing its value.
    fn declaration_dispatch(
        &mut self,
        source_expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        subject: checked_trees::CheckedDispatchSubject,
        result_type: PrimitiveType,
        admitted: &[arena::Handle<typed_trees::expression::TableMatchArm>],
    ) -> Option<CheckedScalarComputationHandle> {
        let checked_trees::CheckedDispatchSubject::Scalar(subject) = subject else {
            return None;
        };
        let authored = self.program.expression_table.match_arms(dispatch.arms);
        let handles = dispatch.arms.handles().collect::<Vec<_>>();
        if admitted.iter().any(|arm| !handles.contains(arm)) {
            return None;
        }
        let last = handles.iter().rposition(|arm| admitted.contains(arm))?;
        let subject_type = self.plans.nodes.get(subject).primitive_type;
        let mut pending = Vec::new();
        let mut arms = Vec::new();
        for (ordinal, (source_arm, arm)) in handles.iter().zip(authored).enumerate().take(last + 1)
        {
            match arm.pattern {
                MatchPattern::Value(pattern) => {
                    pending.push(self.expression(pattern, subject_type)?)
                }
                MatchPattern::Wildcard => {}
                MatchPattern::Destructure(_) => return None,
            }
            if !admitted.contains(source_arm) {
                continue;
            }
            let value = self.expression(arm.value, result_type)?;
            let (pattern, value) = if ordinal == last {
                pending.push(value);
                (
                    CheckedScalarDispatchPattern::Wildcard,
                    self.dispatch_prefix(
                        source_expression,
                        result_type,
                        std::mem::take(&mut pending),
                    )?,
                )
            } else {
                if matches!(arm.pattern, MatchPattern::Wildcard) {
                    return None;
                }
                let pattern = self.dispatch_prefix(
                    source_expression,
                    subject_type,
                    std::mem::take(&mut pending),
                )?;
                (CheckedScalarDispatchPattern::Value(pattern), value)
            };
            arms.push(CheckedScalarDispatchArm {
                equality_use: arena::Handle::invalid(),
                source_arm: *source_arm,
                pattern,
                value,
            });
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

    fn dispatch_prefix(
        &mut self,
        source_expression: ExpressionHandle,
        primitive_type: PrimitiveType,
        operands: Vec<CheckedScalarComputationHandle>,
    ) -> Option<CheckedScalarComputationHandle> {
        let position = operands.len().checked_sub(1)?;
        if position == 0 {
            return operands.first().copied();
        }
        let operands = self.plans.operands.insert_many(operands);
        Some(self.insert(
            primitive_type,
            CheckedScalarComputationKind::Apply {
                source_expression,
                expression: checked_trees::CheckedScalarExpression::Parameter {
                    position,
                    primitive_type,
                },
                operands,
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
            let alias = if dispatch.capture.is_valid() {
                expressions(self.program)
                    .value_capture(dispatch.capture)
                    .value_anchor
            } else {
                ExpressionHandle::invalid()
            };
            // A source-checked place is observed under its existing custody.
            // Copy captures and stored descriptors keep their independent value
            // producer; neither path can turn an affine referent into an owner.
            if !self.uses_captured_dispatch_value(dispatch.subject)
                && validation::reference_shell(self.program, reference).is_none()
                && let Some(place) = self.canonical_place(dispatch.subject)
            {
                let place = if place.root.as_symbol().is_some() {
                    Some(
                        self.borrowed_leaf_argument(dispatch.subject, reference, dispatch.subject)?
                            .0,
                    )
                } else {
                    self.readable_case_subject(dispatch.subject, self.statement_index)
                        .map(|(_, place)| place)
                };
                if let Some(place) = place {
                    // Even a tag-only selection captures its endpoint once.
                    // With no lexical binding the exact subject occurrence is
                    // its private completed-expression name.
                    let alias = if alias.is_valid() {
                        alias
                    } else {
                        dispatch.subject
                    };
                    self.retain_dispatch_receiver(
                        alias,
                        checked_trees::CheckedStructuralValueHandle::invalid(),
                    )?;
                    return Some(checked_trees::CheckedDispatchSubject::Structural {
                        expression: dispatch.subject,
                        value: checked_trees::CheckedStructuralValueHandle::invalid(),
                        alias,
                        place,
                    });
                }
            }
            let value = self.structural_value(dispatch.subject, reference)?;
            self.retained_argument_owner(value, reference)?;
            self.retain_structural_value_root(value, reference)?;
            self.retain_dispatch_receiver(alias, value)?;
            return Some(checked_trees::CheckedDispatchSubject::Structural {
                expression: dispatch.subject,
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
                        logical_root_type_identity: None,
                    },
                    path: Vec::new(),
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        reference,
                        &[],
                        &[],
                    ),
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

    fn retain_dispatch_receiver(
        &mut self,
        alias: ExpressionHandle,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Option<()> {
        if !alias.is_valid() {
            return Some(());
        }
        match self
            .dispatch_receivers
            .entry((alias.arena_index(), alias.generation()))
        {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
                Some(())
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                (*entry.get() == value).then_some(())
            }
        }
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
        let mut rejected_patterns = Vec::new();
        for (ordinal, arm) in authored.iter().enumerate() {
            if self.structural_dispatch_arm_is_impossible(dispatch, arm) {
                continue;
            }
            let source_arm = dispatch.arms.handle_at(u32::try_from(ordinal).ok()?);
            let mut pattern = self.structural_dispatch_pattern(
                source_expression,
                dispatch,
                &subject,
                arm.pattern,
            )?;
            if self.program.static_type_context_is_unreachable(arm.value) {
                // A guard can execute and reject this arm on every normal
                // return. Keep its complete computation on the fallthrough
                // edge; the nonexistent value needs no call or payload plan.
                rejected_patterns.push(checked_trees::CheckedScalarEvaluationOperand::Scalar(
                    pattern.computation()?,
                ));
                continue;
            }
            let mut value = self.expression(arm.value, result_type)?;
            if !rejected_patterns.is_empty() {
                let operands = self
                    .plans
                    .evaluation_operands
                    .insert_many(std::mem::take(&mut rejected_patterns));
                // This generated scope sequences the preceding rejected
                // patterns before this next decision, without reevaluating
                // the dispatch subject or inventing an impossible arm value.
                match pattern {
                    CheckedScalarDispatchPattern::Predicate(predicate) => {
                        pattern = CheckedScalarDispatchPattern::Predicate(self.insert(
                            PrimitiveType::Bool,
                            CheckedScalarComputationKind::WithOperands {
                                source_expression: ExpressionHandle::invalid(),
                                operands,
                                operand: predicate,
                            },
                        ));
                    }
                    CheckedScalarDispatchPattern::Wildcard => {
                        value = self.insert(
                            result_type,
                            CheckedScalarComputationKind::WithOperands {
                                source_expression: ExpressionHandle::invalid(),
                                operands,
                                operand: value,
                            },
                        );
                    }
                    CheckedScalarDispatchPattern::Value(_) => return None,
                }
            }
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
        if !rejected_patterns.is_empty() {
            return None;
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

    /// The subject has already completed. A condition with no prefix arrival,
    /// or a false condition built from its pure tag test, evaluates no guard.
    /// A body contradiction alone cannot erase a reachable guard's effects.
    pub(super) fn structural_dispatch_arm_is_impossible(
        &self,
        dispatch: &TableMatchExpression,
        arm: &typed_trees::expression::TableMatchArm,
    ) -> bool {
        match arm.pattern {
            MatchPattern::Destructure(pattern) => {
                self.program
                    .static_type_context_is_unreachable(pattern.condition)
                    || self.program.static_boolean_value(pattern.condition) == Some(false)
            }
            MatchPattern::Value(_)
                if self.program.static_type_context_is_unreachable(arm.value) =>
            {
                let Some(machine) = self.program.machine_by_symbol(self.machine) else {
                    return false;
                };
                let Some(state) = self.program.state_in_machine(machine, self.state) else {
                    return false;
                };
                validation::structural_match_pattern_case(
                    self.program,
                    machine,
                    state,
                    dispatch,
                    arm.pattern,
                )
                .is_some_and(|case| case.is_valid())
            }
            _ => false,
        }
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
            alias,
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
                    CheckedScalarComputationStructuralArgument::Place(if receiver.is_valid() {
                        place.clone()
                    } else {
                        CheckedUnitStructuralArgumentPlan {
                            source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                expression: *alias,
                                logical_root_type_identity: None,
                            },
                            path: Vec::new(),
                            projection_evaluations: Default::default(),
                            type_identity: place.type_identity,
                            access: place.access,
                        }
                    }),
                    case,
                )
            }
            MatchPattern::Wildcard => return None,
        };
        Some(CheckedScalarDispatchPattern::Predicate(selected))
    }
}

/// The comparison schedule is independent of the selected result's carrier.
pub(super) struct PreparedDispatchArm {
    pub(super) source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
    pub(super) equality_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
    pub(super) pattern: CheckedScalarDispatchPattern,
    pub(super) value: ExpressionHandle,
}

impl Builder<'_, '_> {
    pub(super) fn prepare_dispatch(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
    ) -> Option<(
        checked_trees::CheckedDispatchSubject,
        Vec<PreparedDispatchArm>,
    )> {
        let subject = self.dispatch_subject(dispatch)?;
        let subject_type = match subject {
            checked_trees::CheckedDispatchSubject::Scalar(value) => {
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralValueSubject { expression },
                    value,
                )?;
                Some(self.plans.nodes.get(value).primitive_type)
            }
            checked_trees::CheckedDispatchSubject::Structural { .. } => None,
        };
        let authored = self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .to_vec();
        let mut arms = Vec::new();
        let mut covered = false;
        let mut boolean_coverage = [false; 2];
        for (ordinal, arm) in authored.iter().enumerate() {
            if subject_type.is_none() && self.structural_dispatch_arm_is_impossible(dispatch, arm) {
                continue;
            }
            let source_arm = dispatch.arms.handle_at(u32::try_from(ordinal).ok()?);
            let mut equality_use = arena::Handle::invalid();
            let pattern = if let Some(subject_type) = subject_type {
                match arm.pattern {
                    MatchPattern::Wildcard => {
                        covered = true;
                        checked_trees::CheckedScalarDispatchPattern::Wildcard
                    }
                    MatchPattern::Value(pattern) => {
                        if subject_type == PrimitiveType::Bool
                            && let Some(value) = self.program.expression_table.boolean_node(pattern)
                        {
                            if boolean_coverage[usize::from(*value)] {
                                continue;
                            }
                            boolean_coverage[usize::from(*value)] = true;
                            covered = boolean_coverage == [true, true];
                        }
                        if matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64) {
                            equality_use = self.comparison_use(
                                expression,
                                checked_trees::CheckedOperatorOccurrence::MatchEquality {
                                    source_arm,
                                },
                            )?;
                        }
                        let computation = self.expression(pattern, subject_type)?;
                        self.plans.nodes.get_mut(computation).authored_root = pattern;
                        self.push_scalar_root(
                            CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                            computation,
                        )?;
                        checked_trees::CheckedScalarDispatchPattern::Value(computation)
                    }
                    MatchPattern::Destructure(_) => return None,
                }
            } else {
                self.structural_dispatch_pattern(expression, dispatch, &subject, arm.pattern)?
            };
            if let Some(computation) = pattern.computation()
                && subject_type.is_none()
            {
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                    computation,
                )?;
            }
            arms.push(PreparedDispatchArm {
                source_arm,
                equality_use,
                pattern,
                value: arm.value,
            });
            if matches!(arm.pattern, MatchPattern::Wildcard) || covered {
                break;
            }
        }
        // Structural coverage was established by the common pattern
        // checker, including case completeness and guarded-arm limits.
        if (!covered && subject_type.is_some()) || arms.is_empty() {
            return None;
        }
        Some((subject, arms))
    }
}
