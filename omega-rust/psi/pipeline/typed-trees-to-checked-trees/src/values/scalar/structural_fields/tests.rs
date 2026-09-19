use super::{exact_self_parameter, structural_parameter_field_path};
use checked_trees::CheckedStructuralPredicatePathSegment;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceNode;

fn byte_length_fixture(source: &str) -> (TypedTrees, ExpressionHandle) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let ExpressionNode::Binary(relation) =
        program.expression_table.expression(requirement(&program))
    else {
        panic!("length requirement");
    };
    let length = relation.left;
    (program, length)
}

#[test]
fn byte_field_length_retains_exact_path_and_rejects_wrong_members() {
    let (program, length) = byte_length_fixture(
        "domain [u8; 16]::Utf8 requires valid_utf8(self);
         data Input { text: [u8; 16] in Utf8; }
         data Other { text: [u8; 16] in Utf8; }
         machine Input::measure(&mut self) -> u64
         requires self.text.len == 0 { 0 }",
    );
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    assert!(matches!(
        super::structural_sequence_length(&program, parameters, length),
        Some(checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
            parameter_position: 0, path,
        }) if path == [CheckedStructuralPredicatePathSegment::Field("text".into())]
    ));
    let DataMember::Field(foreign) = &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign text field");
    };
    let ExpressionNode::Member(member) = program.expression_table.expression(length) else {
        panic!("length member");
    };
    let receiver = member.receiver;
    for target in [length, receiver] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(target) else {
            panic!("selected member");
        };
        member.member_symbol = foreign.symbol;
        assert!(super::structural_sequence_length(&invalid, parameters, length).is_none());
    }
    let mut wrong_spelling = program.clone();
    let ExpressionNode::Member(member) = wrong_spelling.expression_table.expression_mut(length)
    else {
        panic!("length member");
    };
    member.member = "capacity".into();
    assert!(super::structural_sequence_length(&wrong_spelling, parameters, length).is_none());
}

#[test]
fn byte_length_keeps_whole_views_distinct_from_field_carriers() {
    for access in ["&", "&mut "] {
        let (program, length) = byte_length_fixture(&format!(
            "machine measure(input: {access}[u8]) -> u64 requires input.len == 0 {{ 0 }}"
        ));
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        assert!(matches!(
            super::structural_sequence_length(&program, parameters, length),
            Some(checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
                parameter_position: 0, path,
            }) if path.is_empty()
        ));
    }
    for field_type in ["u64", "NamedLength"] {
        let (program, length) = byte_length_fixture(&format!(
            "data NamedLength {{ len: u64; }}
             data Input {{ text: {field_type}; }}
             machine measure(input: &Input) -> u64 requires input.text.len == 0 {{ 0 }}"
        ));
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        assert!(
            super::structural_sequence_length(&program, parameters, length).is_none(),
            "nonsequences and nominal len fields are not builtin sequence lengths"
        );
    }
}

#[test]
fn fixed_array_length_retains_static_extent_through_exact_projections() {
    for source in [
        "data Input { bytes: [u8; 32]; }
         machine Input::measure(&mut self) -> u64 requires self.bytes.len == 0 { 0 }",
        "data Inner { bytes: [u16; 32]; }
         data Input { inner: Inner; }
         machine measure(input: &Input) -> u64 requires input.inner.bytes.len == 0 { 0 }",
        "machine measure(input: &[u8; 32]) -> u64 requires input.len == 0 { 0 }",
        "machine measure(input: [u8; 32]) -> u64 requires input.len == 0 { 0 }",
    ] {
        let (program, length) = byte_length_fixture(source);
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        assert!(matches!(
            super::structural_sequence_length(&program, parameters, length),
            Some(checked_trees::CheckedScalarExpression::IntegerLiteral { literal })
                if literal.value_u64() == Some(32)
                    && literal.landing().is_some_and(|landing|
                        landing.landed_type == numerics::literals::LandedIntegerType::U64
                            && landing.domain == numerics::arithmetic::ArithmeticDomain::Exact)
        ));
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(length) else {
            panic!("length member");
        };
        member.member = "capacity".into();
        assert!(super::structural_sequence_length(&invalid, parameters, length).is_none());
    }
}

#[test]
fn boolean_field_value_rejoins_the_exact_declared_root_and_path() {
    let program = fixture();
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    let expression = requirement(&program);
    assert!(
        matches!(super::lower_structural_parameter_field(&program, parameters, expression),
        Some((checked_trees::CheckedScalarExpression::Boolean(value), _))
            if matches!(value.as_ref(), checked_trees::CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 0, path,
            } if path == &[CheckedStructuralPredicatePathSegment::Field("allowed".into())]))
    );
    let foreign_parameters =
        program.state_parameters(&program.machine_states(&program.machines()[1])[0]);
    assert!(
        super::lower_structural_parameter_field(&program, foreign_parameters, expression).is_none()
    );
    let DataMember::Field(foreign) = &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign field");
    };
    let mut unresolved = program.clone();
    let ExpressionNode::Member(member) = unresolved.expression_table.expression_mut(expression)
    else {
        panic!("field source");
    };
    member.member_symbol = SymbolHandle::invalid();
    assert!(
        super::lower_structural_parameter_field(&unresolved, parameters, expression).is_some(),
        "absent typed selection uses exact canonical receiver resolution"
    );
    for symbol in [
        foreign.symbol,
        SymbolHandle::from_parts(
            foreign.symbol.arena_index(),
            foreign.symbol.generation() + 1,
        ),
    ] {
        let mut changed = program.clone();
        let ExpressionNode::Member(member) = changed.expression_table.expression_mut(expression)
        else {
            panic!("field source");
        };
        member.member_symbol = symbol;
        assert!(
            super::lower_structural_parameter_field(&changed, parameters, expression).is_none()
        );
    }
}

