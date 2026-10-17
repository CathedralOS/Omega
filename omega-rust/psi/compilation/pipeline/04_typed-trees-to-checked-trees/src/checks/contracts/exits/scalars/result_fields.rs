//! Result projections use the same typed constructor projection as field-domain
//! checking. Scalar congruence then compares selected computations in one exact
//! place namespace, retaining carrier and overflow policy. It does not replay
//! local initializers or calls. The whole return must preserve its operands:
//! a later sibling field may otherwise invalidate an earlier field's read.
//!
//! A slice stored in a returned record publishes that field's closed extent.
//! The extent is the initializer's geometry, not a second length word on the
//! record. A match publishes one extent only when every arm agrees. Arms with
//! different extents still satisfy a relation when each arm does, and they
//! never publish one arm's extent as the other's. A value arm measures its
//! window after the pattern integer replaces the matched subject, so
//! `match which { 0 -> Route { events: &route[0..which + 1] } }` has length 1
//! in that arm. A binding retains the selected arm and ordered pattern prefix:
//! earlier failed patterns or constructor fields may change the live subject.
//! The shared occurrence query admits the equality only before such effects;
//! later writes cannot invalidate a window already captured.

use validation::CanonicalPlace;

use super::{ExitScalars, ExpressionHandle, ScalarValue, exit_return_expression, stable_segments};
use crate::flow::{self};
use checked_trees::CheckedScalarExpression;
use facts::PlaceRoot;
use typed_trees::expression::{BinaryOperator, ExpressionNode};
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

#[derive(Clone)]
struct MatchedReturnInteger {
    subject: ExpressionHandle,
    value: numerics::bignum::BigInt,
    patterns: Vec<ExpressionHandle>,
    arm: ExpressionHandle,
}

impl ExitScalars<'_, '_> {
    /// Return substitution and call substitution meet in the same scoped solver.
    /// Only live invocation guarantees are premises; the return contract under
    /// proof is never imported. Constructor fields retain their exact values,
    /// and entry operands retain the existing origin/preservation checks.
    pub(super) fn proves_return_arithmetic(&self, expression: ExpressionHandle) -> bool {
        self.return_arithmetic(expression) == Some(true)
    }

    fn return_arithmetic(&self, expression: ExpressionHandle) -> Option<bool> {
        use super::super::super::prover::call_guarantees::{self, arithmetic, callable::Callable};
        let entry = self.program.machine_states(self.machine).first()?;
        let state =
            validation::find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
        let (_, caller) = self.facts.flow.control.states.iter().find(|(_, state)| {
            state.machine_symbol == self.machine.symbol
                && state.state_symbol == self.exit.state_symbol
        })?;
        let goal = arithmetic::proposition(
            self.program,
            self.facts,
            &Callable::Machine {
                machine: self.machine,
                state: entry,
            },
            expression,
            |occurrence, primitive| {
                if let Some(length) = arithmetic::collection_length_value(
                    self.program,
                    &Callable::Machine {
                        machine: self.machine,
                        state: entry,
                    },
                    occurrence,
                    |place, observation| {
                        arithmetic::observation_origin(
                            self.program,
                            self.facts,
                            caller,
                            self.exit.statement_index,
                            self.call_frames?,
                            self.contract_place_at_exit(place)?,
                            observation,
                        )
                    },
                ) {
                    return Some(length);
                }
                if let Some((selected, remaining)) = self.result_projection(occurrence) {
                    if remaining.is_empty() {
                        let value = arithmetic::scalar_term(
                            self.program,
                            self.facts,
                            &Callable::Machine {
                                machine: self.machine,
                                state,
                            },
                            selected,
                            |value, carrier| {
                                let place = call_guarantees::direct_place(self.program, value)?;
                                let place = arithmetic::value_origin(
                                    self.program,
                                    self.facts,
                                    caller,
                                    self.exit.statement_index,
                                    self.call_frames?,
                                    place,
                                )?;
                                Some(arithmetic::atom(place, carrier))
                            },
                        )?;
                        return Some(value);
                    }
                    let mut place = call_guarantees::direct_place(self.program, selected)?;
                    place.extend_segments(&remaining);
                    let place = arithmetic::value_origin(
                        self.program,
                        self.facts,
                        caller,
                        self.exit.statement_index,
                        self.call_frames?,
                        place,
                    )?;
                    return Some(arithmetic::atom(place, primitive));
                }
                let mut value = crate::values::lower_unit_scalar_argument(
                    self.program,
                    &self.facts.operators,
                    entry,
                    0,
                    occurrence,
                    primitive,
                )?;
                let mut subjects = Vec::new();
                self.bind_current_scalar(&mut value, entry, true, &mut subjects)?;
                let CheckedScalarExpression::Parameter { position, .. } = value else {
                    return None;
                };
                // A reference formal denotes current storage, unlike an
                // owned input's preserved invocation value. Reconstruct this
                // read at the exit frontier too: a saved pre-write read may
                // otherwise have the same place spelling as new contents.
                let place = arithmetic::value_origin(
                    self.program,
                    self.facts,
                    caller,
                    self.exit.statement_index,
                    self.call_frames?,
                    subjects.get(position)?.clone(),
                )?;
                Some(arithmetic::atom(place, primitive))
            },
        )?;
        Some(arithmetic::proves_at_exit(
            self.program,
            self.facts,
            caller,
            self.exit.statement_index,
            self.contexts,
            self.call_frames?,
            &goal,
        ))
    }

