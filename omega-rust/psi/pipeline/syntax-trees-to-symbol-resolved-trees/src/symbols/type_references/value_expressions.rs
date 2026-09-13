//! Type-bound value expressions use the callable's ordinary value namespace.
//! Type/domain names retain declaration lookup; index expressions must resolve
//! lexical subjects before a later phase can establish that their value is static.

use arena::{Arena, Handle};
use symbol_resolved_trees::{
    expression::ExpressionTable,
    signature::StateParameter,
    types::{TypeConstraint, TypeReference},
};
use symbols::{SymbolHandle, SymbolTable};

use crate::symbols::{expressions::assign_statement_expression_symbols, scope::MachineScope};

#[allow(clippy::too_many_arguments)]
pub(in crate::symbols) fn assign_type_value_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    state_symbol: SymbolHandle,
    expressions: &mut ExpressionTable,
    child_types: &mut Arena<TypeReference>,
    type_reference: &TypeReference,
) {
    let mut child = |handle: Handle<TypeReference>| {
        let reference = child_types.get(handle).clone();
        assign_type_value_expression_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expressions,
            child_types,
            &reference,
        );
    };
    match type_reference {
        TypeReference::Reference(reference) => child(reference.referee),
        TypeReference::FixedArray(array) => child(array.element_type),
        TypeReference::Slice(slice) => child(slice.element_type),
        TypeReference::Generic(generic) => {
            for offset in 0..generic.arguments.count() {
                let start = generic.arguments.start();
                child(Handle::from_parts(
                    start.arena_index() + offset,
                    start.generation(),
                ));
            }
        }
        TypeReference::Constrained(constrained) => {
            child(constrained.base_type);
            for constraint in machine
                .type_constraints
                .span_or_empty(constrained.constraints)
            {
                match constraint {
                    TypeConstraint::Range {
                        minimum, maximum, ..
                    } => {
                        for expression in [*minimum, *maximum] {
                            assign_statement_expression_symbols(
                                symbols,
                                machine,
                                parameters,
                                state_symbol,
                                expressions,
                                child_types,
                                expression,
                            );
                        }
                    }
                    TypeConstraint::Domain(domain) => {
                        for offset in 0..domain.arguments.count() {
                            let start = domain.arguments.start();
                            let handle = Handle::from_parts(
                                start.arena_index() + offset,
                                start.generation(),
                            );
                            let reference = child_types.get(handle).clone();
                            assign_type_value_expression_symbols(
                                symbols,
                                machine,
                                parameters,
                                state_symbol,
                                expressions,
                                child_types,
                                &reference,
                            );
                        }
                    }
                    TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReference::ConstExpression(expression) => {
            // Resolve lexical subjects before determining whether an index is
            // closed. A runtime binding cannot become a same-named constant.
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expressions,
                child_types,
                *expression,
            );
        }
        TypeReference::Named { .. }
        | TypeReference::DynamicTrait { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => {}
    }
}
