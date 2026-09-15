//! Named constants require copy permission and no cleanup on their complete type.
//!
//! This declaration check is independent of active initializer materialization
//! and generic atom encoding. Ordinary property validation still checks every
//! declared `[copy]` claim. Here exact nominal owners additionally exclude
//! cleanup, including inactive case payloads; empty arrays retain their element
//! eligibility without inheriting native plain-storage geometry restrictions.

use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use symbols::BuiltinTypeAtom;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

pub(crate) fn validate_constants(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for declaration in program.const_declarations() {
        if !eligible(program, declaration.declared_type, &mut Vec::new()) {
            diagnostics.push(Diagnostic::error(
                "constant type must permit copying and contain no cleanup, shared ownership, or interior mutability",
            ).with_source_span(declaration.initializer_source_span));
        }
    }
}

fn eligible(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> bool {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
        || active.contains(&reference)
    {
        return false;
    }
    active.push(reference);
    let result = eligible_type(program, reference, active);
    active.pop();
    result
}

fn eligible_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Unit => true,
        TypeReferenceNode::Constrained { base_type, .. } => eligible(program, *base_type, active),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            eligible(program, *element_type, active)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(atom) = program.symbols.builtin_type_atom(*symbol) {
                return matches!(
                    atom,
                    BuiltinTypeAtom::Bool
                        | BuiltinTypeAtom::I8
                        | BuiltinTypeAtom::I16
                        | BuiltinTypeAtom::I32
                        | BuiltinTypeAtom::I64
                        | BuiltinTypeAtom::U8
                        | BuiltinTypeAtom::U16
                        | BuiltinTypeAtom::U32
                        | BuiltinTypeAtom::U64
                        | BuiltinTypeAtom::Address
                        | BuiltinTypeAtom::F32
                        | BuiltinTypeAtom::F64
                        | BuiltinTypeAtom::UInt
                        | BuiltinTypeAtom::Int
                );
            }
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|data| data.symbol == *symbol);
            let Some(data) = definitions.next() else {
                return false;
            };
            if !symbol.is_valid()
                || definitions.next().is_some()
                || data.properties.multiplicity != Multiplicity::Unrestricted
                || !program.data_type_parameters(data).is_empty()
                || active[..active.len() - 1].iter().any(|ancestor| matches!(program.type_reference_table.type_reference(*ancestor), TypeReferenceNode::Named { symbol: previous, .. } if previous == symbol))
                || program.machines().iter().any(|machine| {
                    machine.attached_data_symbol == *symbol
                        && machine.name.as_str().ends_with("::drop")
                })
            {
                return false;
            }
            program
                .data_members(data)
                .iter()
                .all(|member| match member {
                    DataMember::Field(field) => eligible(program, field.type_reference, active),
                    DataMember::Variant(variant) => program
                        .data_payload_fields(variant)
                        .iter()
                        .all(|field| eligible(program, field.type_reference, active)),
                })
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_) => false,
    }
}