    pub(super) fn result_projection(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
        let place = validation::reserved_result_place(self.program, expression)?;
        if place.machine_symbol != self.machine.symbol || !stable_segments(&place.segments) {
            return None;
        }
        let returned = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        let entry = self.program.machine_states(self.machine).first()?;
        if place.segments.is_empty() {
            let reference = validation::field_domains::readable_type_reference(
                self.program,
                entry.return_type,
            )?;
            return self
                .program
                .primitive_type_reference(reference)
                .is_some()
                .then_some((returned, Vec::new()));
        }
        let mut projections = flow::literal_expression_projections(
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
        let state =
            validation::find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
        // Descriptor metadata is a value projection too. Rebase the exact
        // reserved result onto the returned view; the shared geometry query
        // retains captured closed values and excludes later live hypotheses.
        if let Some(receiver) = validation::collection_length_receiver(
            self.program,
            self.machine,
            Some(state),
            expression,
        ) {
            if let Some(length) = self.agreed_result_collection_length(state, receiver) {
                return Some(ScalarValue::Integer(numerics::bignum::BigInt::from_u64(
                    length,
                )));
            }
            // The whole returned descriptor keeps its previous route when the
            // field projection does not apply. A record field is not that route.
            if validation::reserved_result_owner(self.program, receiver)
                .is_some_and(|(owner, _)| owner == self.machine.symbol)
            {
                let returned = exit_return_expression(self.program, self.exit);
                if !self.return_expression_is_stable(returned) {
                    return None;
                }
                return validation::closed_collection_length(
                    self.program,
                    self.machine,
                    state,
                    returned,
                )
                .map(numerics::bignum::BigInt::from_u64)
                .map(ScalarValue::Integer);
            }
        }
        let (value, remaining) = self.result_projection(expression)?;
        if remaining.is_empty() {
            let reference =
                validation::reserved_result_place(self.program, expression)?.type_reference;
            // The selected constructor child may be a readable reference.
            // Observe its referent, not the descriptor's pointer bits; the
            // capture lookup below retains its actual source.
            let reference =
                validation::field_domains::readable_type_reference(self.program, reference)?;
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
        // The completed result owns its evaluated children. A stored reference
        // child names its captured backing, not the descriptor slot's contents;
        // a detached child keeps its own completion epoch instead. Observe
        // that retained operand rather than replaying a local initializer.
        if !self
            .facts
            .flow
            .control
            .operands
            .is_valid(self.exit.returned_value)
        {
            return None;
        }
        let capture = self
            .facts
            .flow
            .control
            .operands
            .get(self.exit.returned_value);
        if capture.expression != exit_return_expression(self.program, self.exit)
            || capture.state_symbol != self.exit.state_symbol
            || capture.statement_index != self.exit.statement_index
        {
            return None;
        }
        let (_, state) = self.facts.flow.control.states.iter().find(|(_, state)| {
            state.machine_symbol == self.machine.symbol
                && state.state_symbol == self.exit.state_symbol
        })?;
        let place = validation::reserved_result_place(self.program, expression)?;
        super::super::super::prover::captured_scalar_at_path(
            self.program,
            self.facts,
            state,
            capture,
            &place.segments,
            self.exit.statement_index.checked_add(1)?,
            self.exit.entry_constraints,
            self.call_frames?,
        )
    }

    pub(super) fn proves_result_field_equality(&self, expression: ExpressionHandle) -> bool {
        self.result_field_equality(expression) == Some(true)
    }

    /// `result.field.len` compared with a literal. Every constructed arm must
    /// establish the same relation. Disagreeing extents do not become one value.
    pub(super) fn proves_result_collection_length(&self, expression: ExpressionHandle) -> bool {
        self.result_collection_length_relation(expression) == Some(true)
    }

    fn result_collection_length_relation(&self, expression: ExpressionHandle) -> Option<bool> {
        let comparison = self.program.expression_table.binary_node(expression)?;
        let state =
            validation::find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
        let (receiver, expected, operator) = self.length_comparison(state, comparison)?;
        let lengths = self.result_collection_lengths(state, receiver)?;
        if lengths.is_empty() {
            return None;
        }
        Some(lengths.iter().all(|length| {
            let length = numerics::bignum::BigInt::from_u64(*length);
            Self::length_relation(&length, operator, &expected)
        }))
    }

    fn length_comparison(
        &self,
        state: &State,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<(ExpressionHandle, numerics::bignum::BigInt, BinaryOperator)> {
        let left_length = validation::collection_length_receiver(
            self.program,
            self.machine,
            Some(state),
            comparison.left,
        );
        let right_length = validation::collection_length_receiver(
            self.program,
            self.machine,
            Some(state),
            comparison.right,
        );
        let (receiver, expected, operator) = if left_length.is_some() {
            (
                left_length?,
                self.integer_literal(comparison.right)?,
                comparison.operator,
            )
        } else if right_length.is_some() {
            (
                right_length?,
                self.integer_literal(comparison.left)?,
                Self::flipped_relation(comparison.operator)?,
            )
        } else {
            return None;
        };
        matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        )
        .then_some((receiver, expected, operator))
    }

    fn integer_literal(&self, expression: ExpressionHandle) -> Option<numerics::bignum::BigInt> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Integer(value) => value.value_bignum(),
            _ => None,
        }
    }

