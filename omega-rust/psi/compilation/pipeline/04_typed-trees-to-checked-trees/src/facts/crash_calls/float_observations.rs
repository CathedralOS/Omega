//! IEEE observations of completed invocation operands. Copies keep their
//! completion facts and reaching stores; views observe invocation backing.
//! Replaying an initializer in later contexts would turn an earlier copy into
//! a newer value. Source expressions classify literals and representation
//! views only; they never supply missing storage provenance.

use arena::HandleSpan;
use checked_trees::{FlowCallFact, FlowConstraintRef, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactContextHandle, FactPlan};
use numerics::float_semantics::{FloatMeaning, FloatSemantics};
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod operations;

pub(super) struct FloatObservations<'a> {
    program: &'a dyn validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
    semantic: &'a FactPlan,
    flow: &'a FlowFacts,
}

impl<'a> FloatObservations<'a> {
    pub(super) fn new(
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
    ) -> Self {
        Self {
            program,
            operators,
            semantic,
            flow,
        }
    }

    pub(super) fn call_argument(
        &self,
        state: &FlowStateFact,
        call: &FlowCallFact,
        position: usize,
        reference: TypeReferenceHandle,
    ) -> Option<String> {
        let (operands, offset) =
            crate::flow::captured_call_operands(self.program, self.flow, state, call)?;
        let values = self.capture_values(
            state,
            operands.get(position + offset)?,
            reference,
            call.entry_constraints,
            operands.last()?,
        )?;
        observation_spelling(&values)
    }

    /// Preserve every possible completed meaning until the final observation.
    /// IEEE equality alone cannot distinguish the two signed zeros; arithmetic
    /// must carry both rather than treating a representative as exact input.
    fn capture_values(
        &self,
        state: &FlowStateFact,
        operand: &FlowOperandFact,
        reference: TypeReferenceHandle,
        invocation: HandleSpan<FlowConstraintRef>,
        epoch: &FlowOperandFact,
    ) -> Option<Vec<FloatMeaning>> {
        let values = crate::flow::captured_operand_values(
            self.program,
            self.flow,
            self.semantic,
            state,
            operand,
            reference,
            &[],
        )?;
        let values = crate::flow::observe_captured_values(
            self.program,
            self.flow,
            self.semantic,
            state,
            values,
            epoch,
        );
        let mut possibilities = Vec::new();
        for value in values {
            // The observation and its reaching producer are separate namespaces.
            // A later guard can constrain a copy without constraining its source.
            let observed = [
                (
                    value.observation,
                    value.observation_type,
                    &value.observation_projection,
                    crate::flow::captured_observation_places(self.program, self.flow, &value),
                    invocation,
                    epoch,
                ),
                (
                    value.operand,
                    value.type_reference,
                    &value.projection,
                    crate::flow::captured_value_places(self.program, self.flow, &value),
                    value.producer_invocation(invocation),
                    value.producer_epoch.unwrap_or(epoch),
                ),
            ]
            .into_iter()
            .find_map(
                |(capture, reference, projection, places, invocation, epoch)| {
                    let contexts = crate::flow::operand_contexts(
                        self.program,
                        &self.flow.contexts,
                        &self.flow.control,
                        self.semantic,
                        capture,
                        reference,
                        invocation,
                    );
                    let recorded = places.and_then(|places| {
                        let mut possibilities = Vec::new();
                        for place in places {
                            let spelling = crate::values::ieee_equal_spelling_at_place(
                                self.program,
                                self.semantic,
                                contexts
                                    .iter()
                                    .map(|handle| self.semantic.contexts.get(*handle)),
                                &place.projected,
                            )?;
                            let observed = numerics::literals::FloatLiteral::parse(&spelling)?;
                            let observed = FloatMeaning::from_f64(observed.landed_f64());
                            if observed.is_zero() {
                                extend_values(
                                    &mut possibilities,
                                    [
                                        FloatMeaning::Zero { negative: false },
                                        FloatMeaning::Zero { negative: true },
                                    ],
                                );
                            } else {
                                extend_values(&mut possibilities, [observed]);
                            }
                        }
                        (!possibilities.is_empty()).then_some(possibilities)
                    });
                    recorded.or_else(|| {
                        if !projection.remaining.is_empty() {
                            return None;
                        }
                        self.produced(
                            state,
                            capture,
                            projection.expression,
                            invocation,
                            epoch,
                            &contexts,
                        )
                    })
                },
            )?;
            extend_values(&mut possibilities, observed);
        }
        (!possibilities.is_empty()).then_some(possibilities)
    }

