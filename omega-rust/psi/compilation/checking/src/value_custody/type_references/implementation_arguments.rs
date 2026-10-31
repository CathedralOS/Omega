//! Static implementation operands retain ordinary argument formation.
//!
//! Closing a named implementation establishes its category and complete
//! telescope. Every supplying expression still owes its own arithmetic and
//! construction obligations, even when the implementation ignores that slot.

use super::{
    TopLevelSymbols, TypeParameterScope, TypeReferenceOwner,
    generic_arguments::{AppliedValueCarrier, validate_const_argument_expression_formation},
    validate_type_reference_node,
};
use diagnostics::Diagnostic;
use typed_trees::{
    data::TypeParameterKind,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(super) fn validate_operand_formation(
    program: &dyn crate::CheckingInput,
    argument: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    owner: TypeReferenceOwner<'_>,
    scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(argument);
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return;
    };
    let Some(declaration) = program.conformance_by_symbol(*base_symbol) else {
        return;
    };
    let parameters = program.conformance_type_parameters(declaration);
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    for (parameter, argument) in parameters.iter().zip(arguments) {
        match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                validate_const_argument_expression_formation(
                    program,
                    &program.symbols.display_path(declaration.symbol, "::"),
                    parameter,
                    AppliedValueCarrier::applied(type_reference, parameters, arguments),
                    *argument,
                    scope,
                    &owner.value_formation_facts(program),
                    owner.local_value_formation(),
                    diagnostics,
                    obligations,
                );
            }
            TypeParameterKind::Type => validate_type_reference_node(
                program,
                *argument,
                symbols,
                diagnostics,
                owner.generic_argument(),
                false,
                scope,
                obligations,
            ),
            TypeParameterKind::Implementation => validate_operand_formation(
                program,
                *argument,
                symbols,
                owner.generic_argument(),
                scope,
                diagnostics,
                obligations,
            ),
            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {}
        }
    }
}
