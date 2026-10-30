use super::*;
use arena::HandleSpan;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{
        Expression, NamePath, TableBinaryExpression, TableMatchArm, TableMatchExpression,
    },
    name::Identifier,
    signature::StateParameter,
    state::State,
};

fn fixture() -> (TypedTrees, State, SymbolHandle, HandleSpan<ProofFact>) {
    let mut symbols = SymbolTableBuilder::default();
    let integer = symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
    let pair = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Pair"));
    let fields = symbols.insert_children(
        pair,
        [
            (SymbolKind::Field, SymbolNameRef::Static("lo")),
            (SymbolKind::Field, SymbolNameRef::Static("hi")),
        ],
    );
    let envelope = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Envelope"));
    let members = symbols.insert_children(
        envelope,
        [
            (SymbolKind::Field, SymbolNameRef::Static("marker")),
            (SymbolKind::Variant, SymbolNameRef::Static("Empty")),
            (SymbolKind::Variant, SymbolNameRef::Static("Point")),
        ],
    );
    let payload = symbols
        .insert_children(
            members.handle_at(2),
            [(SymbolKind::Field, SymbolNameRef::Static("pair"))],
        )
        .handle_at(0);
    let state_symbol = symbols.insert_root(SymbolKind::State, SymbolNameRef::Static("entry"));
    let parameter = symbols
        .insert_children(
            state_symbol,
            [(SymbolKind::Parameter, SymbolNameRef::Static("value"))],
        )
        .handle_at(0);
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let mut named = |symbol, name: &str| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::from(name),
            })
    };
    let integer_type = named(integer, "u64");
    let pair_type = named(pair, "Pair");
    let envelope_type = named(envelope, "Envelope");
    let mut pair_members = HandleSpan::empty();
    let mut operands = Vec::new();
    for (position, name) in ["lo", "hi"].into_iter().enumerate() {
        let symbol = fields.handle_at(position as u32);
        program.data_members.append_to_span(
            &mut pair_members,
            DataMember::Field(DataField {
                identity: Some(20 + position as u64),
                symbol,
                name: Identifier::from(name),
                type_reference: integer_type,
                ..Default::default()
            }),
        );
        operands.push(
            program
                .expression_table
                .insert_tree(&Expression::Name(NamePath::resolved(
                    vec![Identifier::from(name)],
                    symbol,
                    symbol,
                ))),
        );
    }
    let equality = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: operands[0],
            operator: BinaryOperator::Equal,
            right: operands[1],
        }));
    let mut pair_facts = HandleSpan::empty();
    program
        .proof_facts
        .append_to_span(&mut pair_facts, ProofFact::Expression(equality));
    program.push_data_definition(DataDefinition {
        symbol: pair,
        name: Identifier::from("Pair"),
        members: pair_members,
        where_facts: pair_facts,
        ..Default::default()
    });
    let mut payload_fields = HandleSpan::empty();
    program.data_payload_fields.append_to_span(
        &mut payload_fields,
        DataField {
            identity: Some(10),
            symbol: payload,
            name: Identifier::from("pair"),
            type_reference: pair_type,
            ..Default::default()
        },
    );
    let mut envelope_members = HandleSpan::empty();
    program.data_members.append_to_span(
        &mut envelope_members,
        DataMember::Field(DataField {
            identity: Some(0),
            symbol: members.handle_at(0),
            name: Identifier::from("marker"),
            type_reference: integer_type,
            ..Default::default()
        }),
    );
    for (position, name) in [(1, "Empty"), (2, "Point")] {
        program.data_members.append_to_span(
            &mut envelope_members,
            DataMember::Variant(typed_trees::data::DataVariant {
                identity: Some(position as u64),
                symbol: members.handle_at(position),
                name: Identifier::from(name),
                payload: if position == 2 {
                    payload_fields
                } else {
                    HandleSpan::empty()
                },
                ..Default::default()
            }),
        );
    }
    program.push_data_definition(DataDefinition {
        symbol: envelope,
        name: Identifier::from("Envelope"),
        members: envelope_members,
        ..Default::default()
    });
    let borrowed = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: envelope_type,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    let mut parameters = HandleSpan::empty();
    program.state_parameters.append_to_span(
        &mut parameters,
        StateParameter {
            symbol: parameter,
            name: Identifier::from("value"),
            type_reference: borrowed,
            ..Default::default()
        },
    );
    (
        program,
        State {
            symbol: state_symbol,
            parameters,
            ..Default::default()
        },
        envelope,
        pair_facts,
    )
}

