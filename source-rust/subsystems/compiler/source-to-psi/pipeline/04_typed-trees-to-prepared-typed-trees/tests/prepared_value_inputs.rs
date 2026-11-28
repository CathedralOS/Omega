//! Stage 04 must hand lowering every constant a prepared value was computed
//! from, by the declaration the source named, and the value the source
//! driver evaluated for that exact declaration and operand. Lowering reads the
//! constants from `PreparedTypedTrees::constants()` and their values from the
//! declarations; it does not look a name up again. So a constant that names
//! the wrong declaration, or a call evaluated with another call's operand,
//! becomes a program that computes with the wrong number.
//!
//! The spec rule is in `wiki/spec/language/constants.md`, "Evaluation": "A
//! constant position requests semantic evaluation", and in
//! `wiki/spec/language/evaluation.md`, which says what an evaluation reads.
//!
//! Each program below is built so that a wrong choice is still a valid
//! program with a different answer: two constants with the same value, two
//! calls to one machine in two files, and a record
//! whose fields are written in the opposite order to their declaration.

#[path = "support/prepared_package.rs"]
mod package;

use build_model::values::BuildTimeValue;
use package::{Demand, integer, prepare, typed};
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::test_support::preparation::{
    PreparedValue, TargetInputs, ValueInputs, prepare_typed_trees,
};
use symbols::SymbolHandle;

fn aliases(selected: &str) -> String {
    format!("const FIRST: u8 = 7u8; const OTHER: u8 = 7u8; const COPY: u8 = {selected};")
}

/// `COPY` names `FIRST` or `OTHER`, and both hold 7. The value alone cannot
/// tell them apart, so only the recorded input says which declaration `COPY`
/// depends on.
#[test]
fn an_alias_lists_the_constant_it_names_not_another_with_the_same_value() {
    for (selected, unselected) in [("FIRST", "OTHER"), ("OTHER", "FIRST")] {
        let prepared = prepare(&[("main.omg", &aliases(selected))], Demand::Package).unwrap();
        let copy = prepared.inputs_for("COPY");
        assert_eq!(copy.constants, [selected], "COPY = {selected}");
        assert_eq!(copy.targets, TargetInputs::None);
        for path in ["COPY", "FIRST", "OTHER"] {
            assert_eq!(integer(prepared.value(path)), Some(7), "{path}");
        }
        for row in prepared.inputs() {
            assert!(
                !row.constants.iter().any(|path| path == unselected),
                "{row:?} lists {unselected}, which COPY does not name"
            );
        }
    }
}

/// `RECORD` calls `aggregate`, which reads `SEED` into a local it never uses.
/// The read is still part of the computation, so `SEED` is an input of
/// `RECORD`. `PLAIN` calls a machine that reads no constant, and nothing
/// reads `UNREAD`, which has the same value as `SEED`.
#[test]
fn a_constant_read_inside_a_called_machine_is_an_input_of_the_caller() {
    let program = "const SEED: u8 = 9u8; const UNREAD: u8 = 9u8; \
        machine aggregate(x: u8) -> u8 crashes none { let seed: u8 = SEED; x } \
        machine plain(x: u8) -> u8 crashes none { x } \
        const RECORD: u8 = aggregate(5u8); const PLAIN: u8 = plain(5u8);";
    let prepared = prepare(&[("main.omg", program)], Demand::Package).unwrap();
    assert_eq!(prepared.inputs_for("RECORD").constants, ["SEED"]);
    assert!(prepared.inputs_for("PLAIN").constants.is_empty());
    for row in prepared.inputs() {
        assert!(
            !row.constants.iter().any(|path| path == "UNREAD"),
            "{row:?}"
        );
    }
    assert_eq!(integer(prepared.value("RECORD")), Some(5));
    assert_eq!(integer(prepared.value("PLAIN")), Some(5));
}

const WIDTH: &str = "public machine width(n: u8) -> u8 crashes none { n +% 1u8 }";

