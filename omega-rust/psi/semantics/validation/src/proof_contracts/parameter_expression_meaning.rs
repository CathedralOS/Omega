//! Abstract callable contracts have a parameter telescope, not an executable
//! machine. Rejoin each operator with that declaration's selected meaning;
//! this query grants neither a predicate nor argument snapshot evidence.

use language_core::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

pub fn has_builtin_parameter_bound_expression_meaning(
    program: &TypedTrees,
    owner_symbol: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    let mut pending = vec![(expression, false)];
    let mut visited = Vec::new();
    let mut active = Vec::new();
    while let Some((expression, completed)) = pending.pop() {
        if completed {
            active.pop();
            visited.push(expression);
            continue;
        }
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if visited.contains(&expression) {
            continue;
        }
        if active.contains(&expression) {
            return false;
        }
        active.push(expression);
        pending.push((expression, true));
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                let spelling = match binary.operator {
                    BinaryOperator::Equal => Some(OperatorSpelling::Equal),
                    BinaryOperator::NotEqual => Some(OperatorSpelling::NotEqual),
                    BinaryOperator::Less => Some(OperatorSpelling::Less),
                    BinaryOperator::LessOrEqual => Some(OperatorSpelling::LessEqual),
                    BinaryOperator::Greater => Some(OperatorSpelling::Greater),
                    BinaryOperator::GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
                    BinaryOperator::Add => Some(OperatorSpelling::Add),
                    BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
                    BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
                    BinaryOperator::Divide => Some(OperatorSpelling::Divide),
                    BinaryOperator::Modulo => Some(OperatorSpelling::Modulo),
                    BinaryOperator::CaseMembership => return false,
                    // These nodes have no overloadable operator spelling.
                    BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => None,
                };
                if let Some(spelling) = spelling {
                    let operands = [binary.left, binary.right].map(|operand| {
                        crate::parameter_expression_result_type_reference(
                            program,
                            owner_symbol,
                            parameters,
                            operand,
                        )
                    });
                    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        owner_symbol,
                        expression,
                        spelling,
                        &operands,
                    ) {
                        return false;
                    }
                }
                pending.extend([(binary.left, false), (binary.right, false)]);
            }
            ExpressionNode::Unary(unary) => pending.push((unary.operand, false)),
            ExpressionNode::Borrow(borrow) => pending.push((borrow.target, false)),
            ExpressionNode::Cast(cast) => pending.push((cast.value, false)),
            ExpressionNode::Atomic(atomic) => pending.push((atomic.value, false)),
            // Calls and places remain symbolic leaves. Consumers separately
            // check their carrier, invocation and captured value identity.
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::has_builtin_parameter_bound_expression_meaning;
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    fn typed(source: &str) -> typed_trees::TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolved");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typed")
    }

    #[test]
    fn requirement_arithmetic_preserves_selected_operator_meaning() {
        for (declaration, builtin) in [
            ("", true),
            (
                "boundary operator + u64::custom(left: u64, right: u64) -> u64;",
                false,
            ),
        ] {
            let program = typed(&format!(
                "{declaration}
                boundary trait Counter {{ machine increase(input: u64) -> u64
                    ensures result == input + 1; }}"
            ));
            let signature = &program.trait_machine_signatures(&program.traits()[0])[0];
            let parameters = program.state_signature_parameters(signature);
            let expression = program.expression_table.iter_expressions().find_map(|(handle, node)| {
                matches!(node, ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal)
                    .then_some(handle)
            }).expect("guarantee");
            assert_eq!(
                has_builtin_parameter_bound_expression_meaning(
                    &program,
                    signature.symbol,
                    parameters,
                    expression
                ),
                builtin
            );
        }
    }

    #[test]
    fn requirement_parameter_types_reject_foreign_telescope_and_keep_policy() {
        let program = typed(
            "data Count { remaining: u64 in Wrapping; }
            boundary trait Counter {
                machine first(input: Count) -> u64 ensures result == input.remaining + 1;
                machine second(other: Count) -> u64;
            }",
        );
        let signatures = program.trait_machine_signatures(&program.traits()[0]);
        let expression = program.expression_table.iter_expressions().find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Member(member) if member.member.as_str() == "remaining")
                .then_some(handle)
        }).expect("parameter field");
        let reference = crate::parameter_expression_result_type_reference(
            &program,
            signatures[0].symbol,
            program.state_signature_parameters(&signatures[0]),
            expression,
        )
        .expect("exact parameter type");
        assert_eq!(
            program.arithmetic_domain_for_type_reference(reference),
            numerics::arithmetic::ArithmeticDomain::Wrapping
        );
        assert!(
            crate::parameter_expression_result_type_reference(
                &program,
                signatures[1].symbol,
                program.state_signature_parameters(&signatures[1]),
                expression
            )
            .is_none()
        );
        let arithmetic = program.expression_table.iter_expressions().find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Add)
                .then_some(handle)
        }).expect("addition");
        let reference = crate::parameter_expression_result_type_reference(
            &program,
            signatures[0].symbol,
            program.state_signature_parameters(&signatures[0]),
            arithmetic,
        )
        .expect("selected arithmetic result");
        assert_eq!(
            program.arithmetic_domain_for_type_reference(reference),
            numerics::arithmetic::ArithmeticDomain::Wrapping
        );
    }
}
