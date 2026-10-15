//! Descriptor observations use the same captured roster as scalar values.
//! A range's endpoints are captured when the descriptor is formed; later
//! endpoint stores cannot enlarge that descriptor. Construction supplies no
//! bounds authority: selected builtin meaning and the current hypotheses must
//! independently establish the range. Returning a descriptor binds its actual
//! observation to the exact contract owner, never to the backing capacity.

use super::{
    CurrentScalarValues, Length, LengthSubject, admitted, fixed_length, integer_carrier, scoped,
};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};

impl CurrentScalarValues<'_> {
    pub(crate) fn postcondition(
        &mut self,
        expression: ExpressionHandle,
        returned: ExpressionHandle,
        origins: impl Iterator<Item = (SymbolHandle, SymbolHandle)>,
    ) -> Option<ScopedArithmeticExpression> {
        if let Some(value) = self.collection_length(returned, 0) {
            let entry = self.program.machine_states(self.machine).first()?;
            self.lengths.push(Length {
                subject: LengthSubject::Result(self.machine.symbol),
                name: String::new(),
                stable: true,
                type_reference: entry.return_type,
                value: Some(value),
            });
        }
        // The declaration's postcondition names invocation parameters, while
        // the live hypotheses name the exit state's parameters. Bind those
        // namespaces through the independently established incoming origins;
        // spelling, slot order and a write-free body are not identity evidence.
        let entry = self.program.machine_states(self.machine).first()?;
        let mut bindings = self.current.clone();
        let mut lengths = self.lengths.clone();
        if entry.symbol != self.state.symbol {
            bindings.clear();
            lengths.retain(|length| matches!(length.subject, LengthSubject::Result(_)));
            for (entry_parameter, state_parameter) in origins {
                if let Some(current) = self
                    .current
                    .iter()
                    .find(|binding| binding.symbol == state_parameter)
                {
                    let mut binding = current.clone();
                    binding.symbol = entry_parameter;
                    binding.name = self.program.symbols.name(entry_parameter).to_owned();
                    bindings.push(binding);
                }
                if let Some(current) = self
                    .lengths
                    .iter()
                    .find(|length| length.subject == LengthSubject::Storage(state_parameter))
                {
                    let mut length = current.clone();
                    length.subject = LengthSubject::Storage(entry_parameter);
                    length.name = self.program.symbols.name(entry_parameter).to_owned();
                    lengths.push(length);
                }
            }
        }
        if !admitted(
            self.program,
            self.machine,
            entry,
            expression,
            &bindings,
            &lengths,
            0,
        ) {
            return None;
        }
        let mut goal = scoped(
            self.machine,
            entry,
            expression,
            &bindings,
            &lengths,
            self.program,
        )?;
        if let Some(returned) = self.current(returned) {
            goal.bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Result,
                value: ScopedArithmeticValue::Term(returned),
            });
        }
        Some(goal)
    }

    pub(super) fn capture_collection(
        &mut self,
        symbol: SymbolHandle,
        name: &str,
        type_reference: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement: usize,
    ) {
        let value = self.collection_length(expression, 0);
        self.lengths
            .retain(|length| length.subject != LengthSubject::Storage(symbol));
        // Only descriptors of reference collection types participate. Bounded
        // owners have live counts, not a frozen length equal to their capacity.
        if fixed_length(self.program, type_reference, String::new).is_some() {
            self.lengths.push(Length {
                subject: LengthSubject::Storage(symbol),
                name: name.to_owned(),
                stable: false,
                type_reference,
                // Completing a descriptor always produces its own length
                // observation, even when no endpoint equation is available.
                // A later guard may constrain that observation without gaining
                // any unproved relationship to its backing or source values.
                value: Some(value.unwrap_or_else(|| ScopedArithmeticValue::Atom {
                    identity: format!(
                        "state:{:?}:store:{statement}:descriptor:{symbol:?}:occurrence:{expression:?}:length",
                        self.state.symbol
                    ),
                    unsigned: true,
                })),
            });
        }
    }

    pub(crate) fn byte_length_bound(&self, expression: ExpressionHandle) -> Option<u64> {
        let value = self.collection_length(expression, 0)?;
        let interval = validation::scoped_arithmetic_value_bounds(
            self.program,
            self.machine,
            &self.hypotheses,
            &value,
        )?;
        interval.high?.to_u64()
    }

    pub(super) fn collection_length(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        if depth >= 64
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => self.collection_length(borrow.target, depth + 1),
            ExpressionNode::Name(name) if name.symbol == name.head_symbol => self
                .lengths
                .iter()
                .find(|length| length.subject == LengthSubject::Storage(name.symbol))
                .and_then(|length| length.value.clone())
                .or_else(|| self.declared_extent(expression)),
            ExpressionNode::Indexed(indexed)
                if validation::has_builtin_subslice_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                ) =>
            {
                let range = self.program.expression_table.range_node(indexed.index)?;
                let source = self.collection_length(indexed.collection, depth + 1)?;
                let zero = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::zero());
                let start = if range.start.is_valid() {
                    self.capture(self.program, self.machine, self.state, range.start)?
                } else {
                    zero.clone()
                };
                let authored_end = if range.end.is_valid() {
                    self.capture(self.program, self.machine, self.state, range.end)?
                } else {
                    source.clone()
                };
                let end = if range.end_inclusive {
                    if !range.end.is_valid() {
                        return None;
                    }
                    ScopedArithmeticValue::IntegerOperation {
                        operator: BinaryOperator::Add,
                        left: Box::new(authored_end.clone()),
                        right: Box::new(ScopedArithmeticValue::Integer(
                            numerics::bignum::BigInt::from_u64(1),
                        )),
                    }
                } else {
                    authored_end.clone()
                };
                for (left, right) in [
                    (&zero, &start),
                    (&start, &end),
                    (&end, &source),
                    (&zero, &authored_end),
                ] {
                    if !self.proves_values_relation(
                        self.program,
                        self.machine,
                        BinaryOperator::LessOrEqual,
                        left,
                        right,
                    ) {
                        return None;
                    }
                }
                Some(ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Subtract,
                    left: Box::new(end),
                    right: Box::new(start),
                })
            }
            ExpressionNode::Call(call) => {
                let parameters =
                    validation::call_target_parameters(self.program, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                if parameters.len() != arguments.len() {
                    return None;
                }
                let mut bindings = Vec::new();
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let value = if integer_carrier(self.program, parameter.type_reference).is_some()
                    {
                        self.capture(self.program, self.machine, self.state, *argument)
                    } else if !parameter.is_mutable
                        && fixed_length(self.program, parameter.type_reference, String::new)
                            .is_some()
                    {
                        self.collection_length(*argument, depth + 1)
                    } else {
                        None
                    };
                    if let Some(value) = value {
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(*argument),
                            value,
                        });
                    }
                }
                Some(ScopedArithmeticValue::CollectionLength(
                    ScopedArithmeticExpression {
                        owner: self.state.symbol,
                        expression,
                        bindings,
                    },
                ))
            }
            _ => super::field_collection_length(self.program, self.machine, self.state, expression)
                .or_else(|| self.declared_extent(expression)),
        }
    }

    fn declared_extent(&self, expression: ExpressionHandle) -> Option<ScopedArithmeticValue> {
        let mut reference = validation::declared_place_type_raw(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        )?;
        // Do not peel predicate qualifications: a qualified byte owner may
        // have a live count smaller than its fixed backing capacity.
        while let TypeReferenceNode::Reference { referee, .. } =
            self.program.type_reference_table.type_reference(reference)
        {
            reference = *referee;
        }
        if !matches!(
            self.program.type_reference_table.type_reference(reference),
            TypeReferenceNode::FixedArray { .. }
        ) {
            return None;
        }
        let extent = validation::declared_collection_extent(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        )?;
        Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(extent).ok()?),
        ))
    }
}
