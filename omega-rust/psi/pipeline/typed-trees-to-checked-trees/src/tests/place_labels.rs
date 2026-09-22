//! The canonical place label is the text diagnostics quote for a fact place
//! (`self.items[0]`, `self.nat::Succ.previous`). Facts owns the renderer;
//! this pins the exact spelling for every root and segment kind so the
//! diagnostics this crate emits cannot drift silently.

use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use facts::{PlaceRoot, PlaceSegment};
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::ExpressionNode;

const SOURCE: &str = r#"
    data Inner { value: u64; }
    data Nat { case Zero; case Succ(previous: Nat); }
    data Carrier {
        count: u64;
        inner: Inner;
        nat: Nat;
        items: [u64; 4];
        bytes: [u8; 8];
    }

    machine Carrier::run(&mut self) {
        let picked: u64 = 1;
    }
"#;

fn parse_typed_trees(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn field_symbol(program: &TypedTrees, data_name: &str, field_name: &str) -> SymbolHandle {
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == data_name)
        .unwrap_or_else(|| panic!("data {data_name}"));
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name => Some(field.symbol),
            DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.name.as_str() == field_name)
                .map(|field| field.symbol),
            DataMember::Field(_) => None,
        })
        .unwrap_or_else(|| panic!("field {data_name}.{field_name}"))
}

fn variant_symbol(program: &TypedTrees, data_name: &str, variant_name: &str) -> SymbolHandle {
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == data_name)
        .unwrap_or_else(|| panic!("data {data_name}"));
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.name.as_str() == variant_name => {
                Some(variant.symbol)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("variant {data_name}::{variant_name}"))
}

/// The `self` parameter and the `picked` local of `Carrier::run`.
fn receiver_and_local(program: &TypedTrees) -> (SymbolHandle, SymbolHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Carrier::run")
        .expect("machine Carrier::run");
    let state = program
        .machine_states(machine)
        .first()
        .expect("run has one state");
    let receiver = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .expect("self parameter")
        .symbol;
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        })
        .expect("picked local");
    (receiver, local)
}

#[test]
fn place_labels_spell_every_root_and_segment_kind() {
    let mut program = parse_typed_trees(SOURCE);
    let (receiver, local) = receiver_and_local(&program);
    let count = field_symbol(&program, "Carrier", "count");
    let inner = field_symbol(&program, "Carrier", "inner");
    let value = field_symbol(&program, "Inner", "value");
    let nat = field_symbol(&program, "Carrier", "nat");
    let succ = variant_symbol(&program, "Nat", "Succ");
    let previous = field_symbol(&program, "Nat", "previous");
    let items = field_symbol(&program, "Carrier", "items");
    let bytes = field_symbol(&program, "Carrier", "bytes");
    let three = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
    let count_type = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Carrier")
        .and_then(|data| {
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field) if field.symbol == count => Some(field.type_reference),
                    _ => None,
                })
        })
        .expect("count type");

    let label = |root: PlaceRoot, segments: &[PlaceSegment]| {
        facts::canonical_place_label_from_parts(&program, root, segments)
    };
    let receiver_root = PlaceRoot::Symbol(receiver);

    assert_eq!(label(receiver_root, &[]), "self");
    assert_eq!(
        label(receiver_root, &[PlaceSegment::Field { symbol: count }]),
        "self.count"
    );
    assert_eq!(
        label(
            receiver_root,
            &[
                PlaceSegment::Field { symbol: inner },
                PlaceSegment::Field { symbol: value },
            ]
        ),
        "self.inner.value"
    );
    assert_eq!(
        label(
            receiver_root,
            &[
                PlaceSegment::Field { symbol: nat },
                PlaceSegment::Case { variant: succ },
                PlaceSegment::Field { symbol: previous },
            ]
        ),
        "self.nat::Succ.previous"
    );
    assert_eq!(
        label(
            receiver_root,
            &[
                PlaceSegment::Field { symbol: items },
                PlaceSegment::FixedIndex { index: 0 },
            ]
        ),
        "self.items[0]"
    );
    assert_eq!(
        label(
            receiver_root,
            &[
                PlaceSegment::Field { symbol: bytes },
                PlaceSegment::FixedRange { start: 0, end: 4 },
            ]
        ),
        "self.bytes[0..4]"
    );
    assert_eq!(
        label(
            receiver_root,
            &[
                PlaceSegment::Field { symbol: items },
                PlaceSegment::Index { expression: three },
            ]
        ),
        "self.items[3]"
    );
    assert_eq!(
        label(
            PlaceRoot::Symbol(local),
            &[PlaceSegment::Field { symbol: count }]
        ),
        "picked.count"
    );
    assert_eq!(label(PlaceRoot::Expression(three), &[]), "3");
    assert_eq!(label(PlaceRoot::TypeReference(count_type), &[]), "u64");
    assert_eq!(label(PlaceRoot::Unknown, &[]), "unknown");
    assert_eq!(
        label(PlaceRoot::Symbol(SymbolHandle::invalid()), &[]),
        "unknown"
    );
}

/// The receiver-rooted label is the one `language_core::receiver_binding`
/// builds and takes apart, so the two spellings must agree.
#[test]
fn receiver_rooted_labels_round_trip_through_language_core() {
    let program = parse_typed_trees(SOURCE);
    let (receiver, _) = receiver_and_local(&program);
    let items = field_symbol(&program, "Carrier", "items");
    let label = facts::canonical_place_label_from_parts(
        &program,
        PlaceRoot::Symbol(receiver),
        &[
            PlaceSegment::Field { symbol: items },
            PlaceSegment::FixedIndex { index: 2 },
        ],
    );
    assert_eq!(label, language_core::receiver_place_label("items[2]"));
    assert_eq!(
        language_core::receiver_place_field(&label),
        Some("items[2]")
    );
}
