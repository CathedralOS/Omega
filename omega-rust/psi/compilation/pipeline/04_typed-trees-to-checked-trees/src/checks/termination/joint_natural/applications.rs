//! Apply the selected declaration to an exact captured input and retain all
//! result facts established by its independent formation judgment.

use crate::checks::current_scalar_values::CurrentScalarValues;
use validation::BigIntInterval;
use validation::{ScopedArithmeticExpression, ScopedArithmeticValue};

#[derive(Clone, Copy)]
pub(super) struct Application {
    pub(super) measure: symbols::SymbolHandle,
    pub(super) parameter: symbols::SymbolHandle,
    pub(super) body: typed_trees::expression::ExpressionHandle,
    pub(super) input_type: typed_trees::types::TypeReferenceHandle,
    pub(super) result_primitive: typed_trees::types::PrimitiveType,
}

pub(super) struct ObservedApplication {
    pub(super) value: ScopedArithmeticValue,
    pub(super) formation: validation::IntegerApplicationFormation,
}

impl Application {
    pub(super) fn shape(
        self,
        program: &dyn validation::CheckingInput,
    ) -> Option<validation::DeclaredInputShape> {
        validation::declared_input_shape(
            program,
            program
                .measures()
                .iter()
                .find(|declaration| declaration.symbol == self.measure)?,
        )
    }

    pub(super) fn observe(
        self,
        values: &CurrentScalarValues<'_>,
        machine: &typed_trees::machine::Machine,
        input_type: typed_trees::types::TypeReferenceHandle,
        bindings: Vec<validation::ScopedArithmeticBinding>,
    ) -> Option<ObservedApplication> {
        let expression = ScopedArithmeticExpression {
            owner: self.measure,
            expression: self.body,
            bindings,
        };
        let formation = values.declared_integer_application_formation(
            machine,
            &expression,
            input_type,
            self.result_primitive,
        )?;
        let (minimum, maximum) = self.result_primitive.integer_bounds()?;
        // The child-first application proof established these bounds. A later
        // relation query must not lose them when normalizing a polynomial such
        // as (count-1)^2. These facts never justify the application's formation.
        Some(ObservedApplication {
            formation,
            value: ScopedArithmeticValue::BoundedInteger {
                value: Box::new(ScopedArithmeticValue::Term(expression)),
                interval: BigIntInterval {
                    low: Some(numerics::bignum::BigInt::from_i128(minimum)),
                    high: Some(numerics::bignum::BigInt::from_i128(maximum)),
                },
            },
        })
    }
}
