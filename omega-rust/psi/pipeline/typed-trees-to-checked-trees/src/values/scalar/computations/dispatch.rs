//! A dispatch retains one subject computation and its covered authored prefix.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    ExpressionNode, PrimitiveType,
};
use crate::values::operator_is_builtin;
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::structural_fields::structural_parameter_field_path;
use crate::values::scalar_expression_type;
use checked_trees::{
    CheckedScalarComputationStructuralArgument, CheckedScalarDispatchArm,
    CheckedScalarDispatchPattern, CheckedStructuralAccess, CheckedStructuralPredicatePathSegment,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use typed_trees::expression::{MatchPattern, TableMatchExpression};
use validation::{MatchCaseDispatch, MatchCaseSubject};

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
        let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
        let state = self
            .program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == self.state)?;
        // A case-classified match on a payloadless-case sum subject is a
        // discriminant dispatch: ordered case-membership selections, with the
        // wildcard (or the last case arm) as the tail. No pattern value is
        // ever evaluated, so the scalar subject gates below do not apply.
        if let Some(case_dispatch) =
            validation::match_case_dispatch(self.program, machine, state, dispatch)
        {
            return self.case_dispatch(
                source_expression,
                machine,
                state,
                dispatch.subject,
                &case_dispatch,
                result_type,
            );
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
        } else if let Some(operand) = self.integer_operand(dispatch.subject)
            && scalar_expression_type(&operand.value).is_some_and(is_integer)
        {
            self.materialize_integer(operand)?
        } else {
            self.expression(dispatch.subject, PrimitiveType::Bool)?
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
                subject,
                arms,
            },
        ))
    }

    /// Fold an admitted discriminant dispatch into nested scalar selections:
    /// `match subject { C1 -> a, C2 -> b, _ -> rest }` becomes
    /// `select (subject is C1) a (select (subject is C2) b rest)`. The first
    /// wildcard arm supplies the fallback; authored arms after it stay
    /// unreachable, matching scalar dispatch's covered-prefix rule.
    fn case_dispatch(
        &mut self,
        source_expression: ExpressionHandle,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        subject: ExpressionHandle,
        dispatch: &MatchCaseDispatch,
        result_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let mut cases = Vec::new();
        let mut fallback = None;
        for arm in &dispatch.arms {
            let value = self.expression(arm.value, result_type)?;
            match arm.case {
                Some(case) => cases.push((case, value)),
                None => {
                    fallback = Some(value);
                    break;
                }
            }
        }
        // Exhaustive case coverage without a wildcard leaves no reachable
        // tail; the validator's coverage rule requires the wildcard anyway,
        // so a missing fallback is the unsupported shape.
        let mut selected = fallback?;
        for (case, value) in cases.iter().rev() {
            let condition = self.case_dispatch_condition(
                source_expression,
                machine,
                state,
                &dispatch.subject,
                subject,
                *case,
            )?;
            selected = self.insert(
                result_type,
                CheckedScalarComputationKind::Select {
                    source_expression,
                    condition,
                    when_true: *value,
                    when_false: selected,
                },
            );
        }
        Some(selected)
    }

    /// The membership test for one discriminant-dispatch arm, routed by the
    /// admitted subject's observable shape. Every form names the authored
    /// match as its source so lowering custody pairs the selection chain with
    /// the dispatch's classifier arms.
    fn case_dispatch_condition(
        &mut self,
        source_expression: ExpressionHandle,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        subject: &MatchCaseSubject,
        subject_expression: ExpressionHandle,
        case: symbols::SymbolHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        match subject {
            MatchCaseSubject::ParameterField => {
                let mut path = Vec::new();
                let parameter_position = structural_parameter_field_path(
                    self.program,
                    self.authored_parameters,
                    subject_expression,
                    &mut path,
                )?;
                let authored_position = usize::try_from(parameter_position).ok()?;
                // Structural argument plans index the dense structural
                // namespace. A borrowed `self` joins it only when the
                // receiver-observation check retains it, a decision made after
                // this emission — so a different subject root positioned after
                // such a receiver can never settle its dense index here.
                let subject_is_self = self
                    .authored_parameters
                    .get(authored_position)
                    .is_some_and(|parameter| parameter.is_self);
                if !subject_is_self
                    && self
                        .authored_parameters
                        .iter()
                        .take(authored_position)
                        .any(|parameter| {
                            parameter.is_self
                                && crate::execution::terminal_unit::is_reference(
                                    self.program,
                                    parameter.type_reference,
                                )
                        })
                {
                    return None;
                }
                let parameter_index = u32::try_from(
                    self.authored_parameters
                        .iter()
                        .take(authored_position)
                        .filter(|parameter| {
                            crate::execution::terminal_unit::structural_parameter_candidate(
                                self.program,
                                parameter,
                            )
                        })
                        .count(),
                )
                .ok()?;
                let path = path
                    .into_iter()
                    .map(|segment| match segment {
                        CheckedStructuralPredicatePathSegment::Field(identity) => {
                            Some(CheckedUnitStructuralPathSegment::Field(identity))
                        }
                        CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                            Some(CheckedUnitStructuralPathSegment::FixedIndex(index))
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                let type_reference = validation::declared_place_type_raw(
                    self.program,
                    machine,
                    Some(state),
                    subject_expression,
                )?;
                let type_identity = self
                    .program
                    .normalized_type_identity(type_reference)
                    .as_str()
                    .to_owned();
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::CaseMembership {
                        source_expression,
                        subject: CheckedScalarComputationStructuralArgument::Place(
                            CheckedUnitStructuralArgumentPlan {
                                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index,
                                },
                                path,
                                type_identity,
                                access: CheckedStructuralAccess::SharedBorrow,
                            },
                        ),
                        case,
                    },
                ))
            }
            MatchCaseSubject::ImmutableLocal {
                symbol,
                type_reference,
            } => {
                let type_identity = self
                    .program
                    .normalized_type_identity(*type_reference)
                    .as_str()
                    .to_owned();
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::CaseMembership {
                        source_expression,
                        subject: CheckedScalarComputationStructuralArgument::Place(
                            CheckedUnitStructuralArgumentPlan {
                                source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                                    symbol: *symbol,
                                },
                                path: Vec::new(),
                                type_identity,
                                access: CheckedStructuralAccess::SharedBorrow,
                            },
                        ),
                        case,
                    },
                ))
            }
            MatchCaseSubject::Constructor => {
                let constructor = self.case_construction(subject_expression)?;
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::CaseMembership {
                        source_expression,
                        subject: CheckedScalarComputationStructuralArgument::Case(constructor),
                        case,
                    },
                ))
            }
        }
    }
}
