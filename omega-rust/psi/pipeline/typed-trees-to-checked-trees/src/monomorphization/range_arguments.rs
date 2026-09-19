//! Ordinary call inference from declared integer range endpoints.
//!
//! Range selection is not an exact type equation: bind an omitted endpoint,
//! then let ordinary call validation check the complete substituted parameter.
//! Explicit selections remain fixed and repeated inferred occurrences must agree.
//! Never use value/flow bounds or intersect predicates to manufacture a maximum.
//!
//! Authored endpoints retain their inclusion kind. The shared numeric query
//! checks each endpoint before taking an exclusive predecessor in proof integers.
//! Closed anonymous arithmetic uses exact evaluation and canonical const leaves,
//! so inference cannot truncate fractions or overflow an intermediate carrier.
//! Closed builtin typed arithmetic retains exact constant points and checks
//! each fixed-width operation before interval projection.
//!
//! An open endpoint still carries an exact structural equation when it spells
//! one caller const binder in the same authored inclusion position: the
//! callee's endpoint then binds that binder's identity, and the retained
//! selection rechecks the occurrence once the caller specializes. Any other
//! open shape records an unresolved occurrence instead; neither flow bounds
//! nor type display strings establish a static endpoint.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};
use validation::{
    closed_integer_range_bound, closed_integer_range_maximum,
    declared_integer_range as declared_range,
};

