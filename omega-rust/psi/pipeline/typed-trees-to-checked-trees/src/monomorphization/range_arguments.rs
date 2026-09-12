//! Ordinary call inference from declared integer range endpoints.
//!
//! Range selection is not an exact type equation: bind an omitted endpoint,
//! then let ordinary call validation check the complete substituted parameter.
//! Explicit selections remain fixed and repeated inferred occurrences must agree.
//! Never use value/flow bounds or intersect predicates to manufacture a maximum.
//!
//! Endpoints include the parser's inclusive-end conversion. Closed anonymous
//! arithmetic uses the shared exact numeric evaluator and canonical const leaves,
//! so inference cannot truncate fractions or overflow an intermediate carrier.
//! Closed builtin typed arithmetic retains exact constant points and checks
//! each fixed-width operation before interval projection. Named computations and
//! owner-sensitive selections still need their own evaluation custody; neither
//! flow bounds nor type display strings establish a static endpoint.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};
use validation::{closed_integer_range_bound, declared_integer_range as declared_range};

pub(super) fn collect_literals(program: &TypedTrees, literals: &mut Vec<String>) {
    for (_, constraints) in program.type_reference_table.constrained_type_references() {
        for constraint in program.type_reference_table.constraints(constraints) {
            if let TypeConstraintNode::Range { minimum, maximum } = constraint {
                for endpoint in [*minimum, *maximum] {
                    if let Some(value) = closed_integer_range_bound(program, endpoint) {
                        literals.push(value.to_string());
                    }
                }
            }
        }
    }
}

pub(super) fn infer(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[(SymbolHandle, String, TypeReferenceHandle)],
    fixed_parameters: &[usize],
    candidate_index: usize,
    proposals: &mut Vec<(usize, usize, TypeReferenceHandle)>,
) {
    let Some((required_carrier, required_endpoints)) = declared_range(program, required) else {
        return;
    };
    let Some((actual_carrier, actual_endpoints)) = declared_range(program, actual) else {
        return;
    };
    if required_carrier != actual_carrier {
        return;
    }
    let values =
        match actual_endpoints.map(|endpoint| closed_integer_range_bound(program, endpoint)) {
            [Some(minimum), Some(maximum)] if minimum <= maximum => Some([minimum, maximum]),
            _ => None,
        };
    for (endpoint_index, required_endpoint) in required_endpoints.into_iter().enumerate() {
        let ExpressionNode::Name(name) = program.expression_table.expression(required_endpoint)
        else {
            continue;
        };
        let Some(parameter_index) = parameters
            .iter()
            .position(|(symbol, _, _)| symbol.is_valid() && *symbol == name.symbol)
        else {
            continue;
        };
        if fixed_parameters.contains(&parameter_index) {
            continue;
        }
        let binding = values
            .as_ref()
            .and_then(|values| {
                let value = values[endpoint_index].to_string();
                program
                    .type_reference_table
                    .named_references()
                    .find(|(_, symbol, name)| !symbol.is_valid() && *name == value)
                    .map(|(binding, _, _)| binding)
            })
            .unwrap_or_default();
        // Zero records an unresolved occurrence, not permission to ignore it.
        // Another argument must not select a compatible but unequal endpoint
        // before a caller's substitution makes this occurrence checkable.
        // Unsupported endpoint computations likewise cannot be silently bypassed.
        proposals.push((candidate_index, parameter_index, binding));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn declared_range_keeps_full_width_endpoints_and_rejects_ambiguous_shells() {
        let tokens = source_files_to_tokens::Lexer::new(
            "machine value(input: u64[0..=18446744073709551615]) -> u64 { input }",
        )
        .tokenize()
        .expect("range tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("range syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("range symbols");
        let mut program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("range types");
        let (range, _, constraints) = program
            .type_reference_table
            .constrained_type_reference_sites()[0];
        let (_, endpoints) = declared_range(&program, range).expect("one declared range");
        assert_eq!(
            closed_integer_range_bound(&program, endpoints[1])
                .expect("literal endpoint")
                .to_string(),
            "18446744073709551615"
        );
        let duplicated = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: range,
                constraints,
            });
        assert!(declared_range(&program, duplicated).is_none());
        let policy = program.type_reference_table.insert_constraints([
            TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Wrapping),
        ]);
        let policy_wrapped = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: range,
                constraints: policy,
            });
        assert!(declared_range(&program, policy_wrapped).is_none());
    }
}
