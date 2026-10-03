use crate::ResolutionRequest;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{DataField, DataMember};
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use symbols::SymbolHandle;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve(source: &str) -> SymbolResolvedTrees {
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
    crate::resolve(ResolutionRequest::new(syntax)).expect("resolve field type expressions")
}

fn fields<'program>(
    program: &'program SymbolResolvedTrees,
    owner: &str,
) -> Vec<&'program DataField> {
    let definition = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == owner)
        .unwrap();
    program
        .data_members(definition.members)
        .iter()
        .flat_map(|member| match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(variant) => program.data_payload_fields(variant.payload),
        })
        .collect()
}

fn endpoint(program: &SymbolResolvedTrees, reference: &TypeReference) -> ExpressionHandle {
    match reference {
        TypeReference::Constrained(constrained) => program
            .tables
            .types
            .constraints
            .span_or_empty(constrained.constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraint::Range { maximum, .. } => Some(*maximum),
                _ => None,
            })
            .expect("range endpoint"),
        TypeReference::Reference(reference) => {
            endpoint(program, program.child_type_reference(reference.referee))
        }
        TypeReference::FixedArray(array) => {
            endpoint(program, program.child_type_reference(array.element_type))
        }
        TypeReference::Slice(slice) => {
            endpoint(program, program.child_type_reference(slice.element_type))
        }
        _ => panic!("range or containing type"),
    }
}

fn entry(program: &SymbolResolvedTrees, name: &str) -> SymbolHandle {
    let machine = program
        .machines
        .iter()
        .find(|machine| program.symbols.display_path(machine.symbol, "::") == name)
        .unwrap();
    program
        .machine_state(program.machine_state_handles(machine.states)[0])
        .symbol
}

fn assert_call(program: &SymbolResolvedTrees, field: &DataField, target: SymbolHandle) {
    let ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(endpoint(program, &field.type_reference))
    else {
        panic!("computed endpoint")
    };
    assert!(target.is_valid());
    assert_eq!(call.target_symbol, target);
}

#[test]
fn field_type_values_resolve_calls_inside_records_payloads_and_nested_types() {
    let program = resolve(
        "machine capacity() -> u64 {17}
        data Limits {} machine Limits::capacity() -> u64 {256}
        data Other {} machine Other::capacity() -> u64 {511}
        data Bounds {
            direct: u64[0..=capacity()];
            borrowed: &u64[0..=Limits::capacity()];
            array: [u64[0..=Other::capacity()]; 2];
            slice: &[u64[0..=capacity()]];
            case Some(value: u64[0..=Limits::capacity()]);
        }",
    );
    let expected = [
        "capacity",
        "Limits::capacity",
        "Other::capacity",
        "capacity",
        "Limits::capacity",
    ];
    let fields = fields(&program, "Bounds");
    assert_eq!(fields.len(), expected.len());
    for (field, target) in fields.into_iter().zip(expected) {
        assert_call(&program, field, entry(&program, target));
    }
}

#[test]
fn field_type_values_keep_payload_callable_binders_and_field_types() {
    let program = resolve("machine capacity() -> u64 {17}
        data Bounds<machine capacity> where machine capacity() -> u64; {
            case Some(value: u64[0..=capacity()]);
        }
        data Limits {} machine Limits::capacity() -> u64 {256}
        data Other {} machine Other::capacity() -> u64 {511}
        data Choice { receiver: Limits; case Some(receiver: Other, value: u64[0..=receiver.capacity()]); }");
    let definition = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Bounds")
        .unwrap();
    let binder = program.data_type_parameters(definition.type_parameters)[0].symbol;
    assert_call(&program, fields(&program, "Bounds")[0], binder);
    assert_call(
        &program,
        fields(&program, "Choice")[2],
        entry(&program, "Other::capacity"),
    );
}
