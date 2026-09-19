//! Result projections use the same typed constructor projection as field-domain
//! checking. Scalar congruence then compares selected computations in one exact
//! place namespace, retaining carrier and overflow policy. It does not replay
//! local initializers or calls. The whole return must preserve its operands:
//! a later sibling field may otherwise invalidate an earlier field's read.

use super::{ExitScalars, ExpressionHandle, ScalarValue, exit_return_expression, stable_segments};
use crate::flow::{self, CanonicalPlace};
use checked_trees::CheckedScalarExpression;
use facts::PlaceRoot;
use typed_trees::expression::{BinaryOperator, ExpressionNode};
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

impl ExitScalars<'_, '_> {
    fn result_projection(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
        let place = validation::reserved_result_place(self.program, expression)?;
        if place.machine_symbol != self.machine.symbol
            || place.segments.is_empty()
            || !stable_segments(&place.segments)
        {
            return None;
        }
        let returned = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        let entry = self.program.machine_states(self.machine).first()?;
        let mut projections = flow::literal_value_projections(
            self.program,
            returned,
            entry.return_type,
            &place.segments,
            false,
        )?;
        let selected = projections.pop()?;
        projections
            .is_empty()
            .then_some((selected.expression, selected.remaining))
    }

    pub(super) fn result_field_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        let (value, remaining) = self.result_projection(expression)?;
        let state = crate::semantic_calls::find_state_in_machine(
            self.program,
            self.machine.symbol,
            self.exit.state_symbol,
        )?;
        if remaining.is_empty() {
            let reference =
                validation::reserved_result_place(self.program, expression)?.type_reference;
            let primitive = self.program.primitive_type_reference(reference)?;
            if !validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(state),
                value,
            ) {
                return None;
            }
            if let Some(selected) = crate::values::lower_unit_scalar_argument(
                self.program,
                &self.facts.operators,
                state,
                self.exit.statement_index,
                value,
                primitive,
            ) {
                // Closed leaves require no operand substitution. Current-place
                // reads below use flow values instead of source initializers.
                if let Some(value) = crate::values::evaluate_checked_scalar(
                    &selected,
                    &mut crate::values::BoundScalarValues {
                        symbols: &[],
                        value_at_symbol: |_| None,
                    },
                ) {
                    return Some(value);
                }
            }
        }
        let mut place = flow::canonical_place_from_expression_in_state(
            self.program,
            self.exit.state_symbol,
            self.exit.statement_index,
            value,
        )?;
        place.extend_segments(&remaining);
        self.value_at_place(&place)
    }

    pub(super) fn proves_result_field_equality(&self, expression: ExpressionHandle) -> bool {
        self.result_field_equality(expression) == Some(true)
    }

    fn result_field_equality(&self, expression: ExpressionHandle) -> Option<bool> {
        let ExpressionNode::Binary(comparison) =
            self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if comparison.operator != BinaryOperator::Equal {
            return None;
        }
        let (selected, projected, expected) =
            if let Some(selected) = self.result_projection(comparison.left) {
                (selected, comparison.left, comparison.right)
            } else {
                (
                    self.result_projection(comparison.right)?,
                    comparison.right,
                    comparison.left,
                )
            };
        let entry = self.program.machine_states(self.machine).first()?;
        let state = crate::semantic_calls::find_state_in_machine(
            self.program,
            self.machine.symbol,
            self.exit.state_symbol,
        )?;
        let reference = validation::reserved_result_place(self.program, projected)?.type_reference;
        let primitive = self.program.primitive_type_reference(reference)?;
        let expected_reference = validation::expression_result_type_reference(
            self.program,
            self.machine,
            entry,
            expected,
        );
        let operand_types = if projected == comparison.left {
            [Some(reference), expected_reference]
        } else {
            [expected_reference, Some(reference)]
        };
        // IEEE equality is not reflexive. Boolean compound denotation has a
        // separate owner; this path proves integer computation congruence.
        if !primitive.accepts_integer_literal()
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.machine.symbol,
                expression,
                language_core::OperatorSpelling::Equal,
                &operand_types,
            )
            || !validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(entry),
                expected,
            )
            || !validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(state),
                selected.0,
            )
        {
            return None;
        }
        let mut expected = crate::values::lower_unit_scalar_argument(
            self.program,
            &self.facts.operators,
            entry,
            0,
            expected,
            primitive,
        )?;
        let mut subjects = Vec::new();
        self.bind_current_scalar(&mut expected, entry, true, &mut subjects)?;
        let mut actual = if selected.1.is_empty() {
            crate::values::lower_unit_scalar_argument(
                self.program,
                &self.facts.operators,
                state,
                self.exit.statement_index,
                selected.0,
                primitive,
            )?
        } else {
            let mut place = flow::canonical_place_from_expression_in_state(
                self.program,
                state.symbol,
                self.exit.statement_index,
                selected.0,
            )?;
            place.extend_segments(&selected.1);
            return Some(expected == intern_place(place, primitive, &mut subjects));
        };
        self.bind_current_scalar(&mut actual, state, false, &mut subjects)?;
        Some(actual == expected)
    }

    fn bind_current_scalar(
        &self,
        expression: &mut CheckedScalarExpression,
        state: &State,
        contract: bool,
        subjects: &mut Vec<CanonicalPlace>,
    ) -> Option<()> {
        use CheckedScalarExpression as Scalar;
        let parameters = self.program.state_parameters(state);
        let scalar_parameters = || {
            parameters.iter().filter(|parameter| {
                !parameter.relevance.is_erased()
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
            })
        };
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            let (mut place, primitive) = match expression {
                Scalar::Parameter {
                    position,
                    primitive_type,
                } => (
                    flow::canonical_place_from_symbol(scalar_parameters().nth(*position)?.symbol)?,
                    *primitive_type,
                ),
                Scalar::StructuralParameterField {
                    parameter_position,
                    path,
                    primitive_type,
                } => {
                    let (symbol, segments, _, _) =
                        crate::values::resolve_structural_parameter_path(
                            self.program,
                            parameters,
                            *parameter_position,
                            path,
                        )?;
                    (
                        CanonicalPlace {
                            root: PlaceRoot::Symbol(symbol),
                            segments,
                        },
                        *primitive_type,
                    )
                }
                Scalar::StorageRead {
                    symbol,
                    primitive_type,
                } => (flow::canonical_place_from_symbol(*symbol)?, *primitive_type),
                Scalar::Local {
                    position,
                    primitive_type,
                } if !contract => {
                    let ordinal = position.checked_sub(scalar_parameters().count())?;
                    let local = self
                        .program
                        .statement_table
                        .statements(state.statement_nodes)
                        .get(..self.exit.statement_index)?
                        .iter()
                        .filter_map(|node| {
                            let typed_trees::statement::StatementNode::LocalData(local) = node
                            else {
                                return None;
                            };
                            (!local.is_mutable
                                && local.initial_value.is_valid()
                                && self
                                    .program
                                    .primitive_type_reference(local.type_reference)
                                    .is_some())
                            .then_some(local)
                        })
                        .nth(ordinal)?;
                    (
                        flow::canonical_place_from_symbol(local.symbol)?,
                        *primitive_type,
                    )
                }
                Scalar::IntegerLiteral { .. } => continue,
                Scalar::IntegerBinary { left, right, .. } => {
                    pending.extend([left.as_mut(), right.as_mut()]);
                    continue;
                }
                Scalar::IntegerBitwiseNot { operand, .. }
                | Scalar::IntegerWiden { operand, .. }
                | Scalar::IntegerExactCast { operand, .. }
                | Scalar::IntegerWrappingCast { operand, .. }
                | Scalar::IntegerTrappingCast { operand, .. } => {
                    pending.push(operand.as_mut());
                    continue;
                }
                _ => return None,
            };
            if contract {
                let PlaceRoot::Symbol(root) = place.root else {
                    return None;
                };
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == root)?;
                if parameter.is_self || parameter.is_const {
                    return None;
                }
                let mut origins = self
                    .facts
                    .flow
                    .control
                    .exit_parameter_origins
                    .span_or_empty(self.exit.parameter_origins)
                    .iter()
                    .filter(|origin| {
                        origin.contract == self.contract && origin.entry_parameter == root
                    });
                let origin = origins.next()?;
                if origins.next().is_some() || !origin.state_parameter.is_valid() {
                    return None;
                }
                place.root = PlaceRoot::Symbol(origin.state_parameter);
                let mut reference = parameter.type_reference;
                while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                    self.program.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                // Reference formals denote current referent contents, not an
                // entry snapshot. The origin row preserves their reference
                // identity; whole-return stability fixes the read revision.
                // Owned input values additionally need preservation before
                // comparing them with caller-substituted invocation inputs.
                if !matches!(
                    self.program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    // Mutable owned scalar inputs have no tracked arrival
                    // origin across entry backedges. Storage identity cannot
                    // substitute for the original invocation value.
                    if parameter.is_mutable {
                        return None;
                    }
                    self.preserved_contract_subject(&place)?;
                }
            }
            *expression = intern_place(place, primitive, subjects);
        }
        Some(())
    }

    fn preserved_contract_subject(&self, subject: &CanonicalPlace) -> Option<()> {
        let entry = self.program.machine_states(self.machine).first()?;
        let frames = self.call_frames?;
        // Owned-record origins are not tracked across arrival edges yet. The
        // entry exit's identity row alone cannot identify an invocation's
        // original record after a loop re-enters with different arguments.
        if !subject.segments.is_empty() && self.program.machine_states(self.machine).iter().any(|state| {
            self.program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
                let typed_trees::statement::StatementNode::Transition(transition) = statement else { return false; };
                [transition.target, transition.continuation].iter().any(|target| {
                    target.is_valid() && match self.program.statement_table.transition_target(*target) {
                        typed_trees::statement::TransitionTargetNode::Named { path, .. } =>
                            path.symbol == entry.symbol || path.symbol == self.machine.symbol,
                        typed_trees::statement::TransitionTargetNode::SelfTarget => state.symbol == entry.symbol,
                        _ => false,
                    }
                })
            })
        }) { return None; }
        // Cross-state scalar origins preserve identity, but do not yet retain
        // a field-specific write history. Require parameter preservation in
        // every state in that case, never replay a same-spelled entry binding.
        for state in self.program.machine_states(self.machine) {
            let statements = self
                .program
                .statement_table
                .statements(state.statement_nodes);
            let prefix = if state.symbol == self.exit.state_symbol {
                statements.get(..self.exit.statement_index)?
            } else if entry.symbol == self.exit.state_symbol {
                continue;
            } else {
                statements
            };
            for (ordinal, statement) in prefix.iter().enumerate() {
                // Ordinary call frames do not summarize atomic destinations or
                // selected operator implementations. Unknown writes cannot
                // preserve an invocation input merely by lacking a frame row.
                let mut nodes = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    self.program,
                    statement,
                    &mut nodes,
                );
                if matches!(
                    statement,
                    typed_trees::statement::StatementNode::AssemblyFact(_)
                ) || nodes.iter().any(|node| {
                    matches!(
                        self.program.expression_table.expression(*node),
                        ExpressionNode::Atomic(_)
                    )
                }) || self.facts.operators.uses.iter().any(|(_, selected)| {
                    nodes.contains(&selected.expression)
                        && selected.status
                            != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                }) {
                    return None;
                }
                let mut writes = flow::statement_storage_writes(
                    self.program,
                    self.machine.symbol,
                    state.symbol,
                    ordinal,
                    statement,
                    self.call_frames,
                )?;
                let mut value_frames =
                    vec![frames.statement_value_write_frame(self.machine, statement)];
                if let typed_trees::statement::StatementNode::Call(call) = statement {
                    value_frames.push(frames.may_write_frame(self.machine, call));
                }
                for frame in value_frames {
                    writes.extend(flow::frame_storage_writes(
                        self.program,
                        self.machine.symbol,
                        state.symbol,
                        ordinal,
                        &frame,
                        Some(frames),
                    )?);
                }
                for write in writes {
                    let overlaps = if entry.symbol == self.exit.state_symbol {
                        write.root == subject.root
                            && flow::canonical_place_segments_may_overlap(
                                self.program,
                                &write.segments,
                                &subject.segments,
                            )
                    } else {
                        self.program
                            .state_parameters(state)
                            .iter()
                            .any(|parameter| write.root == PlaceRoot::Symbol(parameter.symbol))
                    };
                    if overlaps {
                        return None;
                    }
                }
            }
        }
        Some(())
    }
}

fn intern_place(
    place: CanonicalPlace,
    primitive_type: PrimitiveType,
    subjects: &mut Vec<CanonicalPlace>,
) -> CheckedScalarExpression {
    let position = subjects
        .iter()
        .position(|candidate| *candidate == place)
        .unwrap_or_else(|| {
            let position = subjects.len();
            subjects.push(place);
            position
        });
    CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    }
}