    fn flipped_relation(operator: BinaryOperator) -> Option<BinaryOperator> {
        Some(match operator {
            BinaryOperator::Equal => BinaryOperator::Equal,
            BinaryOperator::NotEqual => BinaryOperator::NotEqual,
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            _ => return None,
        })
    }

    fn length_relation(
        length: &numerics::bignum::BigInt,
        operator: BinaryOperator,
        expected: &numerics::bignum::BigInt,
    ) -> bool {
        match operator {
            BinaryOperator::Equal => length == expected,
            BinaryOperator::NotEqual => length != expected,
            BinaryOperator::Less => length < expected,
            BinaryOperator::LessOrEqual => length <= expected,
            BinaryOperator::Greater => length > expected,
            BinaryOperator::GreaterOrEqual => length >= expected,
            _ => false,
        }
    }

    /// One closed extent when every arm of this result projection agrees.
    fn agreed_result_collection_length(
        &self,
        state: &State,
        receiver: ExpressionHandle,
    ) -> Option<u64> {
        let lengths = self.result_collection_lengths(state, receiver)?;
        let first = *lengths.first()?;
        lengths
            .iter()
            .all(|length| *length == first)
            .then_some(first)
    }

    /// Closed extents of the slice selected by `receiver` on this exit.
    /// A match contributes every arm. A record field contributes that field's
    /// initializer. The record itself is not a descriptor.
    fn result_collection_lengths(
        &self,
        state: &State,
        receiver: ExpressionHandle,
    ) -> Option<Vec<u64>> {
        let segments = self.result_receiver_segments(receiver)?;
        if !stable_segments(&segments) {
            return None;
        }
        let returned = exit_return_expression(self.program, self.exit);
        let entry = self.program.machine_states(self.machine).first()?;
        self.collection_lengths_through(state, returned, entry.return_type, &segments, &[])
    }

