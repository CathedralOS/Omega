//! Receiving carriers for the shared ordered child roster.
//!
//! A contextual collection literal has no standalone type reference. Capturing
//! it under a call's result type discarded that entire normal-return path,
//! including unrelated scalar facts needed by later arguments. Calls and
//! constructors instead supply their exact selected operand declarations.
//! These are typing contexts, not evidence of argument admission, referent
//! origins or post-call preservation. Those judgments retain their owners.

use super::super::super::specification_calls::{
    SelectedCallContract, call_parameter_scope, owner_leading_operands, selected_call_contract,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::TypeReferenceHandle;

/// Positions match `expression_traversal::children`, including its receiver
/// slot even when absent. No expression-handle search may select a receiving
/// position: the same handle can occur on more than one operand edge.
pub(super) fn receiving_types(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    expression: ExpressionHandle,
    node: &ExpressionNode,
    receiving: TypeReferenceHandle,
) -> Option<Vec<TypeReferenceHandle>> {
    match node {
        ExpressionNode::Call(call) => {
            let mut owner = selected_call_contract(program, caller.symbol, call)?;
            // A static generic qualifier owns its public application just as
            // a dynamic slot owns its instantiated data telescope. Keep that
            // scope; an implementation's private parameter types are not it.
            if let SelectedCallContract::Signature(signature) = owner
                && program
                    .closed_requirement_application(signature.symbol)
                    .is_none()
                && call.requirement_application.is_valid()
            {
                let (declaration, _) = program.trait_machine_signature(signature.symbol)?;
                let application = program
                    .applied_requirement_scope(caller.symbol, call.requirement_application)?
                    .declaring_scope(program, declaration.symbol)?;
                // Preparation recursively instantiated composite carriers.
                // A shallow substitution cannot turn &[T] into &[u8]. Keep
                // a closed instance's own method-local arguments above, and
                // require this exact prepared public view for an open owner.
                owner = SelectedCallContract::Signature(
                    program.applied_callable_signature(&application, signature.symbol)?,
                );
            }
            let parameters = call_parameter_scope(
                program,
                caller.symbol,
                &owner,
                crate::CallSite::Expression { expression, call },
            )?;
            let leading = owner_leading_operands(&owner);
            if program
                .expression_table
                .expression_handles(call.arguments)
                .len()
                != leading + parameters.len()
            {
                return None;
            }
            // Dispatch-prefix values are independently typed. Only data
            // arguments receive the public callable's parameter carriers.
            let mut references = vec![TypeReferenceHandle::invalid(); 1 + leading];
            references.extend(parameters.iter().map(|parameter| parameter.type_reference));
            Some(references)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            // Each element receives the element carrier, not the aggregate.
            // This also preserves contextual literals nested inside literals.
            let element = crate::value_custody::places::collection_element_type_reference(
                program, receiving,
            )?;
            Some(vec![
                element;
                program
                    .expression_table
                    .expression_handles(*elements)
                    .len()
            ])
        }
        ExpressionNode::StructLiteral(literal) => Some(
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .map(|field| {
                    crate::value_custody::struct_literals::selected_construction_field_type(
                        program,
                        expression,
                        literal,
                        field.field_symbol,
                    )
                    .unwrap_or_else(TypeReferenceHandle::invalid)
                })
                .collect(),
        ),
        _ => None,
    }
}