    fn produced(
        &self,
        state: &FlowStateFact,
        operand: &FlowOperandFact,
        expression: typed_trees::expression::ExpressionHandle,
        invocation: HandleSpan<FlowConstraintRef>,
        epoch: &FlowOperandFact,
        contexts: &[FactContextHandle],
    ) -> Option<Vec<FloatMeaning>> {
        if let Some(value) = self.operation(state, operand, expression) {
            return Some(value);
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Float(_) => {
                let carrier = self
                    .program
                    .primitive_type_reference(operand.type_reference)?;
                Some(vec![meaning(validation::closed_float_range_endpoint(
                    self.program,
                    expression,
                    carrier,
                )?)])
            }
            ExpressionNode::Name(path) if path.symbol == path.head_symbol => {
                self.constant(path.symbol).map(|value| vec![value])
            }
            ExpressionNode::UndeterminedNaN(format) => {
                let expected = match format {
                    numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                };
                (self
                    .program
                    .primitive_type_reference(operand.type_reference)
                    == Some(expected))
                .then_some(vec![FloatMeaning::NaN])
            }
            ExpressionNode::Match(_) => {
                let mut possibilities = Vec::new();
                for alternative in self.flow.control.operands.span(operand.alternatives)? {
                    let observed = self.capture_values(
                        state,
                        alternative,
                        alternative.type_reference,
                        invocation,
                        epoch,
                    )?;
                    extend_values(&mut possibilities, observed);
                }
                (!possibilities.is_empty()).then_some(possibilities)
            }
            ExpressionNode::Call(authored) => {
                // Expression identity alone is not an invocation: bind its exact
                // state, statement, selected target and authenticated argument roster.
                let mut calls = self
                    .flow
                    .control
                    .calls
                    .span(state.calls)?
                    .iter()
                    .filter(|call| {
                        call.statement_index == operand.statement_index
                            && operand.state_symbol == state.state_symbol
                            && call.authored_expression == expression
                    });
                let call = calls.next()?;
                if calls.next().is_some() || call.target_symbol != authored.target_symbol {
                    return None;
                }
                let (callee, entry) =
                    validation::find_machine_head(self.program, call.target_symbol)?;
                if !callee.supplies_checked_body()
                    || !callee.body_is_present
                    || callee.target_family_interface
                {
                    return None;
                }
                let returned = typed_trees::statement::direct_state_return_values(
                    &self.program.statement_table,
                    self.program
                        .statement_table
                        .statements(entry.statement_nodes),
                )?;
                let mut formal = None;
                for returned in returned {
                    let ExpressionNode::Name(path) =
                        self.program.expression_table.expression(returned)
                    else {
                        return None;
                    };
                    if path.symbol != path.head_symbol
                        || self
                            .program
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            != 1
                        || formal.is_some_and(|known| known != path.symbol)
                    {
                        return None;
                    }
                    formal = Some(path.symbol);
                }
                let formal = formal?;
                let (position, parameter) = self
                    .program
                    .state_parameters(entry)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == formal && !parameter.is_mutable)?;
                let carrier = self
                    .program
                    .primitive_type_reference(parameter.type_reference)?;
                if !carrier.accepts_float_literal()
                    || self.program.primitive_type_reference(entry.return_type) != Some(carrier)
                {
                    return None;
                }
                let (operands, offset) =
                    crate::flow::captured_call_operands(self.program, self.flow, state, call)?;
                self.capture_values(
                    state,
                    operands.get(position + offset)?,
                    parameter.type_reference,
                    call.entry_constraints,
                    operands.last()?,
                )
            }
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                // Borrowing a literal is closed. Storage reads were handled by
                // the captured referent places, never by following a name here.
                self.produced(state, operand, borrow.target, invocation, epoch, contexts)
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => self
                .recast(state, operand, epoch, cast, contexts)
                .map(|value| vec![value]),
            _ => None,
        }
    }

    /// Completed declaration meaning is independent of caller storage and its
    /// current guards. Semantic NaN supplies no chosen representation payload.
    fn constant(&self, symbol: symbols::SymbolHandle) -> Option<FloatMeaning> {
        use language_core::constant_value::EvaluatedConstant;
        use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};
        let declaration = self.program.const_declaration_by_symbol(symbol)?;
        let carrier = self
            .program
            .primitive_type_reference(declaration.declared_type)?;
        let expected = match carrier {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        match declaration.evaluated_value.as_ref()? {
            EvaluatedConstant::UndeterminedNaN(format) if *format == expected => {
                Some(FloatMeaning::NaN)
            }
            EvaluatedConstant::Encoded(encoding) => {
                let DecodedCanonicalConstValue::Float { format, bits } = (CanonicalConstIdentity {
                    type_name: expected.name().to_owned(),
                    encoding: encoding.clone(),
                })
                .decode_declaration_encoding()?
                else {
                    return None;
                };
                if format != expected {
                    return None;
                }
                Some(meaning(match format {
                    numerics::literals::FloatFormat::F32 => {
                        semantic_vocabulary::IeeeFloatValue::Binary32(u32::try_from(bits).ok()?)
                    }
                    numerics::literals::FloatFormat::F64 => {
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits)
                    }
                }))
            }
            _ => None,
        }
    }

    fn recast(
        &self,
        state: &FlowStateFact,
        operand: &FlowOperandFact,
        epoch: &FlowOperandFact,
        cast: &typed_trees::expression::TableCastExpression,
        contexts: &[FactContextHandle],
    ) -> Option<FloatMeaning> {
        let peeled = self
            .program
            .type_reference_table
            .peeled_type_reference(cast.target_type);
        let carrier = match self.program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => peeled,
        };
        let target = self.program.primitive_type_reference(carrier)?;
        if !target.accepts_float_literal() {
            return None;
        }
        let source = match self.program.expression_table.expression(cast.value) {
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            _ => cast.value,
        };
        let (machine, body) = validation::machine_state_by_symbols(
            self.program,
            state.machine_symbol,
            operand.state_symbol,
        )?;
        let source_type =
            self.program
                .primitive_type_reference(validation::expression_result_type_reference(
                    self.program,
                    machine,
                    body,
                    source,
                )?)?;
        if !source_type.is_fixed_integer()
            || source_type.scalar_byte_size() != target.scalar_byte_size()
        {
            return None;
        }
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            operand.state_symbol,
            operand.statement_index,
            source,
        )?;
        // This source place is classified from the recast, not replayed. A
        // completed prefix that could change its binding prevents borrowing an
        // earlier origin for that read; unknown selectors remain unresolved.
        if !crate::flow::operand_entry_origin_is_stable(self.program, self.flow, epoch, &place) {
            return None;
        }
        let facts::ScalarValue::Integer(integer) = crate::values::scalar_value_at_place(
            self.program,
            self.semantic,
            contexts
                .iter()
                .map(|handle| self.semantic.contexts.get(*handle)),
            &place,
        )?
        else {
            return None;
        };
        let admitted =
            crate::values::admitted_integer(crate::values::integer_type(source_type)?, &integer)?;
        let bits = match admitted {
            semantic_vocabulary::IntegerValue::Signed(value) => value as u128,
            semantic_vocabulary::IntegerValue::Unsigned(value) => value,
        };
        Some(match target {
            PrimitiveType::F32 => FloatMeaning::from_f32(f32::from_bits(bits as u32)),
            PrimitiveType::F64 => FloatMeaning::from_f64(f64::from_bits(bits as u64)),
            _ => return None,
        })
    }
}

fn extend_values(
    values: &mut Vec<FloatMeaning>,
    additional: impl IntoIterator<Item = FloatMeaning>,
) {
    for value in additional {
        // Structural meaning keeps opposite zero signs distinct. NaN payloads
        // are not part of this proof-only observation and cannot be invented.
        if !values.contains(&value) {
            values.push(value);
        }
    }
}

fn observation_spelling(values: &[FloatMeaning]) -> Option<String> {
    let first = values.first()?;
    if !values
        .iter()
        .all(|value| FloatSemantics::equal(first, value) || (first.is_nan() && value.is_nan()))
    {
        return None;
    }
    Some(
        numerics::literals::FloatLiteral::from_f64(first.to_f64())
            .text()
            .to_owned(),
    )
}

fn meaning(value: semantic_vocabulary::IeeeFloatValue) -> FloatMeaning {
    match value {
        semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
            FloatMeaning::from_f32(f32::from_bits(bits))
        }
        semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
            FloatMeaning::from_f64(f64::from_bits(bits))
        }
    }
}
