//! Declared reference correspondence only: these queries do not establish a
//! loan, permit a reference move, or prove a field's current storage provenance.

use source::SourceMap;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::checked_argument_matches_type_reference;

fn typed_source(source: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("reference_fields.omg".into(), source.to_owned())
        .source_id;
    crate::front_end::typed_program_from_source_map(sources, &[(source_id, source)])
}

fn fixture(actual: &str, required: &str, projection: &str) -> TypedTrees {
    typed_source(&format!(
        "data Context {{ value: u64; }}
         data Other {{ value: u64; }}
         data Carrier {{ context: {actual}; spare: {actual}; }}
         data Foreign {{ context: {actual}; }}
         data Outer {{ inner: Carrier; }}
         machine inspect(carrier: &mut Carrier, outer: &mut Outer) {{
             let selected: {actual} = {projection};
         }}
         machine require(value: {required}) {{}}"
    ))
}

fn member(program: &TypedTrees, spelling: &str) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            (matches!(expression, ExpressionNode::Member(_))
                && program.expression_table.display_name(handle) == spelling)
                .then_some(handle)
        })
        .expect("fixture member expression")
}

fn required_type(program: &TypedTrees) -> TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "require")
        .expect("required type declaration");
    program.state_parameters(&program.machine_states(machine)[0])[0].type_reference
}

fn field(program: &TypedTrees, owner: &str, name: &str) -> (SymbolHandle, TypeReferenceHandle) {
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == owner)
        .expect("field owner");
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => {
                Some((field.symbol, field.type_reference))
            }
            _ => None,
        })
        .expect("declared field")
}

fn matches(program: &TypedTrees, spelling: &str) -> bool {
    checked_argument_matches_type_reference(
        program,
        member(program, spelling),
        required_type(program),
    )
}

#[test]
fn missing_conflicting_and_non_value_roots_cannot_recover_from_spelling() {
    for corruption in [
        "missing",
        "missing head",
        "missing symbol",
        "conflicting",
        "non-value",
    ] {
        let mut program = fixture("&Context", "&Context", "carrier.context");
        assert!(matches(&program, "carrier.context"));
        let foreign = program.machines()[1].symbol;
        let expression = member(&program, "carrier.context");
        let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
            unreachable!()
        };
        let receiver = member.receiver;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            unreachable!()
        };
        match corruption {
            "missing" => {
                path.symbol = SymbolHandle::invalid();
                path.head_symbol = SymbolHandle::invalid();
            }
            "missing head" => path.head_symbol = SymbolHandle::invalid(),
            "missing symbol" => path.symbol = SymbolHandle::invalid(),
            "conflicting" => path.head_symbol = foreign,
            "non-value" => {
                path.symbol = foreign;
                path.head_symbol = foreign;
            }
            _ => unreachable!(),
        }
        assert!(!matches(&program, "carrier.context"), "{corruption}");
    }
}

#[test]
fn missing_nominal_owner_identity_cannot_resolve_fields_by_type_name() {
    let mut program = fixture("&Context", "&Context", "outer.inner.context");
    assert!(matches(&program, "outer.inner.context"));
    let (_, inner_type) = field(&program, "Outer", "inner");
    program.type_reference_table.substitute_node(
        inner_type,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("Carrier"),
        },
    );
    assert!(!matches(&program, "outer.inner.context"));
}

#[test]
fn missing_receiver_and_field_types_reject_reference_correspondence() {
    for field_name in ["inner", "context"] {
        let mut program = fixture("&Context", "&Context", "outer.inner.context");
        assert!(matches(&program, "outer.inner.context"));
        let owner = if field_name == "inner" {
            "Outer"
        } else {
            "Carrier"
        };
        let (_, type_reference) = field(&program, owner, field_name);
        program
            .type_reference_table
            .substitute_node(type_reference, TypeReferenceNode::Unit);
        assert!(
            !matches(&program, "outer.inner.context"),
            "missing {owner}::{field_name} type"
        );
    }
}
