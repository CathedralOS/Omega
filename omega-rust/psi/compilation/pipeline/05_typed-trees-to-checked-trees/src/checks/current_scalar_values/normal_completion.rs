//! Normal-completion observations bind the exact outgoing result.
//!
//! Current stores already retain bounded joins as well as concrete values.
//! Rejoin those observations to the declaration's result paths after all return
//! operand writes have retired old facts. The guarantee contributes no bound;
//! this roster comes only from the completed value's existing typed capture.
//! It is neither a descriptor-content permission nor an executed scalar read.

use super::scalar_fields::{CapturedScalarField, capture_path, same_projection};
use super::{CurrentScalarValues, Length, LengthSubject, ScopedOccurrence, admitted, scoped};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};

pub(super) struct CapturedResultFields {
    pub(super) owner: SymbolHandle,
    fields: Vec<CapturedScalarField>,
}

/// Only this invocation's exact result namespace may consume this roster.
/// A same-shaped result on a selected callee is a different subject.
pub(super) fn result_scalar_field_occurrence(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let result = occurrence.result_fields?;
    let place = validation::reserved_result_place(occurrence.program, expression)?;
    if place.machine_symbol != result.owner || place.machine_symbol != occurrence.machine.symbol {
        return None;
    }
    let path = capture_path(&place.segments, |_| None)?;
    let carrier = super::integer_carrier(occurrence.program, place.type_reference)?;
    let mut fields = result.fields.iter().filter(|field| {
        field.carrier == carrier && same_projection(occurrence, &field.path, &path)
    });
    let field = fields.next()?;
    fields.next().is_none().then(|| field.value.clone())
}

impl CurrentScalarValues<'_> {
    pub(crate) fn prepare_exit_return(&mut self, returned: ExpressionHandle) {
        if let Some(value) = self.collection_length(returned, 0)
            && let Some(entry) = self.program.machine_states(self.machine).first()
        {
            self.lengths.push(Length {
                subject: LengthSubject::Result(self.machine.symbol),
                name: String::new(),
                stable: true,
                type_reference: entry.return_type,
                value: Some(value),
            });
        }
        self.record_returned_field_lengths(returned);
        self.result_fields = self
            .program
            .machine_states(self.machine)
            .first()
            .map(|entry| CapturedResultFields {
                owner: self.machine.symbol,
                fields: self.captured_scalar_fields_of_type(returned, entry.return_type, 0),
            });
    }

    pub(crate) fn postcondition(
        &self,
        expression: ExpressionHandle,
        returned: Option<ExpressionHandle>,
        origins: impl Iterator<Item = (SymbolHandle, SymbolHandle)>,
    ) -> Option<ScopedArithmeticExpression> {
        // The declaration's postcondition names invocation parameters, while
        // the live hypotheses name the exit state's parameters. Bind those
        // namespaces through the independently established incoming origins;
        // spelling, slot order and a write-free body are not identity evidence.
        let entry = self.program.machine_states(self.machine).first()?;
        let mut bindings = self.current.clone();
        let mut lengths = self.lengths.clone();
        if entry.symbol != self.state.symbol {
            bindings.clear();
            lengths.retain(|length| {
                matches!(
                    length.subject,
                    LengthSubject::Result(_) | LengthSubject::ResultField { .. }
                )
            });
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
        let fields = if entry.symbol == self.state.symbol {
            self.fields.as_slice()
        } else {
            &[]
        };
        // Only this completed exit binds its reserved result. Ordinary and
        // callee occurrences deliberately carry no result roster, including
        // another invocation of the same recursive declaration.
        let mut occurrence = self.occurrence_at(self.program, self.machine, entry, &lengths);
        occurrence.result_fields = self.result_fields.as_ref();
        if !admitted(occurrence, expression, &bindings, fields, 0) {
            return None;
        }
        let mut goal = scoped(occurrence, expression, &bindings, fields)?;
        if let Some(returned) = returned.and_then(|returned| self.current(returned)) {
            goal.bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Result,
                value: ScopedArithmeticValue::Term(returned),
            });
        }
        Some(goal)
    }
}