pub(super) fn collect_literals(program: &TypedTrees, literals: &mut Vec<String>) {
    for (_, constraints) in program.type_reference_table.constrained_type_references() {
        for constraint in program.type_reference_table.constraints(constraints) {
            if let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            {
                if let Some(value) = closed_integer_range_bound(program, *minimum) {
                    literals.push(value.to_string());
                }
                if let Some(value) = closed_integer_range_maximum(program, *maximum, *end_inclusive)
                {
                    literals.push(value.to_string());
                    // Either authored boundary kind may be required by the
                    // callee. This conversion is proof-integer normalization,
                    // not solving an arbitrary endpoint equation.
                    literals.push(
                        value
                            .add(&numerics::bignum::BigInt::from_u64(1))
                            .to_string(),
                    );
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
    let Some((required_carrier, required_endpoints, required_inclusive)) =
        declared_range(program, required)
    else {
        return;
    };
    let Some((actual_carrier, actual_endpoints, actual_inclusive)) =
        declared_range(program, actual)
    else {
        return;
    };
    if required_carrier != actual_carrier {
        return;
    }
    let values = match [
        closed_integer_range_bound(program, actual_endpoints[0]),
        closed_integer_range_maximum(program, actual_endpoints[1], actual_inclusive),
    ] {
        [Some(minimum), Some(maximum)] if minimum <= maximum => Some([
            minimum,
            if required_inclusive {
                maximum
            } else {
                maximum.add(&numerics::bignum::BigInt::from_u64(1))
            },
        ]),
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
            .or_else(|| {
                open_endpoint_binder(
                    program,
                    actual_endpoints[endpoint_index],
                    endpoint_index == 0 || required_inclusive == actual_inclusive,
                )
            })
            .unwrap_or_default();
        // Zero records an unresolved occurrence, not permission to ignore it.
        // Another argument must not select a compatible but unequal endpoint
        // before a caller's substitution makes this occurrence checkable.
        // Unsupported endpoint computations likewise cannot be silently bypassed.
        proposals.push((candidate_index, parameter_index, binding));
    }
}

/// Exact binder evidence for one open actual endpoint. Only a bare caller
/// const binder in the same authored inclusion position carries the
/// structural equation; the endpoint's named reference is materialized by
/// `collect_binders` so this lookup stays a pure read.
fn open_endpoint_binder(
    program: &TypedTrees,
    actual_endpoint: typed_trees::expression::ExpressionHandle,
    same_inclusion_position: bool,
) -> Option<TypeReferenceHandle> {
    if !same_inclusion_position {
        return None;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(actual_endpoint) else {
        return None;
    };
    if !is_const_parameter_symbol(program, name.symbol) {
        return None;
    }
    program
        .type_reference_table
        .find_named_type_reference(name.symbol)
}

fn is_const_parameter_symbol(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && program.machines().iter().any(|machine| {
            program
                .machine_type_parameters(machine)
                .iter()
                .any(|parameter| {
                    parameter.symbol == symbol
                        && matches!(parameter.kind, TypeParameterKind::Const { .. })
                })
        })
}

/// Retain the binder identity of open declared-range endpoints. A generic
/// caller's const parameter occurring as an endpoint needs a named type
/// reference so `infer` can propose the exact structural equation instead of
/// an anonymous value leaf.
pub(super) fn collect_binders(
    program: &TypedTrees,
    types: &mut Vec<(SymbolHandle, typed_trees::name::Identifier)>,
) {
    for (_, constraints) in program.type_reference_table.constrained_type_references() {
        for constraint in program.type_reference_table.constraints(constraints) {
            let TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            else {
                continue;
            };
            for endpoint in [minimum, maximum] {
                let ExpressionNode::Name(name) = program.expression_table.expression(*endpoint)
                else {
                    continue;
                };
                if !is_const_parameter_symbol(program, name.symbol)
                    || types.iter().any(|(symbol, _)| *symbol == name.symbol)
                {
                    continue;
                }
                if let Some(member) = program
                    .expression_table
                    .name_path_members(name.members)
                    .first()
                {
                    types.push((name.symbol, member.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TypeConstraintNode, TypeParameterKind, TypedTrees, closed_integer_range_bound};
    use crate::monomorphization::range_arguments::declared_range;
    use crate::monomorphization::range_arguments::infer;
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::types::TypeReferenceNode;

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("range tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("range syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("range symbols");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("range types")
    }

    #[test]
    fn open_symbolic_endpoint_binds_the_caller_binder_until_it_specializes() {
        let mut program = typed(
            "machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
             machine forward<const K: u64>(v: u64[0..=K]) -> u64 { upper_bound(v) }
             machine caller(v: u64[0..=256]) -> u64 { forward<256>(v) }",
        );
        crate::checking::specialize_static_machine_calls_with_selections(&mut program, true)
            .expect("open endpoint forwards the caller binder");
        let forward = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "forward")
            .expect("forward template");
        let upper_bound = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "upper_bound")
            .expect("upper_bound template");
        assert!(
            program
                .machine_specializations
                .iter()
                .any(|instance| instance.template == forward.symbol),
            "forward<256> must specialize"
        );
        assert!(
            program
                .machine_specializations
                .iter()
                .any(|instance| instance.template == upper_bound.symbol),
            "the forwarded upper_bound call must specialize inside forward<256>"
        );
    }

    #[test]
    fn open_exclusive_symbolic_endpoint_binds_the_caller_binder() {
        let mut program = typed(
            "machine upper_bound<const N: u64>(value: u64[0..N]) -> u64 { N }
             machine forward<const K: u64>(v: u64[0..K]) -> u64 { upper_bound(v) }
             machine caller(v: u64[0..256]) -> u64 { forward<256>(v) }",
        );
        crate::checking::specialize_static_machine_calls_with_selections(&mut program, true)
            .expect("exclusive open endpoint forwards the caller binder");
        let upper_bound = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "upper_bound")
            .expect("upper_bound template");
        assert!(
            program
                .machine_specializations
                .iter()
                .any(|instance| instance.template == upper_bound.symbol),
            "the forwarded upper_bound call must specialize inside forward<256>"
        );
    }

    #[test]
    fn mixed_inclusion_open_endpoint_records_no_equation() {
        let mut program = typed(
            "machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
             machine forward<const K: u64>(v: u64[0..K]) -> u64 { upper_bound(v) }",
        );
        super::super::materialize_static_argument_types(&mut program);
        let upper_bound = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "upper_bound")
            .expect("upper_bound template");
        let forward = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "forward")
            .expect("forward template");
        let required =
            program.state_parameters(&program.machine_states(upper_bound)[0])[0].type_reference;
        let actual =
            program.state_parameters(&program.machine_states(forward)[0])[0].type_reference;
        let const_parameters = program
            .machine_type_parameters(upper_bound)
            .iter()
            .filter_map(|parameter| match parameter.kind {
                TypeParameterKind::Const { type_reference } => Some((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    type_reference,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut proposals = Vec::new();
        infer(
            &program,
            required,
            actual,
            &const_parameters,
            &[],
            0,
            &mut proposals,
        );
        assert_eq!(proposals.len(), 1);
        assert!(
            !proposals[0].2.is_valid(),
            "an exclusive open endpoint cannot satisfy an inclusive required endpoint"
        );
    }

    #[test]
    fn declared_range_keeps_full_width_endpoints_and_rejects_ambiguous_shells() {
        let tokens = source_files_to_tokens::Lexer::new(
            "machine value(input: u64[0..=18446744073709551615]) -> u64 { input }",
        )
        .tokenize()
        .expect("range tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("range syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("range symbols");
        let mut program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("range types");
        let (range, _, constraints) = program
            .type_reference_table
            .constrained_type_reference_sites()[0];
        let (_, endpoints, _) = declared_range(&program, range).expect("one declared range");
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