#[test]
fn declaration_roster_guards_owned_payload_coupling_before_observing_fields() {
    let (program, state, envelope, _) = fixture();
    let operators = CheckedOperatorFacts::default();
    let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
    assert!(declaration_facts_carried(&program, &operators, definition));
    let rows = state_declaration_predicates(&program, &operators, &state);
    assert_eq!(
        rows.len(),
        1,
        "the Point payload's Pair equality must not disappear"
    );
    assert!(rows[0].restored_on_return);
    let identity = |ordinal, name| {
        checked_trees::CheckedMemberIdentity::of_member(Some(ordinal), Identifier::from(name))
    };
    let leaf = |ordinal, name| CheckedScalarExpression::StructuralParameterField {
        observation: Default::default(),
        parameter_position: 0,
        path: vec![
            CheckedStructuralPredicatePathSegment::Case(identity(2, "Point")),
            CheckedStructuralPredicatePathSegment::Field(identity(10, "pair")),
            CheckedStructuralPredicatePathSegment::Field(identity(ordinal, name)),
        ],
        primitive_type: PrimitiveType::U64,
    };
    assert_eq!(
        rows[0].predicate,
        CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Not(Box::new(
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: checked_trees::CheckedStructuralCaseSubject {
                        root: checked_trees::CheckedStructuralCaseRoot::Parameter { position: 0 },
                        path: Vec::new(),
                    },
                    case: identity(2, "Point"),
                }
            ))),
            right: Box::new(CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: Box::new(leaf(20, "lo")),
                right: Box::new(leaf(21, "hi")),
            }),
        }
    );
}

#[test]
fn outer_storage_cannot_hide_an_unsupported_payload_predicate() {
    let (mut program, _, envelope, facts) = fixture();
    // A fully Boolean match is a legitimate predicate shape, but not part of
    // this bounded row vocabulary. Its equivalent-looking branch results do
    // not authorize dropping it or using a necessary interval as its meaning.
    let condition = program.proof_facts.span_or_empty(facts)[0]
        .as_expression()
        .copied()
        .unwrap();
    let yes = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let no = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let capture = program.expression_table.insert_match_subject(condition);
    let arms = program.expression_table.insert_match_arms([
        TableMatchArm {
            pattern: typed_trees::expression::MatchPattern::Value(yes),
            value: yes,
            ..Default::default()
        },
        TableMatchArm {
            pattern: typed_trees::expression::MatchPattern::Value(no),
            value: no,
            ..Default::default()
        },
    ]);
    let predicate = program
        .expression_table
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject: condition,
            capture,
            arms,
        }));
    program
        .expression_table
        .insert(ExpressionNode::SavedSubject(
            typed_trees::expression::TableSavedSubject {
                capture,
                pattern_head: yes,
            },
        ));
    program.proof_facts.span_mut_or_empty(facts)[0] = ProofFact::Expression(predicate);
    let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
    assert!(definition.where_facts.is_empty());
    assert!(
        !declaration_facts_carried(&program, &CheckedOperatorFacts::default(), definition),
        "an unrelated Envelope marker store must not bypass the nested complete-domain fence"
    );
}

#[test]
fn unresolved_nested_array_extents_cannot_hide_payload_requirements() {
    let (mut program, _, envelope, _) = fixture();
    let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
    let payload = program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_variant())
        .find(|case| !case.payload.is_empty())
        .unwrap()
        .payload;
    let pair_type = program.data_payload_fields.span_or_empty(payload)[0].type_reference;
    // This is a pending owned extent, not permission for the roster to run a
    // source evaluator or silently treat an array as empty. The inner array's
    // unresolved extent must not hide Pair's equality from the outer query.
    let extent = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    let extent = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(extent));
    let inner = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: pair_type,
            length: FixedArrayLength::Expression(extent),
        });
    let outer = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: inner,
            length: FixedArrayLength::Expression(extent),
        });
    program.data_payload_fields.span_mut_or_empty(payload)[0].type_reference = outer;
    let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
    assert!(!declaration_facts_carried(
        &program,
        &CheckedOperatorFacts::default(),
        definition
    ));
}

mod own_fact_reuse;
