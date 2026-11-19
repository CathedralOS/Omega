use num_bigint::BigInt;
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypeError, ValueType, resolve_source_package};

const WIDTH: &str = "machine width(n:u8)->u8 reaches none crashes none terminates {n +% 1}";
const BYTE: ValueType = ValueType::Integer { signed: false, width: Some(8) };

fn prepare(source: &str) -> Result<PreparedTrees<'_>, TypeError> {
    prepare_files([("main.omg", source)])
}

fn prepare_files<'text, const N: usize>(files: [(&'text str, &'text str); N]) -> Result<PreparedTrees<'text>, TypeError> {
    let parsed = files.map(|(path, source)| {
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
        (path, source, tokens, tree)
    });
    let sources = parsed.each_ref().map(|(path, text, tokens, tree)| StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: path,
            text,
            tokens,
            tree,
        },
    });
    let package = resolve_source_package(&sources).unwrap();
    prepare_source_package(&package)
}

fn assert_integer_constant(
    prepared: &PreparedTrees<'_>,
    name: &str,
    value: u8,
) {
    let constant = prepared
        .typed()
        .constants
        .iter()
        .find(|constant| constant.declaration.package == "app" && constant.declaration.path == name)
        .unwrap();
    assert_eq!(constant.initializer.carrier, BYTE);
    assert_eq!(prepared.integer_constant(&constant.declaration), Some(&BigInt::from(value)));
}

fn assert_buffer_index(
    prepared: &PreparedTrees<'_>,
    machine: &str,
    value: u8,
) {
    let typed = prepared.typed();
    let buffer = typed
        .resolved()
        .declarations
        .iter()
        .find(|id| id.package == "app" && id.path == "Buffer")
        .unwrap();
    let signature = typed
        .signatures
        .iter()
        .find(|signature| signature.machine.path == machine)
        .unwrap();
    assert_eq!(
        signature.parameters[0].1,
        ValueType::Applied(
            Box::new(buffer.clone()),
            vec![ValueType::StaticInteger {
                carrier: Box::new(BYTE),
                value: value.into()
            }],
        )
    );
}

#[test]
fn the_same_call_supplies_a_named_constant_and_a_closed_index() {
    let source = format!(
        "data Buffer<const N:u8>{{}}
         {WIDTH}
         const SIZE:u8=width(7);
         machine direct(x:Buffer<width(7)>){{}}
         machine named(x:Buffer<SIZE>){{}}
         machine literal(x:Buffer<8>){{}}"
    );
    for (source, value) in [(source.clone(), 8), (source.replace("+% 1", "+% 2"), 9)] {
        let prepared = prepare(&source).unwrap();
        assert_integer_constant(&prepared, "SIZE", value);
        assert_buffer_index(&prepared, "direct", value);
        assert_buffer_index(&prepared, "named", value);
        assert_buffer_index(&prepared, "literal", 8);
    }
}

#[test]
fn matching_node_numbers_in_different_files_keep_their_own_operands() {
    let a = ("a.omg", "const A:u8=width(7);");
    let b = ("b.omg", "const B:u8=width(9);");
    let machines = ("machines.omg", WIDTH);
    for files in [[a, b, machines], [machines, b, a]] {
        let prepared = prepare_files(files).unwrap();
        let [first, second] = prepared.typed().constants.as_slice() else {
            panic!("expected the two source constants");
        };
        assert_eq!(first.initializer.node.node, second.initializer.node.node);
        assert_eq!(first.initializer.expression.span, second.initializer.expression.span);
        assert_ne!(first.initializer.node.file, second.initializer.node.file);
        assert_integer_constant(&prepared, "A", 8);
        assert_integer_constant(&prepared, "B", 10);
    }
}