/// `A` and `B` call `width` from two files, so their initializers are the
/// first expression of their own file and share expression numbers and
/// offsets. Each constant must still be evaluated with its own operand, in
/// either file order.
#[test]
fn calls_in_two_files_are_evaluated_with_their_own_operands() {
    let a = (
        "a.omg",
        "use machines.omg; const A: u8 = |machines|::width(7u8);",
    );
    let b = (
        "b.omg",
        "use machines.omg; const B: u8 = |machines|::width(9u8);",
    );
    let machines = ("machines.omg", WIDTH);
    for files in [[a, b, machines], [machines, b, a]] {
        let prepared = package::prepare_with_imports(
            &files,
            Demand::Package,
            &[
                ("product/a.omg", "product/machines.omg", "machines"),
                ("product/b.omg", "product/machines.omg", "machines"),
            ],
        )
        .unwrap();
        assert_eq!(integer(prepared.value("A")), Some(8), "{files:?}");
        assert_eq!(integer(prepared.value("B")), Some(10), "{files:?}");
        assert_eq!(
            prepared.evaluations(),
            [
                ("A".to_owned(), BuildTimeValue::Int(8)),
                ("B".to_owned(), BuildTimeValue::Int(10)),
            ],
            "{files:?}"
        );
    }
}

/// `Pair` declares `zeta` before `alpha`; `make` writes them the other way
/// round. The frozen value keeps declaration order, so a reader that walks
/// the fields by position meets `zeta` first. `SHAPE` is the second case of
/// `Shape`, and the frozen value names that case.
#[test]
fn a_frozen_record_keeps_declaration_order_and_its_case() {
    let program = "data Pair [copy] { zeta: u8; alpha: u8; } \
        machine make() -> Pair crashes none { Pair { alpha: 2u8, zeta: 1u8 } } \
        const PAIR: Pair = make(); \
        data Shape [copy] { case Round(radius: u8); case Square(side: u8); } \
        machine shape() -> Shape crashes none { Shape::Square { side: 4u8 } } \
        const SHAPE: Shape = shape();";
    let prepared = prepare(&[("main.omg", program)], Demand::Package).unwrap();
    use application_identity::const_value::DecodedCanonicalConstValue as Value;
    let Some(Value::Record { type_name, fields }) = prepared.value("PAIR") else {
        panic!("PAIR is not a record: {:?}", prepared.value("PAIR"))
    };
    assert_eq!(type_name, "Pair");
    let fields: Vec<_> = fields
        .into_iter()
        .map(|(name, value)| (name, integer(Some(value))))
        .collect();
    assert_eq!(
        fields,
        [("zeta".to_owned(), Some(1)), ("alpha".to_owned(), Some(2))]
    );
    let Some(Value::Variant {
        type_name,
        case_name,
        fields,
    }) = prepared.value("SHAPE")
    else {
        panic!("SHAPE is not a case: {:?}", prepared.value("SHAPE"))
    };
    assert_eq!(
        (type_name.as_str(), case_name.as_str()),
        ("Shape", "Square")
    );
    let fields: Vec<_> = fields
        .into_iter()
        .map(|(name, value)| (name, integer(Some(value))))
        .collect();
    assert_eq!(fields, [("side".to_owned(), Some(4))]);
}

fn constant(program: &TypedTrees, path: &str) -> SymbolHandle {
    program
        .const_declarations()
        .iter()
        .map(|declaration| declaration.symbol)
        .find(|&symbol| program.symbols.display_path(symbol, "\\") == path)
        .unwrap_or_else(|| panic!("no constant `{path}`"))
}

fn input(program: &TypedTrees, value: &str, constants: &[&str]) -> ValueInputs {
    ValueInputs::new(
        program,
        PreparedValue::Constant(constant(program, value)),
        constants
            .iter()
            .map(|path| constant(program, path))
            .collect(),
        TargetInputs::None,
    )
}

fn handed_constants(program: TypedTrees, rows: &[ValueInputs]) -> Vec<String> {
    let prepared = prepare_typed_trees(program, rows, &[]).unwrap();
    let trees = prepared.trees();
    let mut paths: Vec<_> = prepared
        .constants()
        .map(|declaration| trees.symbols.display_path(declaration.symbol, "\\"))
        .collect();
    paths.sort();
    paths
}

