use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataField, DataMember, DataVariant};
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use crate::source_symbols::SymbolTableBuilder;
use symbols::{SymbolKind, SymbolNameRef};

#[test]
fn canonical_const_carriers_check_inactive_payloads_and_cycles() {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        crate::source_symbols::builtin_type_symbols()
            .into_iter()
            .chain([
                (SymbolKind::Data, SymbolNameRef::Static("Choice")),
                (SymbolKind::Data, SymbolNameRef::Static("Linear")),
            ]),
    );
    let choice = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
    let linear = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
    let cases = builder.insert_children(
        choice,
        [
            (SymbolKind::Variant, SymbolNameRef::Static("Empty")),
            (SymbolKind::Variant, SymbolNameRef::Static("Value")),
        ],
    );
    let field = builder
        .insert_children(
            cases.handle_at(1),
            [(SymbolKind::Field, SymbolNameRef::Static("value"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let mut named = |symbol, name: &str| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            })
    };
    let integer = named(
        children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32),
        "u64",
    );
    let choice_type = named(choice, "Choice");
    let linear_type = named(linear, "Linear");
    let empty = named(cases.handle_at(0), "Empty");
    let unconstructed = named(cases.handle_at(1), "Value");
    let payload = program.data_payload_fields.insert_many([DataField {
        symbol: field,
        name: "value".into(),
        type_reference: integer,
        ..Default::default()
    }]);
    let members = program.data_members.insert_many([
        DataMember::Variant(DataVariant {
            symbol: cases.handle_at(0),
            name: "Empty".into(),
            ..Default::default()
        }),
        DataMember::Variant(DataVariant {
            symbol: cases.handle_at(1),
            name: "Value".into(),
            payload,
            ..Default::default()
        }),
    ]);
    program.push_data_definition(DataDefinition {
        symbol: choice,
        name: "Choice".into(),
        members,
        properties: crate::source_state::data::DataProperties {
            multiplicity: language_core::Multiplicity::Unrestricted,
        },
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: linear,
        name: "Linear".into(),
        ..Default::default()
    });
    assert!(crate::source_state::canonical_const_carrier_is_supported(
        &program,
        choice_type
    ));
    // A case name is a complete value only when it has no payload.
    // Machine arguments and data indices must use the very same typed key.
    let argument = crate::source_state::expression::StaticMachineArgument {
        symbol: cases.handle_at(0),
        ..Default::default()
    };
    let key = program
        .canonical_aggregate_argument(choice_type, &argument)
        .unwrap();
    assert_eq!(
        crate::source_state::canonical_const_argument_value(&program, empty)
            .unwrap()
            .application_identity(),
        key.application_identity()
    );
    assert!(
        program
            .canonical_aggregate_argument(linear_type, &argument)
            .is_err()
    );
    assert!(crate::source_state::canonical_const_argument_value(&program, unconstructed).is_none());
    let borrowed = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: integer,
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        });
    // Even an Empty value cannot make the other case's reference or
    // linear storage eligible for a canonical constant.
    for ineligible in [borrowed, linear_type] {
        program
            .data_payload_fields
            .get_mut(payload.start())
            .type_reference = ineligible;
        assert!(!crate::source_state::canonical_const_carrier_is_supported(
            &program,
            choice_type
        ));
        assert!(crate::source_state::canonical_const_argument_value(&program, empty).is_none());
    }
    program
        .data_payload_fields
        .get_mut(payload.start())
        .type_reference = integer;
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: choice_type,
            length: FixedArrayLength::Literal(3),
        });
    assert!(crate::source_state::canonical_const_carrier_is_supported(
        &program, array
    ));
    program.type_reference_table.substitute_node(
        array,
        TypeReferenceNode::FixedArray {
            element_type: array,
            length: FixedArrayLength::Literal(3),
        },
    );
    assert!(!crate::source_state::canonical_const_carrier_is_supported(
        &program, array
    ));
    assert!(!crate::source_state::canonical_const_carrier_is_supported(
        &program,
        TypeReferenceHandle::invalid()
    ));
}

#[test]
fn a_materialized_float_keeps_its_exact_carrier_and_representation() {
    use application_identity::const_value::DecodedCanonicalConstValue;
    use numeric_literals::FloatFormat;

    let mut symbols = SymbolTableBuilder::new();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let builtins = symbols.insert_children(root, crate::source_symbols::builtin_type_symbols());
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let mut named = |atom: symbols::BuiltinTypeAtom, name: &str| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: builtins.handle_at(atom.ordinal() as u32),
                name: name.into(),
            })
    };
    let binary32 = named(symbols::BuiltinTypeAtom::F32, "f32");
    let binary64 = named(symbols::BuiltinTypeAtom::F64, "f64");
    // Negative zero and a signed signaling NaN distinguish raw custody from IEEE equality.
    for (carrier, other, format, bits) in [
        (binary32, binary64, FloatFormat::F32, 0x8000_0000),
        (binary64, binary32, FloatFormat::F64, 0xfff0_0000_0000_0123),
    ] {
        let declared = DecodedCanonicalConstValue::Float { format, bits };
        let literal = program
            .materialize_canonical_const_value(carrier, &declared)
            .unwrap();
        let expression = program.expression_table.insert(literal);
        assert!(program.canonical_const_materialization_matches(carrier, expression, &declared));
        assert!(
            program
                .materialize_canonical_const_value(other, &declared)
                .is_none()
        );
        let changed = DecodedCanonicalConstValue::Float {
            format,
            bits: bits ^ 1,
        };
        assert!(!program.canonical_const_materialization_matches(carrier, expression, &changed));
    }
}

#[test]
fn a_settled_integer_argument_has_its_landed_carrier() {
    use crate::source_state::{canonical_const_argument_value, expression::ExpressionNode};
    use application_identity::const_value::DecodedCanonicalConstValue;
    use numeric_literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
    use numerics::arithmetic::ArithmeticDomain;

    let mut program = TypedTrees::default();
    let four = IntegerLiteral::from_value(4);
    for carrier in [
        None,
        Some(LandedIntegerType::U8),
        Some(LandedIntegerType::U64),
    ] {
        let literal = carrier.map_or_else(
            || four.clone(),
            |landed_type| {
                four.with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                })
            },
        );
        let expression = program
            .expression_table
            .insert(ExpressionNode::Integer(literal));
        let argument = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(expression));
        let decoded =
            canonical_const_argument_value(&program, argument).and_then(|v| v.decode_encoding());
        assert_eq!(
            decoded,
            carrier.map(|integer_type| DecodedCanonicalConstValue::Integer {
                integer_type,
                value: 4.into(),
            })
        );
    }
}
