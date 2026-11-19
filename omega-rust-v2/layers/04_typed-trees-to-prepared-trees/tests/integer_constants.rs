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
        ("requires (a < b) && (b < 2)", "precondition is false"),
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
    assert_eq!(error.message, "static evaluation cannot admit this invocation: cyclic constant initializer");
    assert_eq!(error.package, "app");
    assert_eq!(error.file, "main.omg");
    let cycle_uses = [
        source.find("=SECOND").unwrap() + 1,
        source.find("=FIRST").unwrap() + 1,
    ];
    assert!(cycle_uses.contains(&error.span.get_start_offset()));
    assert!(matches!(error.span.slice(source.as_bytes()), Some(b"FIRST" | b"SECOND")));
}

#[test]
fn tail_helpers_use_the_selected_overload_and_each_callers_bound_operands() {
    let helpers = "machine portion(d:u64,n:u64)->u64 requires d != 0 {n / d}
                   machine portion(d:u8,n:u64)->u64 {91u64}
                   machine width(total:u64, divisor:u64)->u64{portion(divisor,total + 3)}
                   machine via(n:u64,d:u64)->u64{width(n,d)}";
    let arrays = "const SIZE:u64=via(9,3);
                  machine carry(x:[u8; SIZE], direct:[u8; via(9,3)])->[u8; 4]{x}";
    for files in [
        [("helpers.omg", helpers), ("arrays.omg", arrays)],
        [("arrays.omg", arrays), ("helpers.omg", helpers)],
    ] {
        let prepared = prepare_files(files).unwrap();
        let typed = prepared.typed();
        let portion = typed
            .calls
            .iter()
            .find(|call| call.machine.path == "portion" && call.caller.as_ref().is_some_and(|c| c.path == "width"))
            .unwrap();
        let word = ValueType::Integer {
            signed: false,
            width: Some(64),
        };
        let signature = typed
            .signatures
            .iter()
            .find(|s| s.machine.path == "portion" && s.parameters[0].1 == word)
            .unwrap();
        assert_eq!(portion.machine, signature.machine);
        let operands: Vec<_> = portion
            .operands
            .iter()
            .map(|op| {
                assert_eq!(op.site.file, "helpers.omg");
                op.site.span.slice(helpers.as_bytes()).unwrap()
            })
            .collect();
        assert_eq!(operands, [b"divisor".as_slice(), b"total + 3"]);
        let size = &typed.constants[0];
        assert_eq!(prepared.integer_constant(&size.declaration), Some(&4.into()));
        let carry = prepared
            .typed()
            .signatures
            .iter()
            .find(|s| s.machine.path == "carry")
            .unwrap();
        let expected = ValueType::Array(Box::new(BYTE), 4);
        assert_eq!(carry.result, expected);
        assert!(carry.parameters.iter().all(|(_, ty)| ty == &expected));
    }
    let altered = helpers.replace("total + 3", "total + 9");
    let error = prepare_files([("arrays.omg", arrays), ("helpers.omg", &altered)]).unwrap_err();
    assert_eq!(error.message, "expected [u8; 4], found [u8; 6]");
    assert_eq!(error.file, "arrays.omg");
    assert_eq!(error.span.slice(arrays.as_bytes()), Some(b"x".as_slice()));
    for demand in [
        "const SIZE:u64=via(9,0);",
        "machine consume(x:[u8; via(9,0)]){}",
    ] {
        let error = prepare_files([("arrays.omg", demand), ("helpers.omg", helpers)]).unwrap_err();
        assert_eq!(
            error.message,
            "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments\ncall chain: via -> width -> portion"
        );
        assert_eq!(error.file, "helpers.omg");
        assert_eq!(error.span.get_start_offset(), helpers.find("portion(divisor,total + 3)").unwrap());
        assert_eq!(error.span.slice(helpers.as_bytes()), Some(b"portion(divisor,total + 3)".as_slice()));
    }
}

