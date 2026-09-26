use super::{DataMember, TypeReferenceHandle, TypeReferenceNode};
use crate::flow::CanonicalPlace;
use crate::flow::ownership::result_storage::is_private_result_place;
use crate::tests::front_end::typed_program;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

fn fixture(result_type: &str) -> (typed_trees::TypedTrees, CanonicalPlace) {
    let program = typed_program(&format!(
        "data Leaf {{ value: u64; }}
         data Inner {{ leaf: Leaf; }}
         data Outer {{ inner: Inner; items: [Inner; 2]; }}
         data Foreign {{ inner: Inner; }}
         data Borrowed {{ inner: &Inner; }}
         data Sliced {{ items: &[Inner]; }}
         data Envelope<T> {{ inner: T; }}
         data Choice {{ case First(inner: Inner); case Second(inner: Inner); }}
         data ForeignChoice {{ case First(inner: Inner); }}
         machine forward(value: {result_type}) -> {result_type} {{ value }}
         machine exercise(value: {result_type}) {{
             let result: {result_type} = forward(value);
         }}"
    ));
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "forward")
                .then_some(handle)
        })
        .expect("fixture forward call");
    let place = CanonicalPlace {
        root: facts::PlaceRoot::Expression(expression),
        segments: Vec::new(),
    };
    (program, place)
}

fn field(
    program: &typed_trees::TypedTrees,
    owner: &str,
    name: &str,
) -> (facts::PlaceSegment, TypeReferenceHandle) {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == owner)
        .expect("fixture field owner");
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => Some((
                facts::PlaceSegment::Field {
                    symbol: field.symbol,
                },
                field.type_reference,
            )),
            _ => None,
        })
        .expect("fixture field")
}

fn case_path(
    program: &typed_trees::TypedTrees,
    owner: &str,
    name: &str,
) -> Vec<facts::PlaceSegment> {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == owner)
        .expect("fixture case owner");
    let variant = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if program.symbols.name(variant.symbol) == name => {
                Some(variant)
            }
            _ => None,
        })
        .expect("fixture case");
    vec![
        facts::PlaceSegment::Case {
            variant: variant.symbol,
        },
        facts::PlaceSegment::Field {
            symbol: program.data_payload_fields(variant)[0].symbol,
        },
    ]
}

fn call_expression(place: &CanonicalPlace) -> ExpressionHandle {
    let facts::PlaceRoot::Expression(expression) = place.root else {
        panic!("fixture expression root")
    };
    expression
}

fn result_type(program: &typed_trees::TypedTrees, place: &CanonicalPlace) -> TypeReferenceHandle {
    let ExpressionNode::Call(call) = program.expression_table.expression(call_expression(place))
    else {
        panic!("fixture call root")
    };
    crate::flow::calls::call_target_return_type(program, call.target_symbol)
        .expect("fixture declared result")
}

#[test]
fn generic_substitution_preserves_the_reference_traversal_boundary() {
    for (argument, admitted) in [("Inner", true), ("Borrowed", false)] {
        let (program, mut place) = fixture(&format!("Envelope<{argument}>"));
        place.segments = vec![field(&program, "Envelope", "inner").0];
        if argument == "Borrowed" {
            place.segments.push(field(&program, "Borrowed", "inner").0);
        }
        place.segments.push(field(&program, "Inner", "leaf").0);
        assert_eq!(
            is_private_result_place(&program, &place),
            admitted,
            "Envelope<{argument}>: {place:?}"
        );
    }
}

#[test]
fn case_payload_paths_require_the_exact_variant_and_field_pair() {
    let (program, mut place) = fixture("Choice");
    let first = case_path(&program, "Choice", "First");
    let second = case_path(&program, "Choice", "Second");
    place.segments = first.clone();
    place.segments.push(field(&program, "Inner", "leaf").0);
    assert!(is_private_result_place(&program, &place));
    for segments in [
        case_path(&program, "ForeignChoice", "First"),
        vec![first[0]],
        vec![first[1]],
        vec![first[0], second[1]],
        vec![first[0], first[0], first[1]],
    ] {
        place.segments = segments;
        assert!(!is_private_result_place(&program, &place), "{place:?}");
    }
}

#[test]
fn unknown_call_targets_do_not_use_the_retained_call_spelling() {
    let (mut program, mut place) = fixture("Outer");
    place.segments = vec![field(&program, "Outer", "inner").0];
    assert!(is_private_result_place(&program, &place));
    let foreign = program.data_definitions()[0].symbol;
    for target in [SymbolHandle::invalid(), foreign] {
        let ExpressionNode::Call(call) = program
            .expression_table
            .expression_mut(call_expression(&place))
        else {
            panic!("fixture call")
        };
        call.target_symbol = target;
        assert!(!is_private_result_place(&program, &place));
    }
}

#[test]
fn unknown_result_and_intermediate_types_do_not_prove_private_projections() {
    for intermediate in [false, true] {
        let (mut program, mut place) = fixture("Outer");
        place.segments = vec![
            field(&program, "Outer", "inner").0,
            field(&program, "Inner", "leaf").0,
        ];
        assert!(is_private_result_place(&program, &place));
        let reference = if intermediate {
            field(&program, "Outer", "inner").1
        } else {
            result_type(&program, &place)
        };
        let TypeReferenceNode::Named { name, .. } = program
            .type_reference_table
            .type_reference(reference)
            .clone()
        else {
            panic!("fixture nominal type")
        };
        program.type_reference_table.substitute_node(
            reference,
            TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name,
            },
        );
        assert!(!is_private_result_place(&program, &place));
    }
}

#[test]
fn constrained_results_preserve_ownership_and_require_a_live_base_type() {
    for nominal in ["Outer", "&Outer"] {
        let (mut program, mut place) = fixture(nominal);
        place.segments = vec![field(&program, "Outer", "inner").0];
        let reference = result_type(&program, &place);
        let node = program
            .type_reference_table
            .type_reference(reference)
            .clone();
        let base_type = program.type_reference_table.insert(node);
        program.type_reference_table.substitute_node(
            reference,
            TypeReferenceNode::Constrained {
                base_type,
                constraints: arena::HandleSpan::empty(),
            },
        );
        assert_eq!(
            is_private_result_place(&program, &place),
            nominal == "Outer"
        );
        program.type_reference_table.substitute_node(
            reference,
            TypeReferenceNode::Constrained {
                base_type: TypeReferenceHandle::invalid(),
                constraints: arena::HandleSpan::empty(),
            },
        );
        assert!(!is_private_result_place(&program, &place));
    }
}

#[test]
fn caller_places_and_non_call_expression_roots_are_not_private_results() {
    let (mut program, mut place) = fixture("Outer");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("fixture caller");
    let caller_parameter = program.state_parameters(&program.machine_states(machine)[0])[0].symbol;
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    for root in [
        facts::PlaceRoot::Unknown,
        facts::PlaceRoot::Symbol(caller_parameter),
        facts::PlaceRoot::Expression(boolean),
        facts::PlaceRoot::Expression(ExpressionHandle::invalid()),
        facts::PlaceRoot::TypeReference(result_type(&program, &place)),
    ] {
        place.root = root;
        assert!(!is_private_result_place(&program, &place), "{root:?}");
    }
}