#[test]
fn fixed_integer_arithmetic_obeys_each_selected_operation() {
    for (ty, body, input, expected) in [
        ("i8", "n +% 1", "127", -128),
        ("i8", "n -% 1", "-128", 127),
        ("u8", "n -% 1", "0", 255),
        ("i8", "n *% 3", "-100", -44),
        ("i8", "n *| 3", "-100", -128),
        ("u8", "n -| 1", "0", 0),
        ("i13", "n +| 1", "4095", 4095),
        ("i1", "n +% n", "-1", 0),
        ("i8", "(n +% 1) -| 1", "127", -128),
        ("i8", "(n - 2) * 3", "1", -3),
    ] {
        let source = format!(
            "data Indexed<const N:{ty}>{{}} machine compute(n:{ty})->{ty}{{{body}}}
             const RESULT:{ty}=compute({input});
             machine compare(x:Indexed<compute({input})>)->Indexed<{expected}>{{x}}"
        );
        let prepared = prepare(&source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        let constant = &prepared.typed().constants[0];
        assert_eq!(prepared.integer_constant(&constant.declaration), Some(&BigInt::from(expected)), "{source}");
    }
}

#[test]
fn signed_constants_and_call_operands_use_their_full_carrier_width() {
    let source = "machine keep(n:i128)->i128{n}
                  const TOP:i128=170141183460469231731687303715884105727;
                  const LOW:i128=keep(TOP +% 1);
                  const HIGH:i128=LOW -% 1;";
    let prepared = prepare(source).unwrap();
    let half = BigInt::from(1u8) << 127u8;
    for constant in &prepared.typed().constants {
        let expected = match constant.declaration.path.as_str() {
            "TOP" | "HIGH" => &half - 1u8,
            "LOW" => -&half,
            other => panic!("unexpected constant {other}"),
        };
        assert_eq!(prepared.integer_constant(&constant.declaration), Some(&expected));
    }
}

#[test]
fn exact_intermediate_overflow_refuses_the_call_before_its_parent_operation() {
    for (body, input, operation) in [
        ("(n + 1) - 1", "127", "addition"),
        ("(n - 1) + 1", "-128", "subtraction"),
        ("(n * 2) - 1", "64", "multiplication"),
    ] {
        let call = format!("compute({input})");
        let source = format!("machine compute(n:i8)->i8{{{body}}} const RESULT:i8={call};");
        let error = prepare(&source).unwrap_err();
        assert_eq!(
            error.message,
            format!("static evaluation cannot admit this invocation: exact {operation} exceeds its selected integer carrier")
        );
        assert_eq!(error.span.slice(source.as_bytes()), Some(call.as_bytes()));
    }
}

#[test]
fn stand_alone_constants_preserve_the_values_l3_landed() {
    let prepared = prepare("const SUFFIX=7u8; const PRODUCT:u8=6*7;").unwrap();
    assert_integer_constant(&prepared, "SUFFIX", 7);
    assert_integer_constant(&prepared, "PRODUCT", 42);
}

#[test]
fn named_dependencies_close_before_their_aliases_and_callers() {
    let source = format!(
        "data Buffer<const N:u8>{{}}
         {WIDTH}
         const NEXT:u8=width(SIZE);
         const ALIAS:u8=SIZE;
         const SIZE:u8=width(7);
         machine alias(x:Buffer<ALIAS>){{}}
         machine next(x:Buffer<NEXT>){{}}"
    );
    let prepared = prepare(&source).unwrap();
    assert_integer_constant(&prepared, "SIZE", 8);
    assert_integer_constant(&prepared, "ALIAS", 8);
    assert_integer_constant(&prepared, "NEXT", 9);
    assert_buffer_index(&prepared, "alias", 8);
    assert_buffer_index(&prepared, "next", 9);
}

#[test]
fn constants_and_indices_refuse_the_same_call_at_its_occurrence() {
    for (machine, argument, diagnostic) in [
        (
            "machine width(n:u8)->u8 requires n < 8 reaches none crashes none terminates {n +% 1}",
            "8",
            "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments",
        ),
        (
            "boundary trait Sensor {} machine width(n:u8)->u8 reaches Sensor crashes none terminates {n +% 1}",
            "7",
            "fact or static argument requires an effect-free computation",
        ),
    ] {
        let call = format!("width({argument})");
        for usage in [
            format!("const SIZE:u8={call};"),
            format!("data Buffer<const N:u8>{{}} machine direct(x:Buffer<{call}>){{}}"),
        ] {
            let source = format!("{machine} {usage}");
            let error = prepare(&source).unwrap_err();
            assert_eq!(error.message, diagnostic);
            assert_eq!(error.package, "app");
            assert_eq!(error.file, "main.omg");
            assert_eq!(error.span.slice(source.as_bytes()), Some(call.as_bytes()));
        }
    }
}

#[test]
fn guarded_division_supplies_the_same_constant_and_generic_index() {
    let source = "data Buffer<const N:u8>{}
                  machine divide(n:u8,d:u8)->u8 requires d != ZERO reaches none crashes none terminates {n / d}
                  const SIZE:u8=divide(12,3);
                  const ZERO:u8=0;
                  machine computed(x:Buffer<divide(12,3)>){}
                  machine named(x:Buffer<SIZE>){}";
    let prepared = prepare(source).unwrap();
    assert_integer_constant(&prepared, "SIZE", 4);
    assert_buffer_index(&prepared, "computed", 4);
    assert_buffer_index(&prepared, "named", 4);
    let source = source.replace("12,3", "12,0");
    let error = prepare(&source).unwrap_err();
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments"
    );
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"divide(12,0)".as_slice()));
}