#[test]
fn every_tail_helper_must_establish_termination() {
    for (source, site, reason) in [
        (
            "public machine helper(n:u64)->u64 reaches none crashes none {n} machine width(n:u64)->u64{helper(n)} const SIZE:u64=width(4);",
            "helper(n)",
            "the selected public machine does not declare termination\ncall chain: width -> helper",
        ),
        (
            "machine a(n:u64)->u64{b(n)} machine b(n:u64)->u64{a(n)} const SIZE:u64=a(4);",
            "a(n)",
            "a cyclic static invocation requires a checked termination proof\ncall chain: a -> b -> a",
        ),
    ] {
        let error = prepare(source).unwrap_err();
        assert_eq!(error.message, format!("static evaluation cannot admit this invocation: {reason}"));
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()));
    }
}

#[test]
fn tail_chain_distinguishes_an_arithmetic_failure_from_a_failed_guard() {
    let source = "machine divide(n:u64,d:u64)->u64{n / d} machine inner(d:u64)->u64{(divide(12,d))}
                  machine width(d:u64)->u64{inner(d)} machine consume(x:[u8; width(3)])->[u8; 4]{x}";
    prepare(source).unwrap();
    let bad = source.replace("width(3)", "width(0)");
    let error = prepare(&bad).unwrap_err();
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: integer division or remainder requires a nonzero divisor\ncall chain: width -> inner -> divide"
    );
    assert_eq!(error.span.slice(bad.as_bytes()), Some(b"divide(12,d)".as_slice()));
}

#[test]
fn tail_cycle_detection_uses_declaration_identity_across_files() {
    let first = ("a.omg", "machine first(n:u8)->u8{second(n)}");
    let second = ("b.omg", "machine second(n:u8)->u8{third(n)}");
    let demand = ("d.omg", "const SIZE:u8=first(7);");
    for leaf in ["n +% 1", "first(n)"] {
        let body = format!("machine third(n:u8)->u8{{{leaf}}}");
        let third = ("c.omg", body.as_str());
        for files in [[first, second, third, demand], [demand, third, second, first]] {
            let result = prepare_files(files);
            if leaf == "first(n)" {
                let error = result.unwrap_err();
                assert_eq!(error.file, "c.omg");
                assert_eq!(error.span.slice(body.as_bytes()), Some(b"first(n)".as_slice()));
                assert_eq!(
                    error.message,
                    "static evaluation cannot admit this invocation: a cyclic static invocation requires a checked termination proof\ncall chain: first -> second -> third -> first"
                );
            } else {
                assert_integer_constant(&result.unwrap(), "SIZE", 8);
            }
        }
    }
}

#[test]
fn closed_crash_guards_use_the_selected_callees_entry_values() {
    let definitions = "machine divide(n:u64,d:u64)->u64 crashes Trap (d == 0) {n /! d}
                       machine width(d:u64)->u64{divide(12,d)}";
    for callee in ["width", "divide"] {
        let call = |divisor| {
            if callee == "width" {
                format!("width({divisor})")
            } else {
                format!("divide(12,{divisor})")
            }
        };
        for extent in [true, false] {
            let demand = |divisor| {
                if extent {
                    format!("machine carry(x:[u8; {}])->[u8;4]{{x}}", call(divisor))
                } else {
                    format!("const SIZE:u64={}; machine carry(x:[u8; SIZE])->[u8;4]{{x}}", call(divisor))
                }
            };
            let pass = demand(3);
            prepare_files([("guard.omg", definitions), ("use.omg", &pass)]).unwrap();
            let fail = demand(0);
            let error = prepare_files([("guard.omg", definitions), ("use.omg", &fail)]).unwrap_err();
            let expected = "static evaluation cannot admit this invocation: the selected Trap route remains possible for the supplied arguments";
            if callee == "width" {
                assert_eq!(error.message, format!("{expected}\ncall chain: width -> divide"));
                assert_eq!(error.file, "guard.omg");
                assert_eq!(error.span.slice(definitions.as_bytes()), Some(b"divide(12,d)".as_slice()));
            } else {
                assert_eq!(error.message, expected);
                assert_eq!(error.file, "use.omg");
                assert_eq!(error.span.slice(fail.as_bytes()), Some(b"divide(12,0)".as_slice()));
            }
        }
    }
}

