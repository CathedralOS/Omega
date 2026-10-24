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

use crate::monomorphization::{BindingContext, StaticProposals};

use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;

use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};
use validation::{
    closed_integer_range_bound, closed_integer_range_maximum,
    declared_integer_range as declared_range,
};

pub(super) fn collect_literals(
    program: &dyn ::validation::CheckingInput,
    literals: &mut Vec<String>,
) {
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
    context: BindingContext<'_>,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    proposals: &mut StaticProposals,
) {
    let Some(fixed_parameters) = context.fixed_range_parameters else {
        return;
    };
    let program = context.program;
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
        let Some(name) = program.expression_table.name_path_node(required_endpoint) else {
            continue;
        };
        let Some(parameter_index) = context
            .const_parameters
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
        proposals
            .constants
            .push((context.candidate_index, parameter_index, binding));
    }
}

/// Exact binder evidence for one open actual endpoint. Only a bare caller
/// const binder in the same authored inclusion position carries the
/// structural equation; the endpoint's named reference is materialized by
/// `collect_binders` so this lookup stays a pure read.
fn open_endpoint_binder(
    program: &dyn ::validation::CheckingInput,
    actual_endpoint: typed_trees::expression::ExpressionHandle,
    same_inclusion_position: bool,
) -> Option<TypeReferenceHandle> {
    if !same_inclusion_position {
        return None;
    }
    let name = program.expression_table.name_path_node(actual_endpoint)?;
    if !is_const_parameter_symbol(program, name.symbol) {
        return None;
    }
    program
        .type_reference_table
        .find_named_type_reference(name.symbol)
}

fn is_const_parameter_symbol(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> bool {
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
    program: &dyn ::validation::CheckingInput,
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
                let Some(name) = program.expression_table.name_path_node(*endpoint) else {
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