#[test]
fn concrete_preconditions_compare_mathematical_values_and_check_every_clause() {
    for (op, pass, fail) in [
        ("<", -1, 1),
        ("<=", 1, 2),
        (">", 2, 1),
        (">=", 1, -1),
        ("==", 1, 2),
        ("!=", -1, 1),
    ] {
        let machine = format!("machine choose(a:i8,b:u16)->u8 requires a {op} b requires b < 2 {{3}}");
        for (left, right, accepted) in [(pass, 1, true), (fail, 1, false)] {
            let source = format!("{machine} const VALUE:u8=choose({left},{right});");
            if accepted {
                assert_integer_constant(&prepare(&source).unwrap(), "VALUE", 3);
            } else {
                assert!(
                    prepare(&source)
                        .unwrap_err()
                        .message
                        .contains("precondition is false"),
                    "{source}"
                );
            }
        }
    }
    for (guard, reason) in [
        ("requires a < b requires b < 2", "precondition is false"),
        ("requires (a < b) && (b > 0)", "needs proof outside"),
    ] {
        let source = format!("machine choose(a:i8,b:u16)->u8 {guard} {{3}} const VALUE:u8=choose(-1,2);");
        let error = prepare(&source).unwrap_err();
        assert!(error.message.contains(reason), "{error:?}");
    }
}

#[test]
fn signed_quotient_and_remainder_keep_the_selected_overflow_policy() {
    for (op, n, d, expected) in [
        ("/", -7, 2, -3),
        ("%", -7, 2, -1),
        ("/", 7, -2, -3),
        ("%", 7, -2, 1),
        ("/%", -128, -1, -128),
        ("/|", -128, -1, 127),
        ("%%", -128, -1, 0),
        ("%|", -128, -1, 0),
    ] {
        let source = format!("machine calc(n:i8,d:i8)->i8{{n {op} d}} const VALUE:i8=calc({n},{d});");
        let prepared = prepare(&source).unwrap();
        assert_eq!(
            prepared.integer_constant(&prepared.typed().constants[0].declaration),
            Some(&BigInt::from(expected)),
            "{source}"
        );
    }
    for op in ["/", "%", "/%", "%%", "/|", "%|"] {
        let source = format!("machine calc(n:i8,d:i8)->i8{{n {op} d}} const VALUE:i8=calc(-128,0);");
        assert!(
            prepare(&source)
                .unwrap_err()
                .message
                .contains("nonzero divisor"),
            "{source}"
        );
        if matches!(op, "/" | "%") {
            let source = source.replace("-128,0", "-128,-1");
            assert!(prepare(&source).unwrap_err().message.contains("exact"), "{source}");
        }
    }
}

#[test]
fn annotated_integer_constants_cannot_supply_each_other_through_a_cycle() {
    let source = "const FIRST:u8=SECOND; const SECOND:u8=FIRST;";
    let error = prepare(source).unwrap_err();
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: cyclic integer constant initializer"
    );
    assert_eq!(error.package, "app");
    assert_eq!(error.file, "main.omg");
    let cycle_uses = [
        source.find("=SECOND").unwrap() + 1,
        source.find("=FIRST").unwrap() + 1,
    ];
    assert!(cycle_uses.contains(&error.span.get_start_offset()));
    assert!(matches!(error.span.slice(source.as_bytes()), Some(b"FIRST" | b"SECOND")));
}