#[test]
fn every_crash_alternative_and_the_operation_itself_must_be_safe() {
    let program = |routes, value| format!("machine f(n:i8)->i8 {routes} {{ 12i8 /! n }} const SIZE:i8=f({value});");
    prepare(&program("crashes Trap false crashes Abort (n == 9)", "3")).unwrap();
    for (routes, value, expected) in [
        (
            "crashes Trap false crashes Abort (n == 9)",
            "9",
            "the selected Abort route remains possible for the supplied arguments",
        ),
        (
            "crashes Trap false crashes Trap (n == 9)",
            "9",
            "the selected Trap route remains possible for the supplied arguments",
        ),
        ("crashes Abort", "3", "the selected Abort route remains possible for the supplied arguments"),
        ("crashes Trap true", "3", "the selected Trap route remains possible for the supplied arguments"),
        ("crashes Trap false", "0", "a zero divisor leaves the selected Trap route possible"),
    ] {
        let source = program(routes, value);
        let error = prepare(&source).unwrap_err();
        assert_eq!(error.message, format!("static evaluation cannot admit this invocation: {expected}"));
    }
    for op in ["/!", "%!"] {
        let source = format!("machine div(n:i8,d:i8)->i8 crashes Trap (d == 0) {{n {op} d}} const SIZE:i8=div(-128,-1);");
        assert_eq!(
            prepare(&source).unwrap_err().message,
            "static evaluation cannot admit this invocation: an unrepresentable quotient leaves the selected Trap route possible"
        );
    }
}

#[test]
fn frozen_arrays_preserve_element_order_and_typed_dependencies() {
    use omega_preparation::PreparedValue::{Array, Bool, Integer};
    let source = "const TABLE:[[u8;2];2]=[ROW,[TWICE,4]]; const ROW:[u8;2]=[1,2];
                  const TWICE:u8=add(1,2); machine add(x:u8,y:u8)->u8{x+y}
                  const SELECTED:bool=OK; const OK:bool=3u8 < 4u8; const FLAGS:[bool;2]=[false,SELECTED];
                  machine count(n:u64)->u64 requires SELECTED {n}
                  machine consume(x:[u8; count(4)])->[u8;4]{x}";
    let prepared = prepare(source).unwrap();
    let get = |path| {
        let id = &prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == path)
            .unwrap()
            .declaration;
        prepared.constant(id).unwrap()
    };
    let row = |a: u8, b: u8| Array(vec![Integer(a.into()), Integer(b.into())].into());
    assert_eq!(get("TABLE"), &Array(vec![row(1, 2), row(3, 4)].into()));
    assert_eq!(get("FLAGS"), &Array(vec![Bool(false), Bool(true)].into()));
    let refused = source.replace("3u8 < 4u8", "4u8 < 4u8");
    let error = prepare(&refused).unwrap_err();
    assert!(error.message.contains("precondition is false"));
    assert_eq!(error.span.slice(refused.as_bytes()), Some(b"count(4)".as_slice()));
}

#[test]
fn unused_constants_must_still_produce_complete_values() {
    for (source, reason) in [
        ("const A:[bool;1]=B; const B:[bool;1]=A;", "cyclic constant initializer"),
        ("const A:[u8;2]=[1,overflow(255)]; machine overflow(n:u8)->u8{n + 1}", "exact addition exceeds"),
        ("const A:[bool;0]=[];", ""),
        ("data Handle [linear] {} const A:[Handle;0]=[];", "array element type"),
    ] {
        let result = prepare(source);
        if reason.is_empty() {
            let result = result.unwrap();
            assert_eq!(
                result.constant(&result.typed().constants[0].declaration),
                Some(&omega_preparation::PreparedValue::Array(vec![].into()))
            );
        } else {
            assert!(result.unwrap_err().message.contains(reason), "{source}");
        }
    }
}

#[test]
fn boolean_guards_only_demand_the_selected_operands() {
    let source = "machine width(d:u64)->u64 requires d == 0 || (12u64 / d > 0) {4u64}
                  const SIZE:u64=width(0); machine take(x:[u8; SIZE])->[u8;4]{x}";
    prepare(source).unwrap();
    let and = source.replace("d == 0 ||", "d != 0 &&");
    assert!(
        prepare(&and)
            .unwrap_err()
            .message
            .contains("precondition is false")
    );
    let division = source.replace("d == 0 ||", "d != 0 ||");
    assert!(
        prepare(&division)
            .unwrap_err()
            .message
            .contains("nonzero divisor")
    );
    let equality = "const A:bool=true; const B:bool=false; const C:bool=(A != B) == true;";
    let result = prepare(equality).unwrap();
    let c = result
        .typed()
        .constants
        .iter()
        .find(|c| c.declaration.path == "C")
        .unwrap();
    assert_eq!(result.constant(&c.declaration), Some(&omega_preparation::PreparedValue::Bool(true)));
}

