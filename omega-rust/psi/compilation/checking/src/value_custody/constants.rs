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

use crate::CheckingInput;
use crate::frozen_program::expression_table;
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use symbols::{BuiltinTypeAtom, SymbolKind};
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Read only a completed scalar declaration value in its independently
/// declared carrier. This validates encoding/fit, not initializer equality;
/// initializer evaluation and source admission remain with their owners.
/// No pending observation or arbitrary initializer traversal supplies a value.
pub(crate) fn completed_integer_constant(
    program: &dyn CheckingInput,
    expression: ExpressionHandle,
) -> Option<typed_trees::closed_numeric::ClosedIntegerValue> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    if !expression_table(program).expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Name(path) = expression_table(program).expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid() || program.symbols.get(path.symbol).kind != SymbolKind::Const {
        return None;
    }
    let mut declarations = program
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.symbol == path.symbol);
    let declaration = declarations.next()?;
    if declarations.next().is_some() || declaration.pending_target_observation {
        return None;
    }
    let primitive = program.primitive_type_reference(declaration.declared_type)?;
    primitive.landed_landing_fixed()?;
    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
    crate::validate_exact_const_value_encoding(program, declaration.declared_type, encoding)
        .ok()?;
    let DecodedCanonicalConstValue::Integer { value, .. } =
        CanonicalConstValue::new("", encoding, "").decode_encoding()?
    else {
        return None;
    };
    let value = numerics::bignum::BigInt::from_i128(value);
    super::literals::land_integer_value(&value, primitive)?;
    Some(typed_trees::closed_numeric::ClosedIntegerValue {
        value,
        primitive: Some(primitive),
        type_reference: Some(declaration.declared_type),
    })
}

pub(crate) fn validate_constants(program: &dyn CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
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
    program: &dyn CheckingInput,
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
                    expression_table(program).expression(expression)
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
        match expression_table(program).expression(expression) {
            ExpressionNode::ArrayLiteral(elements) => {
                if let TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(carrier)
                {
                    pending.extend(
                        expression_table(program)
                            .expression_handles(*elements)
                            .iter()
                            .map(|element| (*element_type, *element)),
                    );
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in expression_table(program).struct_fields(literal.fields) {
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
    program: &dyn CheckingInput,
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
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> bool {
    // A representation stores T's value, not its proof-static policy operand.
    // Copy permission and every cleanup obligation still belong to the complete
    // semantic type. The typed owner authenticates the exact Core application;
    // ordinary opaque data never acquires this storage relationship by name.
    if !matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Constrained { .. }
    ) && let Some(storage) = program.layout_storage_arguments(reference)
    {
        return eligible(program, storage.semantic_type, active);
    }
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

#[cfg(test)]
mod completed_integer_tests {
    use super::*;
    use language_semantics::const_value::CanonicalConstValue;
    use symbols::{SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{
        TypedTrees,
        constant::ConstDeclaration,
        expression::{Expression, NamePath},
        name::Identifier,
        types::{PrimitiveType, TypeReferenceNode},
    };

    fn fixture(
        encoding: Option<String>,
        pending: bool,
        copies: usize,
    ) -> (TypedTrees, ExpressionHandle) {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Module, SymbolNameRef::Static("ns"))]),
        );
        let u64_symbol = children.handle_at(BuiltinTypeAtom::U64.ordinal() as u32);
        let namespace = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let symbol = builder
            .insert_children(namespace, [(SymbolKind::Const, SymbolNameRef::Static("C"))])
            .start();
        let mut program = TypedTrees::default();
        program.symbols = builder.finish();
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: u64_symbol,
                name: Identifier::generated("u64"),
            });
        for _ in 0..copies {
            program.push_const_declaration(ConstDeclaration {
                symbol,
                declared_type: carrier,
                pending_target_observation: pending,
                evaluated_value: encoding
                    .clone()
                    .map(language_core::constant_value::EvaluatedConstant::Encoded),
                ..Default::default()
            });
        }
        let expression =
            program
                .expression_table
                .insert_tree(&Expression::Name(NamePath::resolved(
                    vec![Identifier::generated("ns"), Identifier::generated("C")],
                    namespace,
                    symbol,
                )));
        (program, expression)
    }

    #[test]
    fn completed_integer_leaves_require_exact_ready_canonical_carriers() {
        let encoded = |kind: &str, value| {
            CanonicalConstValue::from_decoded(
                &language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                    type_name: kind.to_owned(),
                    value,
                },
                value.to_string(),
            )
            .expect("test integer has a canonical structural encoding")
            .encoding
        };
        for (encoding, pending, copies, expected) in [
            (Some(encoded("u64", 0)), false, 1, Some(0)),
            (
                Some(encoded("u64", i128::from(u64::MAX))),
                false,
                1,
                Some(u64::MAX),
            ),
            (Some(encoded("u8", 0)), false, 1, None),
            (Some(encoded("u64", -1)), false, 1, None),
            (Some("malformed".to_owned()), false, 1, None),
            (None, false, 1, None),
            (Some(encoded("u64", 0)), true, 1, None),
            (Some(encoded("u64", 0)), false, 0, None),
            (Some(encoded("u64", 0)), false, 2, None),
        ] {
            let (program, expression) = fixture(encoding, pending, copies);
            let value = completed_integer_constant(&program, expression);
            assert_eq!(
                value.as_ref().and_then(|value| value.value.to_u64()),
                expected
            );
            if let Some(value) = value {
                assert_eq!(value.primitive, Some(PrimitiveType::U64));
                assert!(value.type_reference.is_some());
            }
        }
    }
}
