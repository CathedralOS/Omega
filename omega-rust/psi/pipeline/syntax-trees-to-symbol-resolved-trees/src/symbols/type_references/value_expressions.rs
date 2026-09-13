//! Type-bound value expressions use their declaration's ordinary value namespace.
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

/// A field's type can contain the same value expressions as a parameter's type.
/// Resolve them after declaration and type identities exist, before typing can
/// copy an unresolved call into a constant position. Data owns its generic and
/// field names but has no machine activation: no state parameters or locals are
/// in scope. Field references remain symbolic, not same-spelled global constants.
pub(in crate::symbols) fn assign_data_type_value_expression_symbols(
    program: &mut symbol_resolved_trees::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let attached_machines = crate::symbols::scope::attached_machines(program);
    let declarations = &mut program.tables.declarations;
    for definition in program.roots.data_definitions.iter() {
        let members = declarations.data_members.span_or_empty(definition.members);
        let mut scope = MachineScope {
            symbol: definition.symbol,
            attached_machines: &attached_machines,
            type_parameters: declarations
                .data_type_parameters
                .span_or_empty(definition.type_parameters),
            attached_data: Some(&definition.name),
            attached_data_symbol: definition.symbol,
            inherited_data_members: Some(members),
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            data_members: &declarations.data_members,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        for member in members {
            let fields = match member {
                symbol_resolved_trees::data::DataMember::Field(field) => {
                    scope.symbol = definition.symbol;
                    std::slice::from_ref(field)
                }
                symbol_resolved_trees::data::DataMember::Variant(variant) => {
                    // Payload subjects shadow common fields and global names.
                    // The symbol hierarchy retains the enclosing data owner.
                    scope.symbol = variant.symbol;
                    declarations
                        .data_payload_fields
                        .span_or_empty(variant.payload)
                }
            };
            for field in fields {
                assign_type_value_expression_symbols(
                    symbols,
                    &scope,
                    &[],
                    SymbolHandle::invalid(),
                    &mut program.tables.bodies.expressions,
                    &mut declarations.child_type_references,
                    &field.type_reference,
                );
            }
        }
    }
}

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