#[test]
fn integer_preparation_uses_the_declared_width_independently_of_host_words() {
    let top = (BigInt::from(1) << 257u16) - 1u8;
    let source = format!(
        "machine successor(x:u257)->u257 {{x +% 1}}
                          machine mul(x:i)->i {{x * x}}
                          const WRAP:u257=successor({top});
                          const EXACT:i=mul({top});
                          const SMALL:u18446744073709551615=1;"
    );
    let prepared = prepare(&source).unwrap();
    for (name, value) in [
        ("WRAP", BigInt::from(0)),
        ("EXACT", &top * &top),
        ("SMALL", BigInt::from(1)),
    ] {
        let definition = prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.integer_constant(&definition.declaration), Some(&value));
    }
    let source = "machine prev(x:u18446744073709551615)->u18446744073709551615{x -% 1}
                  const TOO_LARGE:u18446744073709551615=prev(0);";
    let error = prepare(source).unwrap_err();
    assert!(error.message.contains("integer allocation budget"), "{}", error.message);
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"prev(0)".as_slice()));
}

#[test]
fn concrete_shifts_use_the_count_carrier_and_selected_policy() {
    for (operator, count, expected) in [("<<", 1, 14u8), ("<<%", 15, 14), ("<<|", 6, 127)] {
        let source = format!("machine shift(n:u7,k:u64)->u7{{n {operator} k}} const VALUE:u7=shift(7,{count});");
        let prepared = prepare(&source).unwrap();
        assert_eq!(
            prepared.integer_constant(&prepared.typed().constants[0].declaration),
            Some(&BigInt::from(expected))
        );
    }
    let failure = "machine shift(n:u7,k:u64)->u7{n << k} const VALUE:u7=shift(7,7);";
    let error = prepare(failure).unwrap_err();
    assert!(error.message.contains("shift count outside carrier width"), "{}", error.message);
}

#[test]
fn trapping_arithmetic_checks_the_actual_result_even_with_a_false_crash_guard() {
    for (operator, safe, overflow) in [("+!", 126, 127), ("-!", -127, -128), ("*!", 7, 64)] {
        let operand = if operator == "*!" { 2 } else { 1 };
        let program = |input| format!("machine calc(n:i8)->i8 crashes Trap false {{n {operator} {operand}}} const RESULT:i8=calc({input});");
        prepare(&program(safe)).unwrap();
        let error = prepare(&program(overflow)).unwrap_err();
        assert!(
            error
                .message
                .contains("overflow leaves the selected Trap route possible"),
            "{}",
            error.message
        );
    }
}

#[test]
fn typed_negation_matches_exact_literals_and_primitive_indices() {
    let source = "data Row<const N:i8>{}
                  machine negate(x:i8)->i8{-x}
                  const EXACT:i8=-5;
                  const SUFFIXED:i8=-5i8;
                  const PARAMETER:i8=negate(5);
                  machine compare(x:Row<(-5i8)>)->Row<negate(5)>{x}";
    let prepared = prepare(source).unwrap();
    for constant in &prepared.typed().constants {
        assert_eq!(prepared.integer_constant(&constant.declaration), Some(&BigInt::from(-5)));
    }
    let signature = prepared
        .typed()
        .signatures
        .iter()
        .find(|sig| sig.machine.path == "compare")
        .unwrap();
    assert_eq!(signature.parameters[0].1, signature.result);
}

#[test]
fn typed_negation_refuses_the_signed_minimum_in_a_constant_or_call() {
    let source = "machine negate(x:i8)->i8{-x}
                  const MIN:i8=-128;";
    for initializer in ["-MIN", "negate(MIN)"] {
        let input = format!("{source} const VALUE:i8={initializer};");
        let error = prepare(&input).unwrap_err();
        assert!(
            error
                .message
                .contains("exact subtraction exceeds its selected integer carrier"),
            "{}",
            error.message
        );
    }
}
