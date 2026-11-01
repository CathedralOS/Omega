//! Proposition type operands apply to the complete parameter carrier. Reuse
//! ordinary simultaneous correspondence; references and arrays are not
//! separate instantiation cases, and matching supplies no evidence.

use super::{ExpressionHandle, ProofFactOwner, TypeReferenceHandle};
use typed_trees::proposition::{
    PropositionApplication, PropositionBinderArgumentKind, PropositionBinderKind,
    PropositionDefinition,
};
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceNode;

pub(super) fn argument_matches(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
    parameter_type: TypeReferenceHandle,
    declaration: Option<&PropositionDefinition>,
    application: &PropositionApplication,
    owner: ProofFactOwner<'_>,
) -> bool {
    let mut substitutions = Vec::new();
    if let Some(declaration) = declaration {
        for (index, binder) in program.proposition_binders(declaration).iter().enumerate() {
            let Some(selected) = application.binder_arguments.get(index) else {
                return false;
            };
            let expected_kind = match binder.kind {
                PropositionBinderKind::Type => PropositionBinderArgumentKind::Type,
                PropositionBinderKind::Const { .. } => PropositionBinderArgumentKind::Const,
                PropositionBinderKind::Machine => PropositionBinderArgumentKind::Machine,
            };
            if selected.kind != expected_kind {
                return false;
            }
            let reference = if let Some(literal) = &selected.const_literal {
                program
                    .type_reference_table
                    .type_reference_rows()
                    .find_map(|(handle, node)| {
                        let TypeReferenceNode::ConstExpression(expression) = node else {
                            return None;
                        };
                        matches!(program.expression_table.expression(*expression),
                        typed_trees::expression::ExpressionNode::Integer(value) if value == literal
                    ).then_some(handle)
                    })
            } else if selected.symbol.is_valid() {
                program
                    .type_reference_table
                    .find_named_type_reference(selected.symbol)
            } else {
                None
            };
            let Some(reference) = reference else {
                return false;
            };
            substitutions.push((binder.symbol, reference));
        }
    }
    carrier_matches(program, argument, parameter_type, &substitutions, owner)
}

fn carrier_matches(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
    parameter_type: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    owner: ProofFactOwner<'_>,
) -> bool {
    if let Some(borrow) = program.expression_table.borrow_expression(argument) {
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            return false;
        };
        // A proof operand states its complete carrier, not an implicit runtime
        // lending conversion. Preserve every explicit borrow shell before
        // comparing the declared target, including nested reference depth.
        return borrow.access == *access
            && carrier_matches(program, borrow.target, *referee, substitutions, owner);
    }
    if let Some(actual) = super::proof_argument_declared_type(program, argument, owner) {
        return program.type_identity(TypeIdentityRequest::ordinary(actual))
            == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                substitutions,
                ..TypeIdentityRequest::ordinary(parameter_type)
            });
    }
    crate::value_custody::expression_types::argument_matches_type_reference_with_substitutions(
        program,
        argument,
        parameter_type,
        substitutions,
    )
}
