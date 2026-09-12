//! Normal-result congruence uses the selected computation, not source spelling
//! or a replay of its body. Both sides enter the immutable entry namespace via
//! exact exit origins before substitution. Unknown leaves remain symbolic;
//! only Boolean congruence and closed connectives establish a truth value.

use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use typed_trees::types::PrimitiveType;

use super::{ExitScalars, ExpressionHandle, exit_return_expression};

impl ExitScalars<'_, '_> {
    pub(super) fn proves_boolean_result(&self, expression: ExpressionHandle) -> Option<bool> {
        let entry = self.program.machine_states(self.machine).first()?;
        if self.program.primitive_type_reference(entry.return_type) != Some(PrimitiveType::Bool) {
            return None;
        }
        let returned = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        let (selected, symbols) = self.selected_return_expression(returned)?;
        let CheckedScalarExpression::Boolean(selected) = selected else {
            return None;
        };
        let predicate = crate::values::lower_scalar_contract_predicate(
            self.program,
            &self.facts.operators,
            self.machine,
            expression,
            true,
        )?;
        let state = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        let parameters = self.program.state_parameters(entry);
        let scalar_parameters = || {
            parameters.iter().filter(|parameter| {
                self.program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
        };
        // A dense slot is not itself identity. Require a unique origin for this
        // clause and an immutable Boolean binder on both ends of that edge.
        let origin = |position| {
            let parameter = scalar_parameters().nth(position)?;
            if parameter.is_mutable
                || parameter.is_self
                || parameter.is_const
                || self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    != Some(PrimitiveType::Bool)
            {
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
                    origin.contract == self.contract && origin.entry_parameter == parameter.symbol
                });
            let origin = origins.next()?;
            if origins.next().is_some() || !origin.state_parameter.is_valid() {
                return None;
            }
            self.program
                .state_parameters(state)
                .iter()
                .find(|parameter| {
                    parameter.symbol == origin.state_parameter
                        && !parameter.is_mutable
                        && !parameter.is_self
                        && !parameter.is_const
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            == Some(PrimitiveType::Bool)
                })
                .map(|parameter| parameter.symbol)
        };
        let mut remaining = 4096;
        let returned = bind_boolean(
            selected,
            &|position| {
                let symbol = *symbols.get(position)?;
                let mut positions = (0..scalar_parameters().count())
                    .filter(|position| origin(*position) == Some(symbol));
                let position = positions.next()?;
                positions.next().is_none().then_some(position)
            },
            None,
            &mut remaining,
            0,
        )?;
        let predicate = bind_boolean(
            &predicate,
            &|position| origin(position).map(|_| position),
            Some((scalar_parameters().count(), &returned)),
            &mut remaining,
            0,
        )?;
        match predicate {
            CheckedBooleanExpression::Constant(value) => Some(value),
            _ => None,
        }
    }
}

fn bind_boolean(
    expression: &CheckedBooleanExpression,
    resolve: &impl Fn(usize) -> Option<usize>,
    result: Option<(usize, &CheckedBooleanExpression)>,
    remaining: &mut usize,
    depth: usize,
) -> Option<CheckedBooleanExpression> {
    use CheckedBooleanExpression as Boolean;
    if depth >= 64 || *remaining == 0 {
        return None;
    }
    *remaining -= 1;
    Some(match expression {
        Boolean::Constant(value) => Boolean::Constant(*value),
        Boolean::Parameter { position } => {
            if let Some((result_position, returned)) = result
                && *position == result_position
            {
                // The returned expression already uses entry positions. Rewalk
                // it with the same budget rather than multiplying unbounded
                // clones for repeated reserved-result occurrences.
                return bind_boolean(returned, resolve, None, remaining, depth + 1);
            }
            Boolean::Parameter {
                position: resolve(*position)?,
            }
        }
        Boolean::Not(operand) => {
            match bind_boolean(operand, resolve, result, remaining, depth + 1)? {
                Boolean::Constant(value) => Boolean::Constant(!value),
                operand => Boolean::Not(Box::new(operand)),
            }
        }
        Boolean::Equal { left, right }
        | Boolean::And { left, right }
        | Boolean::Or { left, right } => {
            let left = Box::new(bind_boolean(left, resolve, result, remaining, depth + 1)?);
            let right = Box::new(bind_boolean(right, resolve, result, remaining, depth + 1)?);
            match expression {
                Boolean::Equal { .. } if left == right => Boolean::Constant(true),
                Boolean::Equal { .. } => match (&*left, &*right) {
                    (Boolean::Constant(left), Boolean::Constant(right)) => {
                        Boolean::Constant(left == right)
                    }
                    _ => Boolean::Equal { left, right },
                },
                Boolean::And { .. } => match (&*left, &*right) {
                    (Boolean::Constant(left), Boolean::Constant(right)) => {
                        Boolean::Constant(*left && *right)
                    }
                    _ => Boolean::And { left, right },
                },
                _ => match (&*left, &*right) {
                    (Boolean::Constant(left), Boolean::Constant(right)) => {
                        Boolean::Constant(*left || *right)
                    }
                    _ => Boolean::Or { left, right },
                },
            }
        }
        // Locals, storage and non-Boolean computations need their own retained
        // value/effect evidence; matching their spelling or shape is not proof.
        _ => return None,
    })
}
