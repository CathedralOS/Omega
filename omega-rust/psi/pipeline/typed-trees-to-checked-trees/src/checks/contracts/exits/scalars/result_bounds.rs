//! Result comparisons use the exact exit expression's immutable bounds. The
//! shared bounds owner checks arithmetic; no initializer or mutable read is replayed.

use super::{ExitScalars, exit_return_expression, has_builtin_operators, is_result_reference};
use language_core::OperatorSpelling;
use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

impl ExitScalars<'_, '_> {
    pub(super) fn proves_result_bounds(&self, expression: ExpressionHandle) -> bool {
        has_builtin_operators(self.program, &self.facts.operators, expression)
            && validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                self.program.machine_states(self.machine).first(),
                expression,
            )
            && self.result_bounds_comparison(expression) == Some(true)
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
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let spelling = match binary.operator {
            BinaryOperator::Equal => OperatorSpelling::Equal,
            BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
            BinaryOperator::Less => OperatorSpelling::Less,
            BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
            BinaryOperator::Greater => OperatorSpelling::Greater,
            BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
            _ => return None,
        };
        let result_on_left = is_result_reference(self.program, self.machine, binary.left);
        let argument = if result_on_left {
            binary.right
        } else if is_result_reference(self.program, self.machine, binary.right) {
            binary.left
        } else {
            return None;
        };
        let entry = self.program.machine_states(self.machine).first()?;
        let primitive = exact_integer_carrier(self.program, entry.return_type)?;
        let state = crate::semantic_calls::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        let returned = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        // These bounds hold at every evaluation. Entry operands use only their
        // immutable declared ranges, while result bounds belong to this live
        // state's exact return occurrence, never another state's same-spelled name.
        let result_bounds = validation::immutable_integer_expression_bounds(
            self.program,
            self.machine,
            state,
            returned,
        )?;
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
                [Some(entry.return_type), argument_type],
            )
        } else {
            (
                argument_bounds,
                result_bounds,
                [argument_type, Some(entry.return_type)],
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
        let equal_points = left.0 == left.1 && right.0 == right.1 && left.0 == right.0;
        let disjoint = left.1 < right.0 || right.1 < left.0;
        let (proved, refuted) = match binary.operator {
            BinaryOperator::Equal => (equal_points, disjoint),
            BinaryOperator::NotEqual => (disjoint, equal_points),
            BinaryOperator::Less => (left.1 < right.0, left.0 >= right.1),
            BinaryOperator::LessOrEqual => (left.1 <= right.0, left.0 > right.1),
            BinaryOperator::Greater => (left.0 > right.1, left.1 <= right.0),
            BinaryOperator::GreaterOrEqual => (left.0 >= right.1, left.1 < right.0),
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
}

fn exact_integer_carrier(
    program: &TypedTrees,
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
                use symbols::BuiltinTypeAtom;
                let atom = program.symbols.builtin_type_atom(*symbol)?;
                if name.as_str() != atom.symbol_name() {
                    return None;
                }
                return Some(match atom {
                    BuiltinTypeAtom::U8 => PrimitiveType::U8,
                    BuiltinTypeAtom::U16 => PrimitiveType::U16,
                    BuiltinTypeAtom::U32 => PrimitiveType::U32,
                    BuiltinTypeAtom::U64 => PrimitiveType::U64,
                    BuiltinTypeAtom::I8 => PrimitiveType::I8,
                    BuiltinTypeAtom::I16 => PrimitiveType::I16,
                    BuiltinTypeAtom::I32 => PrimitiveType::I32,
                    BuiltinTypeAtom::I64 => PrimitiveType::I64,
                    _ => return None,
                });
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{PrimitiveType, TypeReferenceNode, TypedTrees, exact_integer_carrier};
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::name::Identifier;

    #[test]
    fn cyclic_carrier_wrappers_are_unknown() {
        let mut program = TypedTrees::default();
        let first = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let second = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: first,
                constraints: Default::default(),
            });
        for base_type in [first, second] {
            program.type_reference_table.substitute_node(
                first,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints: Default::default(),
                },
            );
            assert_eq!(exact_integer_carrier(&program, first), None);
        }
    }

    #[test]
    fn builtin_carrier_requires_matching_symbol_and_spelling() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let builtins = SymbolTableBuilder::child_handles(
            builder.insert_children(root, symbols::builtin_type_symbols()),
        )
        .collect::<Vec<_>>();
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        for (name, expected) in [("u64", Some(PrimitiveType::U64)), ("u8", None)] {
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: builtins[BuiltinTypeAtom::U64.ordinal()],
                    name: Identifier::generated_static(name),
                });
            assert_eq!(exact_integer_carrier(&program, reference), expected);
        }
    }
}
