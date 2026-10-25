//! Read independently checked result guarantees in the caller's captured
//! arithmetic namespace. The call owner supplies occurrence identities and
//! selected public clauses; it never evaluates a body or imports the contract
//! currently being established. Frame preservation and operand formation stay
//! with the source owner. This adapter transports values, not write authority.

use super::super::scoped_arithmetic::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue, calls::ScopedCalls, install_hypotheses,
};
use super::{Engine, Polynomial};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

impl Engine<'_> {
    /// Prepare call results before normalization while keeping every actual in
    /// the original caller frame. Imported hypotheses are normalized values;
    /// their clause namespace must not replace the next operand's bindings.
    pub(in crate::proof_contracts::contract_entailment) fn capture_expression_calls(
        &mut self,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(symbols) = self.strict_symbol_bindings.as_ref() else {
            return false;
        };
        let mut bindings = Vec::new();
        let mut remaining = 4096usize;
        for (symbol, value) in symbols {
            let Some(value) = captured_polynomial(value, &self.unsigned_atoms, &mut remaining)
            else {
                return false;
            };
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(*symbol),
                value,
            });
        }
        for (occurrence, value) in &self.strict_occurrences {
            let Some(value) = captured_polynomial(value, &self.unsigned_atoms, &mut remaining)
            else {
                return false;
            };
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Occurrence(*occurrence),
                value,
            });
        }
        // Public result transport deliberately treats an actual without a
        // completed occurrence binding as opaque. Capture numeric actuals in
        // postorder, under the original roster: an outer call then consumes
        // the already-captured inner result, never a replay of its body.
        let mut pending = vec![(expression, false, 0usize)];
        while let Some((operand, visited, depth)) = pending.pop() {
            let Some(next) = remaining.checked_sub(1) else {
                return false;
            };
            remaining = next;
            if depth >= 64 {
                return false;
            }
            let node = self.program.expression_table.expression(operand);
            if !visited {
                pending.push((operand, true, depth));
                super::super::super::super::expression_traversal::children(
                    self.program,
                    node,
                    |child| pending.push((child, false, depth + 1)),
                );
                continue;
            }
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            let Some(owner) = super::super::specification_calls::selected_call_contract(
                self.program,
                self.source_owner,
                call,
            ) else {
                continue;
            };
            let parameters =
                super::super::specification_calls::owner_parameters(self.program, &owner);
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments);
            if parameters.len() != arguments.len() {
                return false;
            }
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if !self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
                    || bindings.iter().any(|binding| {
                        binding.binder == ScopedArithmeticBinder::Occurrence(*argument)
                    })
                {
                    continue;
                }
                let Some(argument_bindings) =
                    demanded_bindings(self.program, *argument, &bindings, &mut remaining)
                else {
                    return false;
                };
                bindings.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(*argument),
                    value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: self.source_owner,
                        expression: *argument,
                        bindings: argument_bindings,
                    }),
                });
            }
        }
        let source = ScopedArithmeticExpression {
            owner: self.source_owner,
            expression,
            bindings,
        };
        let mut calls = self
            .captured_calls
            .take()
            .unwrap_or_else(|| ScopedCalls::for_declaration(self.program, self.machine_symbol));
        let prepared = calls.expression(&source);
        let hypotheses = calls.hypotheses(&[]);
        self.captured_calls = Some(calls);
        let (Some(prepared), Some(hypotheses)) = (prepared, hypotheses) else {
            return false;
        };
        let previous_owner = self.source_owner;
        let previous_symbols = self.strict_symbol_bindings.clone();
        let previous_result = self.strict_result_binding.clone();
        let previous_occurrences = self.strict_occurrences.clone();
        let previous_conditions = std::mem::take(&mut self.saved_conditions);
        let prepared_occurrences = self
            .install_scoped_expression(&prepared)
            .then(|| self.strict_occurrences.clone());
        let accepted =
            prepared_occurrences.is_some() && install_hypotheses(self.program, self, &hypotheses);
        self.source_owner = previous_owner;
        self.strict_symbol_bindings = previous_symbols;
        self.strict_result_binding = previous_result;
        self.strict_occurrences = previous_occurrences;
        self.saved_conditions = previous_conditions;
        if !accepted {
            return false;
        }
        for (occurrence, value) in prepared_occurrences.into_iter().flatten() {
            if !self.bind_strict_occurrence(occurrence, value) {
                return false;
            }
        }
        true
    }
}

/// A captured operand owns only the bindings it reads. Copying the complete
/// accumulated roster into each nested actual would expand unrelated earlier
/// captures repeatedly, despite a bounded source expression.
fn demanded_bindings(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    bindings: &[ScopedArithmeticBinding],
    remaining: &mut usize,
) -> Option<Vec<ScopedArithmeticBinding>> {
    let mut selected = Vec::new();
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        *remaining = remaining.checked_sub(1)?;
        let occurrence = ScopedArithmeticBinder::Occurrence(expression);
        if let Some(binding) = bindings.iter().find(|binding| binding.binder == occurrence) {
            if !selected
                .iter()
                .any(|candidate: &ScopedArithmeticBinding| candidate.binder == occurrence)
            {
                selected.push(binding.clone());
            }
            continue;
        }
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(name) = node {
            let binder = ScopedArithmeticBinder::Symbol(name.symbol);
            if let Some(binding) = bindings.iter().find(|binding| binding.binder == binder)
                && !selected.iter().any(|candidate| candidate.binder == binder)
            {
                selected.push(binding.clone());
            }
        }
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
    Some(selected)
}

/// Preserve the already-normalized value, including opaque arithmetic atoms,
/// instead of replaying the source expression under a later binding roster.
fn captured_polynomial(
    polynomial: &Polynomial,
    unsigned: &[String],
    remaining: &mut usize,
) -> Option<ScopedArithmeticValue> {
    let mut sum = ScopedArithmeticValue::Integer(super::super::BigInt::zero());
    for (monomial, coefficient) in &polynomial.terms {
        *remaining = remaining.checked_sub(1)?;
        let mut term = ScopedArithmeticValue::Integer(coefficient.clone());
        for (identity, power) in monomial {
            let atom = ScopedArithmeticValue::Atom {
                identity: identity.clone(),
                unsigned: unsigned.contains(identity),
            };
            for _ in 0..*power {
                *remaining = remaining.checked_sub(1)?;
                term = ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Multiply,
                    left: Box::new(term),
                    right: Box::new(atom.clone()),
                };
            }
        }
        sum = ScopedArithmeticValue::IntegerOperation {
            operator: BinaryOperator::Add,
            left: Box::new(sum),
            right: Box::new(term),
        };
    }
    Some(sum)
}