/// Stage 04 hands lowering the constants the source driver asked for and
/// every constant they were computed from, and no other. Changing only the
/// recorded input from `FIRST` to `OTHER` must change what lowering gets.
#[test]
fn stage_04_hands_lowering_each_value_and_exactly_its_recorded_inputs() {
    let program = typed(&[("main.omg", &aliases("FIRST"))]);
    let first = [input(&program, "COPY", &["FIRST"])];
    let other = [input(&program, "COPY", &["OTHER"])];
    assert_eq!(handed_constants(program.clone(), &first), ["COPY", "FIRST"]);
    assert_eq!(handed_constants(program.clone(), &other), ["COPY", "OTHER"]);
    assert_eq!(handed_constants(program, &[]), Vec::<String>::new());
}

/// Each row carries the identity of the program it was computed for. Stage
/// 04 refuses a row made for another program, even one with the same text,
/// and a row that names a declaration which is not a constant.
#[test]
fn stage_04_refuses_inputs_it_cannot_match_to_this_program() {
    let text = aliases("FIRST");
    let program = typed(&[("main.omg", &text)]);
    let twin = typed(&[("main.omg", &text)]);
    let refusal = |rows: &[ValueInputs]| -> Vec<String> {
        prepare_typed_trees(program.clone(), rows, &[])
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    };
    assert_eq!(
        handed_constants(program.clone(), &[input(&program, "COPY", &["FIRST"])]),
        ["COPY", "FIRST"]
    );
    assert_eq!(
        refusal(&[input(&twin, "COPY", &["FIRST"])]),
        ["prepared value inputs belong to another source program"]
    );

    let width = typed(&[("main.omg", &format!("{WIDTH} {text}"))]);
    let machine = width
        .realized_machine_named("width")
        .expect("the program declares `width`")
        .symbol;
    let not_a_constant = ValueInputs::new(
        &width,
        PreparedValue::Constant(constant(&width, "COPY")),
        vec![machine],
        TargetInputs::None,
    );
    assert_eq!(
        prepare_typed_trees(width.clone(), &[not_a_constant], &[])
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>(),
        ["source preparation lost a demanded constant declaration"]
    );
}

/// `a.omg` and `b.omg` each declare `three`, `four` and `SIZE`, with
/// different bodies, and neither imports the other. `a.omg`'s `SIZE` calls
/// its own file's machine, so it freezes to 3 or 4 by the name written,
/// never to the 5 or 6 that `b.omg`'s machines return, and `b.omg`'s `SIZE`
/// keeps its own 8. The file each frozen value belongs to is the one its
/// initializer was written in.
#[test]
fn a_call_freezes_the_value_of_the_machine_in_its_own_file() {
    let other = (
        "b.omg",
        "machine three() -> u64 crashes none { 5u64 } \
         machine four() -> u64 crashes none { 6u64 } \
         const SIZE: u64 = 8u64;",
    );
    for (selected, expected) in [("three", 3), ("four", 4)] {
        let own = format!(
            "machine three() -> u64 crashes none {{ 3u64 }} \
             machine four() -> u64 crashes none {{ 4u64 }} \
             const SIZE: u64 = {selected}();"
        );
        for files in [
            [("a.omg", own.as_str()), other],
            [other, ("a.omg", own.as_str())],
        ] {
            let prepared = prepare(&files, Demand::Package).unwrap();
            let mut frozen: Vec<_> = prepared
                .program
                .trees()
                .const_declarations()
                .iter()
                .filter(|declaration| prepared.path(declaration.symbol) == "SIZE")
                .map(|declaration| {
                    (
                        prepared
                            .file_at(declaration.initializer_source_span)
                            .to_owned(),
                        integer(package::declared_value(declaration)),
                    )
                })
                .collect();
            frozen.sort();
            assert_eq!(
                frozen,
                [
                    ("a.omg".to_owned(), Some(expected)),
                    ("b.omg".to_owned(), Some(8)),
                ],
                "SIZE = {selected}() in {files:?}"
            );
        }
    }
}