fn fixture() -> TypedTrees {
    let source = "data Input { allowed: bool; }
        data Other { allowed: bool; }
        machine Input::read(&self, fallback: bool) -> bool
        requires self.allowed
        { transition { _ -> finish(fallback) }
          state finish(other: bool) -> bool { other } }
        machine Other::read(&self) -> bool { true }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn requirement(program: &TypedTrees) -> ExpressionHandle {
    program
        .machine_contracts(&program.machines()[0])
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("self requirement")
}

fn path(program: &TypedTrees) -> Option<Vec<CheckedStructuralPredicatePathSegment>> {
    let entry = &program.machine_states(&program.machines()[0])[0];
    let mut path = Vec::new();
    let position = structural_parameter_field_path(
        program,
        program.state_parameters(entry),
        requirement(program),
        &mut path,
    )?;
    assert_eq!(position, 0);
    Some(path)
}

#[test]
fn runtime_self_alias_preserves_identity_and_rejects_missing_or_foreign_fields() {
    let program = fixture();
    assert_eq!(
        path(&program),
        Some(vec![CheckedStructuralPredicatePathSegment::Field(
            "allowed".into()
        )])
    );
    let DataMember::Field(foreign) = &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign field");
    };
    let root = requirement(&program);
    let ExpressionNode::Member(member) = program.expression_table.expression(root) else {
        panic!("self field");
    };
    let stale = SymbolHandle::from_parts(
        member.member_symbol.arena_index(),
        member.member_symbol.generation() + 1,
    );
    for selected in [SymbolHandle::invalid(), foreign.symbol, stale] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(root) else {
            panic!("self field");
        };
        member.member_symbol = selected;
        assert!(path(&invalid).is_none());
    }
    let mut numbered = program.clone();
    let members = numbered.data_definitions()[0].members;
    let DataMember::Field(field) = &mut numbered.tables.data_members.span_mut_or_empty(members)[0]
    else {
        panic!("original field");
    };
    field.identity = Some(7);
    assert_eq!(
        path(&numbered),
        Some(vec![CheckedStructuralPredicatePathSegment::Field(
            "#7".into()
        )])
    );
}

#[test]
fn runtime_self_alias_requires_the_exact_first_state_telescope() {
    let program = fixture();
    let states = program.machine_states(&program.machines()[0]);
    let parameters = program.state_parameters(&states[0]);
    let ExpressionNode::Member(member) = program.expression_table.expression(requirement(&program))
    else {
        panic!("self field");
    };
    let receiver = member.receiver;
    assert!(exact_self_parameter(&program, parameters, receiver).is_some());
    assert!(
        exact_self_parameter(&program, program.state_parameters(&states[1]), receiver).is_none()
    );
    let mut duplicate = parameters.to_vec();
    duplicate.push(parameters[0].clone());
    assert!(exact_self_parameter(&program, &duplicate, receiver).is_none());
    let mut reordered = parameters.to_vec();
    reordered.reverse();
    assert!(exact_self_parameter(&program, &reordered, receiver).is_none());
    let mut invalid = program.clone();
    let foreign_parameter =
        program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol;
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(states[0].parameters)[0]
        .symbol = foreign_parameter;
    assert!(path(&invalid).is_none());
}

#[test]
fn runtime_self_alias_rejects_wrong_owner_types_and_stale_or_excessive_wrappers() {
    let program = fixture();
    let parameters = program.machine_states(&program.machines()[0])[0].parameters;
    for (symbol, name) in [
        (program.data_definitions()[1].symbol, "Other"),
        (program.data_definitions()[0].symbol, "Self"),
        (SymbolHandle::invalid(), "Self"),
    ] {
        let mut invalid = program.clone();
        let reference = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            });
        invalid
            .tables
            .state_parameters
            .span_mut_or_empty(parameters)[0]
            .type_reference = reference;
        assert!(path(&invalid).is_none());
    }
    let mut invalid = program.clone();
    let original = invalid.tables.state_parameters.span_or_empty(parameters)[0].type_reference;
    let stale = typed_trees::types::TypeReferenceHandle::from_parts(
        original.arena_index(),
        original.generation() + 1,
    );
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(parameters)[0]
        .type_reference = stale;
    assert!(path(&invalid).is_none());
    let mut invalid = program.clone();
    let mut reference = original;
    for _ in 0..65 {
        reference = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: reference,
                constraints: arena::HandleSpan::empty(),
            });
    }
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(parameters)[0]
        .type_reference = reference;
    assert!(path(&invalid).is_none());
}
