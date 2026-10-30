//! Named constants require copy permission and no cleanup on their complete type.
//!
//! This declaration check is independent of active initializer materialization
//! and generic atom encoding. Ordinary property validation still checks every
//! declared `[copy]` claim. Here exact nominal owners additionally exclude
//! cleanup, including inactive case payloads; empty arrays retain their element
//! eligibility without inheriting native plain-storage geometry restrictions.
//!
//! Literal declarations also owe their integer ranges even when never used.
//! Carrier eligibility and substitution do not discharge those predicates.
//! Completed-initializer replay calls the literal membership check below after
//! endpoint normalization. Eager declaration validation cannot do so: private
//! evaluation probes still contain unready endpoints and provisional values.
//! Walk the completed root with its selected member types and share the ordinary
//! exact range reader. This supplies neither domain issuance nor an invocation
//! proof for a computed initializer, whose admission remains independent.

use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use symbols::BuiltinTypeAtom;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_constants(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in program.const_declarations() {
        if !eligible(program, declaration.declared_type, &mut Vec::new()) {
            diagnostics.push(Diagnostic::error(
                "constant type must permit copying and contain no cleanup, shared ownership, or interior mutability",
            ).with_source_span(declaration.initializer_source_span));
        }
    }
}

/// Validate one completed declaration root after its range endpoints are ready.
/// The evaluation owner selects ready declarations; a private placeholder is
/// neither a value to validate here nor evidence exported to a consumer.
pub fn validate_constant_literal_integer_ranges(
    program: &dyn crate::CheckingInput,
    destination: TypeReferenceHandle,
    initializer: ExpressionHandle,
) -> Result<(), &'static str> {
    let mut pending = vec![(destination, initializer)];
    while let Some((destination, expression)) = pending.pop() {
        if program
            .primitive_type_reference(destination)
            .is_some_and(|primitive| primitive.is_integer())
        {
            let ranges = crate::value_custody::type_references::constrained_range_bounds(
                program,
                destination,
                false,
            )
            .ok_or("constant initializer has an unresolved integer range")?;
            if !ranges.is_empty() {
                // Read only the retained literal, never fold a runtime expression
                // or treat its destination as evidence of its own membership.
                let ExpressionNode::Integer(literal) =
                    program.expression_table.expression(expression)
                else {
                    return Err(
                        "constant initializer lacks an integer literal for its declared range",
                    );
                };
                let value = literal.value_bignum().ok_or(
                    "constant initializer lacks an exact integer literal for its declared range",
                )?;
                if ranges
                    .iter()
                    .any(|(minimum, maximum)| &value < minimum || &value > maximum)
                {
                    return Err(
                        "constant initializer literal does not satisfy its declared integer range",
                    );
                }
            }
            continue;
        }
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(destination);
        match program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(elements) => {
                if let TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(carrier)
                {
                    pending.extend(
                        program
                            .expression_table
                            .expression_handles(*elements)
                            .iter()
                            .map(|element| (*element_type, *element)),
                    );
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    let field_type =
                        crate::value_custody::struct_literals::selected_construction_field_type(
                            program,
                            expression,
                            literal,
                            field.field_symbol,
                        )
                        .ok_or("constant initializer lost its selected construction field")?;
                    pending.push((field_type, field.value));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn eligible(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Unit => true,
        // A selector contains code selection only: copying it neither captures
        // user state nor creates cleanup, shared ownership or interior mutation.
        // Construction separately authenticates the exact implementation map.
        TypeReferenceNode::DynamicTrait { symbol, .. } => {
            program.trait_definition_by_symbol(*symbol).is_some()
        }
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
            let Some(data) = program.data_definition_by_symbol(*symbol) else {
                return false;
            };
            if !symbol.is_valid()
                || data.properties.multiplicity != Multiplicity::Unrestricted
                || !program.data_type_parameters(data).is_empty()
                || active[..active.len() - 1].iter().any(|ancestor| {
                    match program.type_reference_table.type_reference(*ancestor) {
                        TypeReferenceNode::Named {
                            symbol: previous, ..
                        } => previous == symbol,
                        TypeReferenceNode::Generic {
                            base_symbol: previous,
                            ..
                        } => previous == symbol,
                        _ => false,
                    }
                })
                || program.attached_drop_machine_exists(*symbol)
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
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(data) = program.data_definition_by_symbol(*base_symbol) else {
                return false;
            };
            if !base_symbol.is_valid()
                || data.properties.multiplicity != Multiplicity::Unrestricted
                || active[..active.len() - 1].iter().any(|ancestor| {
                    match program.type_reference_table.type_reference(*ancestor) {
                        TypeReferenceNode::Named {
                            symbol: previous, ..
                        } => previous == base_symbol,
                        TypeReferenceNode::Generic {
                            base_symbol: previous,
                            ..
                        } => previous == base_symbol,
                        _ => false,
                    }
                })
                || program.attached_drop_machine_exists(*base_symbol)
            {
                return false;
            }
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                if !matches!(
                    program.type_reference_table.type_reference(*argument),
                    TypeReferenceNode::ConstExpression(_)
                ) && !eligible(program, *argument, active)
                {
                    return false;
                }
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
        | TypeReferenceNode::ConstExpression(_) => false,
    }
}
