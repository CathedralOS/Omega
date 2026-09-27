use super::{
    ExpressionHandle, ExpressionNode, TypeReferenceHandle, TypedTrees, declared_place_type_raw,
};
use source::SourceMap;
use typed_trees::name::Identifier;

fn fixture() -> TypedTrees {
    let source = "data Small { value: u8; } data Large { value: u64; }
        data Choice { case Narrow(value: Small); case Wide(value: Large); }
        machine Choice::inspect(self) -> u64 {
            transition self {
                Choice::Narrow { value } -> 0
                Choice::Wide { value } -> value.value
            }
        }";
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("payload_projection.omg".into(), source.to_owned())
        .source_id;
    crate::front_end::typed_program_from_source_map(sources, &[(source_id, source)])
}

fn payload(program: &TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Member(member)
                if member.case_variant.as_ref().map(|name| name.as_str()) == Some("Wide") =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("selected Wide payload")
}

fn declared_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Choice::inspect")
        .expect("receiver machine");
    declared_place_type_raw(
        program,
        machine,
        Some(&program.machine_states(machine)[0]),
        expression,
    )
}

#[test]
fn conflicting_payload_symbol_cannot_use_same_spelled_field() {
    let mut program = fixture();
    let payload = payload(&program);
    assert!(declared_type(&program, payload).is_some());
    let foreign = program.machines()[0].symbol;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(payload) else {
        unreachable!()
    };
    member.member_symbol = foreign;
    assert!(declared_type(&program, payload).is_none());
}

#[test]
fn missing_case_cannot_borrow_another_cases_payload() {
    let mut program = fixture();
    let payload = payload(&program);
    assert!(declared_type(&program, payload).is_some());
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(payload) else {
        unreachable!()
    };
    member.case_variant = Some(Identifier::generated("Missing"));
    assert!(declared_type(&program, payload).is_none());
}