    /// Field path from this machine's reserved result to the length receiver.
    /// An intermediate `result.field` node is not its own contract root, so the
    /// result name owns the path and each field is resolved on the result type.
    fn result_receiver_segments(
        &self,
        receiver: ExpressionHandle,
    ) -> Option<Vec<facts::PlaceSegment>> {
        let mut members = Vec::new();
        let mut cursor = receiver;
        loop {
            if !self.program.expression_table.expression_is_valid(cursor) || members.len() >= 128 {
                return None;
            }
            match self.program.expression_table.expression(cursor) {
                ExpressionNode::Member(member) if member.case_variant.is_none() => {
                    members.push(member);
                    cursor = member.receiver;
                }
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        let (owner, mut type_reference) = validation::reserved_result_owner(self.program, cursor)?;
        if owner != self.machine.symbol {
            return None;
        }
        let mut segments = Vec::with_capacity(members.len());
        for member in members.into_iter().rev() {
            let field = self.field_of_type(type_reference, member)?;
            segments.push(facts::PlaceSegment::Field {
                symbol: field.symbol,
            });
            type_reference = field.type_reference;
        }
        Some(segments)
    }

    fn field_of_type(
        &self,
        type_reference: typed_trees::types::TypeReferenceHandle,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> Option<&typed_trees::data::DataField> {
        let mut reference = type_reference;
        loop {
            match self.program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                | typed_trees::types::TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
                    return self.field_of_data(*base_symbol, member);
                }
                typed_trees::types::TypeReferenceNode::Named { symbol, .. } => {
                    return self.field_of_data(*symbol, member);
                }
                _ => return None,
            }
        }
    }

    fn field_of_data(
        &self,
        symbol: symbols::SymbolHandle,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> Option<&typed_trees::data::DataField> {
        let mut definitions = self
            .program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == symbol);
        let data = definitions.next()?;
        if definitions.next().is_some() {
            return None;
        }
        self.program
            .data_members(data)
            .iter()
            .find_map(|candidate| {
                let field = candidate.as_field()?;
                let selected = if member.member_symbol.is_valid() {
                    field.symbol == member.member_symbol
                } else {
                    field.name.as_str() == member.member.as_str()
                };
                selected.then_some(field)
            })
    }

    fn collection_lengths_through(
        &self,
        state: &State,
        expression: ExpressionHandle,
        reference: typed_trees::types::TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
        matched: &[MatchedReturnInteger],
    ) -> Option<Vec<u64>> {
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression)
        {
            let arms = self.program.expression_table.match_arms(dispatch.arms);
            if arms.is_empty() {
                return None;
            }
            let mut lengths = Vec::new();
            let mut patterns = Vec::new();
            for arm in arms {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    patterns.push(pattern);
                }
                let mut arm_matched = matched.to_vec();
                if let Some((subject, value)) =
                    self.matched_arm_integer(dispatch.subject, arm.pattern)
                {
                    arm_matched.push(MatchedReturnInteger {
                        subject,
                        value,
                        patterns: patterns.clone(),
                        arm: arm.value,
                    });
                }
                lengths.extend(self.collection_lengths_through(
                    state,
                    arm.value,
                    reference,
                    segments,
                    &arm_matched,
                )?);
            }
            return (!lengths.is_empty()).then_some(lengths);
        }
        if segments.is_empty() {
            if !self.return_expression_is_stable(expression) {
                return None;
            }
            // A projected field is evaluated within its whole constructor.
            // Its own inert subtree cannot authenticate an earlier match value
            // after a failed pattern or preceding field changed the subject.
            let current_matches: Vec<_> = matched
                .iter()
                .filter(|binding| {
                    validation::expression_traversal::expression_has_inert_prefix(
                        self.program,
                        &binding.patterns,
                        binding.arm,
                        expression,
                    )
                })
                .map(|binding| (binding.subject, binding.value.clone()))
                .collect();
            return validation::closed_collection_length_under_matches(
                self.program,
                self.machine,
                state,
                expression,
                &current_matches,
            )
            .map(|length| vec![length]);
        }
        let projections = flow::literal_expression_projections(
            self.program,
            expression,
            reference,
            segments,
            false,
        )?;
        if projections.is_empty() {
            return None;
        }
        let mut lengths = Vec::new();
        for projection in projections {
            if !projection.remaining.is_empty() {
                return None;
            }
            lengths.extend(self.collection_lengths_through(
                state,
                projection.expression,
                reference,
                &[],
                matched,
            )?);
        }
        (!lengths.is_empty()).then_some(lengths)
    }

    /// The integer a value pattern contributes to its arm. A wildcard learns
    /// nothing. A non-integer pattern contributes nothing here; the arm still
    /// uses whatever extent its window has without that subject.
    fn matched_arm_integer(
        &self,
        subject: ExpressionHandle,
        pattern: typed_trees::expression::MatchPattern,
    ) -> Option<(ExpressionHandle, numerics::bignum::BigInt)> {
        let typed_trees::expression::MatchPattern::Value(pattern) = pattern else {
            return None;
        };
        let value = self.integer_literal(pattern)?;
        Some((subject, value))
    }

    fn result_field_equality(&self, expression: ExpressionHandle) -> Option<bool> {
        let Some(comparison) = self.program.expression_table.binary_node(expression) else {
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
        let state =
            validation::find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
        let reference = validation::reserved_result_place(self.program, projected)?.type_reference;
        let observed_reference =
            validation::field_domains::readable_type_reference(self.program, reference)?;
        let primitive = self.program.primitive_type_reference(observed_reference)?;
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
            let mut place = validation::canonical_place_from_expression_in_state(
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
                            let local = node.as_local_data()?;
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
                | Scalar::IntegerExactCast { operand, .. } => {
                    pending.push(operand.as_mut());
                    continue;
                }
                _ => return None,
            };
            if contract {
                let root = place.root.as_symbol()?;
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == root)?;
                if parameter.is_const {
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
                let Some(transition) = statement.as_transition() else { return false; };
                [transition.target, transition.continuation].iter().any(|target| {
                    target.is_valid() && match self.program.statement_table.transition_target(*target) {
                        typed_trees::statement::TransitionTargetNode::Named { path, .. } =>
                            path.symbol == entry.symbol || path.symbol == self.machine.symbol,
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
                validation::expression_traversal::collect_statement_expression_trees(
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
                if let Some(call) = statement.as_call() {
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
