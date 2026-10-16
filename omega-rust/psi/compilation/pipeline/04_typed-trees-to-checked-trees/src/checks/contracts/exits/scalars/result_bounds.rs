//! Postconditions compare immutable return bounds or live exit-storage snapshots.
//! The shared bounds owner checks selected arithmetic, including overflow policy;
//! no initializer or stale mutable read is replayed to reconstruct a value.

use super::{ExitScalars, exit_return_expression, has_builtin_operators, is_result_reference};
use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt};
use typed_trees::{
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

mod guarded_returns;
mod reconstruction;

impl ExitScalars<'_, '_> {
    pub(super) fn proves_bounds(&self, expression: ExpressionHandle) -> bool {
        has_builtin_operators(self.program, &self.facts.operators, expression)
            && validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                self.program.machine_states(self.machine).first(),
                expression,
            )
            && (self.result_bounds_comparison(expression) == Some(true)
                || self.guarded_return_bounds_comparison(expression) == Some(true)
                || self.current_bounds_comparison(expression) == Some(true))
    }

    /// Current storage snapshots can bound a postcondition without identifying
    /// one exact scalar value. Retain the selected arithmetic policy, including
    /// wrapping, rather than interpreting its syntax as unbounded arithmetic.
    fn current_bounds_comparison(&self, expression: ExpressionHandle) -> Option<bool> {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        ) {
            return None;
        }
        let entry = self.program.machine_states(self.machine).first()?;
        let parameters = self.program.state_parameters(entry);
        // A reference parameter names live exit storage whether its binding
        // is `self` or an explicit argument. The parser also marks exclusive
        // references mutable, so that flag alone cannot identify owned inputs.
        // Owned mutable inputs still need invocation-value transport, and the
        // origin check below rejects renamed or rebound reference bindings.
        if parameters.iter().any(|parameter| {
            parameter.is_const
                || (parameter.is_mutable
                    && !crate::values::exclusive_reference(self.program, parameter.type_reference))
        }) || self
            .facts
            .flow
            .control
            .exit_parameter_origins
            .span_or_empty(self.exit.parameter_origins)
            .iter()
            .any(|origin| {
                origin.contract == self.contract
                    && (!origin.state_parameter.is_valid()
                        || origin.entry_parameter != origin.state_parameter)
            })
        {
            return None;
        }
        let primitive = [binary.left, binary.right]
            .into_iter()
            .find_map(|operand| {
                validation::expression_result_type_reference(
                    self.program,
                    self.machine,
                    entry,
                    operand,
                )
                .and_then(|reference| self.program.primitive_type_reference(reference))
                .filter(|primitive| validation::carrier_integer_range(*primitive).is_some())
            })?;
        let lower = |operand| {
            crate::values::lower_unit_scalar_argument(
                self.program,
                &self.facts.operators,
                entry,
                0,
                operand,
                primitive,
            )
        };
        let left = lower(binary.left)?;
        let right = lower(binary.right)?;
        let symbols = parameters
            .iter()
            .filter(|parameter| crate::values::occupies_scalar_position(self.program, parameter))
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let mut values = crate::values::LivePlaceIntervals {
            program: self.program,
            semantic: &self.facts.semantic,
            contexts: self.contexts,
            parameters,
            symbols: &symbols,
            state: entry.symbol,
        };
        let left = validation::selected_scalar_interval(&left, &mut values)?;
        let right = validation::selected_scalar_interval(&right, &mut values)?;
        interval_comparison(binary.operator, &left, &right)
    }

    // None means overlapping intervals or unsupported evidence, never false.
    // Negation may consume only a proved or refuted comparison.
    fn result_bounds_comparison(&self, expression: ExpressionHandle) -> Option<bool> {
        if let ExpressionNode::Unary(unary) = self.program.expression_table.expression(expression)
            && unary.operator == UnaryOperator::LogicalNot
        {
            return self
                .result_bounds_comparison(unary.operand)
                .map(|value| !value);
        }
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return None;
        };
        let spelling = validation::comparison_operator_spelling(binary.operator)?;
        // A result operand is the bare return reference or a member
        // projection through it (`result.first`); both carry the selected
        // constructor expression's immutable bounds in the same namespace.
        let (result_operand, argument, result_on_left) =
            if let Some(operand) = self.result_bound_operand(binary.left) {
                (operand, binary.right, true)
            } else {
                let operand = self.result_bound_operand(binary.right)?;
                (operand, binary.left, false)
            };
        let (result_expression, result_type) = result_operand;
        let entry = self.program.machine_states(self.machine).first()?;
        let primitive = exact_integer_carrier(self.program, result_type)?;
        let state = self.state()?;
        let returned = exit_return_expression(self.program, self.exit);
        // Independent intervals drop the shared dividend of `(n / d) * d`.
        // The reconstruction window proves comparisons those intervals leave
        // overlapping, including when a local names `d`.
        if self.truncating_reconstruction_proves(
            binary.operator,
            result_expression,
            argument,
            result_on_left,
        ) {
            return Some(true);
        }
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        // Standing bounds describe every legal observation, including copied
        // constructor children. The occurrence owner can additionally bound
        // this exact evaluated return (for example a numeric conversion).
        // Intersect their independent evidence; neither rereads an initializer
        // or borrows another state's same-spelled parameter.
        let standing = validation::immutable_integer_expression_bounds(
            self.program,
            self.machine,
            state,
            result_expression,
        );
        let observed = validation::occurrence_integer_expression_bounds(
            self.program,
            self.machine.symbol,
            state.symbol,
            self.exit.statement_index,
            result_expression,
        );
        let result_bounds = match (standing, observed) {
            (Some(standing), Some(observed)) => {
                (standing.0.max(observed.0), standing.1.min(observed.1))
            }
            (standing, observed) => standing.or(observed)?,
        };
        let argument_bounds = validation::immutable_integer_expression_bounds(
            self.program,
            self.machine,
            entry,
            argument,
        )?;
        let argument_type = validation::expression_result_type_reference(
            self.program,
            self.machine,
            entry,
            argument,
        );
        if let Some(reference) = argument_type
            && exact_integer_carrier(self.program, reference) != Some(primitive)
        {
            return None;
        }
        // Anonymous operands land at the declared result carrier. Small final
        // bounds do not change the shared query's per-operation safety checks.
        for endpoint in [
            result_bounds.0,
            result_bounds.1,
            argument_bounds.0,
            argument_bounds.1,
        ] {
            typed_trees::closed_numeric::land_integer(&BigInt::from_i64(endpoint), primitive)?;
        }
        let (left, right, types) = if result_on_left {
            (
                result_bounds,
                argument_bounds,
                [Some(result_type), argument_type],
            )
        } else {
            (
                argument_bounds,
                result_bounds,
                [argument_type, Some(result_type)],
            )
        };
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.machine.symbol,
            expression,
            spelling,
            &types,
        ) {
            return None;
        }
        interval_comparison(
            binary.operator,
            &facts::IntegerRange {
                minimum: BigInt::from_i64(left.0),
                maximum: BigInt::from_i64(left.1),
            },
            &facts::IntegerRange {
                minimum: BigInt::from_i64(right.0),
                maximum: BigInt::from_i64(right.1),
            },
        )
    }

    /// The expression and declared type whose immutable bounds a result
    /// operand carries: the whole return for a bare `result` reference, or
    /// the constructor-selected member expression for a projected
    /// `result.field` operand. Longer member paths stay unproven.
    fn result_bound_operand(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
        let entry = self.program.machine_states(self.machine).first()?;
        if is_result_reference(self.program, self.machine, expression) {
            return Some((
                exit_return_expression(self.program, self.exit),
                entry.return_type,
            ));
        }
        let (selected, remaining) = self.result_projection(expression)?;
        if !remaining.is_empty() {
            return None;
        }
        let place = validation::reserved_result_place(self.program, expression)?;
        Some((selected, place.type_reference))
    }
}

