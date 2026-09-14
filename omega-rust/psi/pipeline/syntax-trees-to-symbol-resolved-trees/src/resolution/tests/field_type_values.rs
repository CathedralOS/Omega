use super::*;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{DataField, DataMember};
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use symbols::SymbolHandle;

fn resolve(source: &str) -> SymbolResolvedTrees {
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
    lower_syntax_trees(&syntax).expect("resolve field type expressions")
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
fn field_type_values_keep_generic_common_and_payload_subject_identity() {
    let program = resolve(
        "const Limit: u64 = 17;
        data Bounds<const Limit: u64> {
            maximum: u64;
            generic: u64[0..=Limit];
            common: u64[0..=maximum];
            case Some(maximum: u64, payload: u64[0..=maximum], inherited: u64[0..=Limit]);
        }",
    );
    let definition = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Bounds")
        .unwrap();
    let binder = program.data_type_parameters(definition.type_parameters)[0].symbol;
    let fields = fields(&program, "Bounds");
    for (position, expected) in [
        (1, binder),
        (2, fields[0].symbol),
        (4, fields[3].symbol),
        (5, binder),
    ] {
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(endpoint(&program, &fields[position].type_reference))
        else {
            panic!("symbolic endpoint")
        };
        assert!(expected.is_valid());
        assert_eq!(path.head_symbol, expected);
        assert_eq!(path.symbol, expected);
    }
}

#[test]
fn field_type_values_do_not_turn_runtime_receivers_into_global_type_qualifiers() {
    let program = resolve(
        "data Limits {} machine Limits::capacity() -> u64 {256} data Empty {}
        machine capacity() -> u64 {17}
        data Record { Limits: Empty; value: u64[0..=Limits.capacity()]; }
        data Choice { case Some(Limits: Empty, value: u64[0..=Limits.capacity()]); }",
    );
    for owner in ["Record", "Choice"] {
        let fields = fields(&program, owner);
        let expressions = &program.tables.bodies.expressions;
        let ExpressionNode::Call(call) =
            expressions.expression(endpoint(&program, &fields[1].type_reference))
        else {
            panic!("receiver call")
        };
        let ExpressionNode::Name(receiver) = expressions.expression(call.receiver) else {
            panic!("runtime field receiver")
        };
        assert_eq!(receiver.head_symbol, fields[0].symbol);
        assert_eq!(receiver.symbol, fields[0].symbol);
        // This field's declared type has no runtime-receiver method. Neither
        // the type-qualified helper nor the free helper is a valid fallback.
        assert!(!call.target_symbol.is_valid());
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

#[test]
fn field_type_values_resolve_new_fields_against_retained_declarations() {
    let base_source = "machine capacity() -> u64 {256} data Base {value: u64[0..=capacity()];}";
    let extension_source = "data Added {value: u64[0..=capacity()];}";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let syntax =
        parse_syntax_trees_with_id(base_id, &Lexer::new(base_source).tokenize().unwrap()).unwrap();
    let base = lower_syntax_trees_with_sources(&syntax, Arc::new(sources.clone())).unwrap();
    let target = entry(&base, "capacity");
    assert_call(&base, fields(&base, "Base")[0], target);
    let extension = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source).tokenize().unwrap(),
    )
    .unwrap();
    let program = lower_syntax_extension_against_resolved_base(
        base,
        &extension,
        Arc::new(sources),
        Vec::new(),
    )
    .unwrap();
    for owner in ["Base", "Added"] {
        assert_call(&program, fields(&program, owner)[0], target);
    }
}
