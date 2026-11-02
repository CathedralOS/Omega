//! Closed, formed integer arrivals use the existing strict contradiction judgment.
use super::RangeFacts;
use typed_trees::expression::{BinaryOperator, TableBinaryExpression};

impl RangeFacts<'_> {
    /// The caller established builtin comparison meaning and selected this
    /// exact arm. Literal operands are invocation-fixed; their source terms preserve
    /// formation. Address and IEEE operands cannot supply integer facts.
    pub(in crate::checks::ranges) fn retain_closed_integer_arrival(
        &mut self,
        program: &dyn validation::CheckingInput,
        binary: &TableBinaryExpression,
        operator: BinaryOperator,
    ) {
        if !matches!(
            operator,
            BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Equal
                | BinaryOperator::NotEqual
        ) {
            return;
        }
        let Some(left) = program.expression_table.integer_node(binary.left) else {
            return;
        };
        let Some(right) = program.expression_table.integer_node(binary.right) else {
            return;
        };
        // A substituted const binder retains its landed carrier, while an
        // anonymous integer endpoint may not yet carry a literal landing.
        // Read both as their exact source terms through the strict engine;
        // no mathematical value is supplied as a substitute for formation.
        if [left, right].into_iter().any(|literal| {
            literal.landing().is_some_and(|landing| {
                landing.landed_type == numerics::literals::LandedIntegerType::Addr
            })
        }) {
            return;
        }
        let arrival = (binary.left, binary.right, operator);
        crate::collections::push_unique(&mut self.closed_integer_arrivals, arrival);
    }

    /// Bounds remain ordinary obligations. An exact closed arrival can close
    /// them only when the existing scoped integer engine proves contradiction;
    /// unknown terms/operators decline and no source arm is pruned.
    pub(in crate::checks::ranges) fn closed_integer_arrival_is_impossible(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> bool {
        if self.closed_integer_arrivals.is_empty() {
            return false;
        }
        let term = |expression| {
            validation::ScopedArithmeticValue::Term(validation::ScopedArithmeticExpression {
                owner: machine.symbol,
                expression,
                bindings: Vec::new(),
            })
        };
        let comparisons: Vec<_> = self
            .closed_integer_arrivals
            .iter()
            .map(
                |(left, right, operator)| validation::ScopedArithmeticComparison {
                    operator: *operator,
                    left: term(*left),
                    right: term(*right),
                },
            )
            .collect();
        validation::scoped_arithmetic_comparisons_unsatisfiable(program, machine, &comparisons)
    }
}