fn interval_comparison(
    operator: BinaryOperator,
    left: &facts::IntegerRange,
    right: &facts::IntegerRange,
) -> Option<bool> {
    if left.minimum > left.maximum || right.minimum > right.maximum {
        return None;
    }
    let equal_points = left.minimum == left.maximum
        && right.minimum == right.maximum
        && left.minimum == right.minimum;
    let disjoint = left.maximum < right.minimum || right.maximum < left.minimum;
    let (proved, refuted) = match operator {
        BinaryOperator::Equal => (equal_points, disjoint),
        BinaryOperator::NotEqual => (disjoint, equal_points),
        BinaryOperator::Less => (left.maximum < right.minimum, left.minimum >= right.maximum),
        BinaryOperator::LessOrEqual => {
            (left.maximum <= right.minimum, left.minimum > right.maximum)
        }
        BinaryOperator::Greater => (left.minimum > right.maximum, left.maximum <= right.minimum),
        BinaryOperator::GreaterOrEqual => {
            (left.minimum >= right.maximum, left.maximum < right.minimum)
        }
        _ => return None,
    };
    if proved {
        Some(true)
    } else if refuted {
        Some(false)
    } else {
        None
    }
}

fn exact_integer_carrier(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut carrier = reference;
    // Bound the entire walk, including policy lookup: the general recursive
    // policy query cannot be called before malformed cycles have been excluded.
    for _ in 0..128 {
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint, TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact)
                    })
                {
                    return None;
                }
                carrier = *base_type;
            }
            TypeReferenceNode::Named { symbol, name } => {
                let atom = program.symbols.builtin_type_atom(*symbol)?;
                if name.as_str() != atom.symbol_name() {
                    return None;
                }
                return PrimitiveType::from_builtin_atom(atom);
            }
            _ => return None,
        }
    }
    None
}
