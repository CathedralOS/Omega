//! Apply the selected declaration to an exact captured input and retain all
//! result facts established by its independent formation judgment.

use crate::checks::current_scalar_values::CurrentScalarValues;
use validation::{ScopedArithmeticExpression, ScopedArithmeticValue};

#[derive(Clone, Copy)]
pub(super) struct Application {
    pub(super) measure: symbols::SymbolHandle,
    pub(super) parameter: symbols::SymbolHandle,
    pub(super) body: typed_trees::expression::ExpressionHandle,
    pub(super) primitive: typed_trees::types::PrimitiveType,
}

impl Application {
    pub(super) fn observe(
        self,
        values: &CurrentScalarValues<'_>,
        machine: &typed_trees::machine::Machine,
        input: ScopedArithmeticValue,
    ) -> Option<ScopedArithmeticValue> {
        let expression = ScopedArithmeticExpression {
            owner: self.measure,
            expression: self.body,
            bindings: vec![validation::ScopedArithmeticBinding {
                binder: validation::ScopedArithmeticBinder::Symbol(self.parameter),
                value: input,
            }],
        };
        if !values.forms_declared_integer_application(machine, &expression, self.primitive) {
            return None;
        }
        let (minimum, maximum) = self.primitive.integer_bounds()?;
        // The child-first application proof established these bounds. A later
        // relation query must not lose them when normalizing a polynomial such
        // as (count-1)^2. These facts never justify the application's formation.
        Some(ScopedArithmeticValue::BoundedInteger {
            value: Box::new(ScopedArithmeticValue::Term(expression)),
            minimum: Some(numerics::bignum::BigInt::from_i128(minimum)),
            maximum: Some(numerics::bignum::BigInt::from_i128(maximum)),
        })
    }
}
