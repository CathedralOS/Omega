//! A selected scalar extent guarantee can name descriptor cells behind shared
//! aggregate references without publishing the contents behind that descriptor.

use super::{StructuralTerm, bound_term};
use symbols::SymbolHandle;
use typed_trees::{
    expression::{ExpressionHandle, ExpressionNode},
    signature::StateParameter,
    types::TypeReferenceHandle,
};

pub(super) fn bound_descriptor(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    scope: SymbolHandle,
    parameters: &[StateParameter],
    callable: SymbolHandle,
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: Option<&StructuralTerm>,
    depth: usize,
) -> Option<StructuralTerm> {
    let (reference, value) = descriptor_path(
        program, expression, scope, parameters, callable, bindings, result, depth,
    )?;
    crate::slice_view_shell(program, reference, &[])?;
    Some(value.into_collection_descriptor())
}

fn descriptor_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    scope: SymbolHandle,
    parameters: &[StateParameter],
    callable: SymbolHandle,
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: Option<&StructuralTerm>,
    depth: usize,
) -> Option<(TypeReferenceHandle, StructuralTerm)> {
    if depth >= 64 {
        return None;
    }
    let reference = |expression| {
        crate::parameter_expression_result_type_reference(program, scope, parameters, expression)
    };
    let selected = reference(expression)?;
    let value = match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let value = bound_term(
                program,
                expression,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            let root = value.semantic_value();
            shared_inline_crossings(program, selected)?;
            let mut carrier = selected;
            while let Some(shell) = crate::reference_shell(program, carrier) {
                carrier = shell.referee;
            }
            if matches!(root, StructuralTerm::Constructor { .. }) {
                (carrier.is_valid()
                    && crate::reference_shell(program, carrier).is_none()
                    && program.primitive_type_reference(carrier).is_none())
                .then_some(())?;
            } else {
                let observed = captured_owned_carrier(program, root, depth + 1)?;
                crate::declared_carriers_match(program, carrier, observed).then_some(())?;
            }
            root.clone()
        }
        ExpressionNode::Member(member) if member.case_variant.is_none() => {
            let (receiver, value) = descriptor_path(
                program,
                member.receiver,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            shared_inline_crossings(program, receiver)?;
            let data = crate::value_custody::places::data_definition_for_type(program, receiver)?;
            crate::value_custody::places::exact_data_member_field(
                program,
                data,
                member.member_symbol,
                member.member.as_str(),
                None,
            )?;
            super::super::super::observed_projections::project_field(value, member.member.as_str())?
        }
        ExpressionNode::Indexed(indexed) if !program.expression_table.indexed_on_range(indexed) => {
            let (collection_type, collection) = descriptor_path(
                program,
                indexed.collection,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            shared_inline_crossings(program, collection_type)?;
            let references = [Some(collection_type), reference(indexed.index)];
            if !typed_trees::operator::builtin_index_read_is_spelled(
                program,
                scope,
                expression,
                &references,
            ) {
                return None;
            }
            let index = bound_term(
                program,
                indexed.index,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            if references[1].is_some_and(|reference| {
                !super::super::super::structural_terms::scalar_operand_is_observed(
                    program, reference, &index,
                )
            }) {
                return None;
            }
            StructuralTerm::IndexedValue {
                element_type: selected,
                element_identity: program.normalized_type_identity(selected),
                collection: Box::new(collection),
                index: Box::new(index),
            }
        }
        _ => return None,
    };
    Some((selected, value))
}

fn captured_owned_carrier(
    program: &dyn crate::CheckingInput,
    value: &StructuralTerm,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 64 {
        return None;
    }
    let value = value.semantic_value();
    match value {
        StructuralTerm::CapturedValue { .. } => {}
        StructuralTerm::Projection { subject, .. }
        | StructuralTerm::IndexedValue {
            collection: subject,
            ..
        } => {
            captured_owned_carrier(program, subject, depth + 1)?;
        }
        _ => return None,
    }
    // Every ancestor stays owned; an indexed leaf cannot hide a prior
    // reference crossing merely because its element carrier is owned.
    super::super::super::structural_terms::completed_owned_carrier(program, value)
}

fn shared_inline_crossings(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<()> {
    let mut crossed = Vec::new();
    while let Some(shell) = crate::reference_shell(program, reference) {
        if crossed.contains(&reference)
            || shell.access != language_semantics::ReferenceAccess::Shared
            || crate::slice_view_shell(program, reference, &[]).is_some()
        {
            return None;
        }
        crossed.push(reference);
        reference = shell.referee;
    }
    Some(())
}