#[test]
fn a_computed_extent_rebuilds_dependent_payloads_at_their_original_handles() {
    use typed_trees::{
        data::DataMember,
        data_applications::prepare,
        types::{FixedArrayLength, TypeReferenceNode},
    };

    use typed_trees_to_prepared_typed_trees::constant_arguments::settle_const_argument;

    let text = r#"
data Buffer<T, const N: u64> { slots: [T; N]; }
data Envelope<T> { case Value(payload: T); }
data Holder { value: Envelope<Buffer<u8, capacity()>>; }
machine capacity() -> u64 { 4 }
"#;
    let mut program = typed(&[("main.omg", text)]);
    let holder = program
        .data_definitions()
        .iter()
        .find(|row| row.name.as_str() == "Holder")
        .unwrap();
    let DataMember::Field(field) = &program.data_members(holder)[0] else {
        panic!("Holder.value")
    };
    let envelope = field.type_reference;
    let arguments = |program: &TypedTrees, reference| {
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            panic!("generic source type");
        };
        *arguments
    };
    let buffer = program
        .type_reference_table
        .type_reference_handles(arguments(&program, envelope))[0];
    let parameters = program
        .type_reference_table
        .type_reference_handles(arguments(&program, buffer));
    let (byte, count) = (parameters[0], parameters[1]);
    let extent = |program: &TypedTrees, buffer| {
        let view = program.data_application_definition(buffer).unwrap();
        let DataMember::Field(slots) = &program.data_members(view)[0] else {
            panic!("Buffer.slots")
        };
        slots.type_reference
    };
    let integer = |value| {
        typed_trees::expression::ExpressionNode::Integer(
            numeric_literals::IntegerLiteral::from_value(value),
        )
    };
    let original_extent = extent(&program, buffer);
    assert!(
        matches!(program.type_reference_table.type_reference(original_extent),
        TypeReferenceNode::FixedArray { element_type, length: FixedArrayLength::Expression(arg) }
        if *element_type == byte && *arg == count)
    );
    assert!(!program.data_application_is_closed(envelope));
    assert_eq!(
        settle_const_argument(&mut program, count, integer(-1)),
        Err("an applied array length is not a closed nonnegative integer".into())
    );
    assert!(matches!(
        program.type_reference_table.type_reference(count),
        TypeReferenceNode::ConstExpression(_)
    ));
    assert_eq!(extent(&program, buffer), original_extent);

    settle_const_argument(&mut program, count, integer(4)).unwrap();
    assert_eq!(
        program
            .static_const_literal_from_type_reference(count)
            .and_then(|literal| literal.value_u64()),
        Some(4)
    );
    assert_eq!(
        program
            .closed_data_definition(buffer)
            .unwrap()
            .generic_instance,
        Some(buffer)
    );
    let view = program.closed_data_definition(envelope).unwrap();
    assert_eq!(view.generic_instance, Some(envelope));
    let DataMember::Variant(value) = &program.data_members(view)[0] else {
        panic!("Envelope.Value")
    };
    let nested = program.data_payload_fields(value)[0].type_reference;
    for reference in [
        original_extent,
        extent(&program, buffer),
        extent(&program, nested),
    ] {
        assert!(
            matches!(program.type_reference_table.type_reference(reference),
            TypeReferenceNode::FixedArray { element_type, length: FixedArrayLength::Literal(4) }
            if *element_type == byte)
        );
    }
    let sizes = |program: &TypedTrees| {
        (
            program.type_reference_table.type_reference_count(),
            program.data_members.len(),
            program.data_payload_fields.len(),
        )
    };
    let before = sizes(&program);
    program.discard_open_data_application(buffer);
    assert!(program.data_application_is_closed(buffer));
    assert_eq!(prepare(&mut program, buffer), Ok(true));
    assert_eq!(prepare(&mut program, envelope), Ok(true));
    assert_eq!(sizes(&program), before);
}
