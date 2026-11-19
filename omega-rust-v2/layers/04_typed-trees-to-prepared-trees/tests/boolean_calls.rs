use omega_preparation::{PreparedValue, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;

fn prepare(source: &str) -> Result<omega_preparation::PreparedTrees<'_>, omega_typer::TypeError> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    prepare_source_package(&resolve_source_package(&sources).unwrap())
}

#[test]
fn selected_boolean_calls_produce_the_same_constants_as_literals() {
    for (input, expected) in [("true", true), ("false", false)] {
        let source = format!(
            "machine literal()->bool{{{input}}}
             machine keep(value:bool)->bool{{value}}
             machine wrapped(value:bool)->bool{{keep(value)}}
             machine from_body(value:bool)->bool{{keep(value) && keep(value)}}
             const NAMED:bool=wrapped(INPUT);
             const INPUT:bool={input};
             const LITERAL:bool=literal();
             const NESTED:bool=keep(keep({input}));
             const BODY:bool=from_body(INPUT);"
        );
        let prepared = prepare(&source).unwrap();
        assert_eq!(prepared.typed().constants.len(), 5);
        for constant in &prepared.typed().constants {
            assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(expected)));
        }
    }
}

#[test]
fn a_selected_predicate_checks_the_bound_value_before_running_the_callee() {
    let machines = "machine predicate(value:bool)->bool reaches none crashes none terminates {value}
                    machine guarded(value:bool)->bool requires predicate(value) {value}
                    machine wrapped(value:bool)->bool {guarded(value)}";
    let source = format!("{machines} const VALUE:bool=wrapped(true);");
    let prepared = prepare(&source).unwrap();
    assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Bool(true)));

    for (initializer, call, chain) in [
        ("guarded(false)", "guarded(false)", ""),
        ("true && guarded(false)", "guarded(false)", ""),
        ("wrapped(false)", "guarded(value)", "\ncall chain: wrapped -> guarded"),
    ] {
        let source = format!("{machines} const VALUE:bool={initializer};");
        let error = prepare(&source).unwrap_err();
        assert_eq!(error.package, "app");
        assert_eq!(error.file, "main.omg");
        assert_eq!(error.span.slice(source.as_bytes()), Some(call.as_bytes()));
        assert_eq!(
            error.message,
            format!("static evaluation cannot admit this invocation: the selected Boolean precondition is false for the supplied arguments{chain}")
        );
    }
}

#[test]
fn a_boolean_call_reports_the_typed_edge_that_closes_the_recursive_component() {
    let source = "machine first()->bool {second() && true}
                  machine second()->bool {first()}
                  const VALUE:bool=first();";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"first()".as_slice()));
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: a cyclic static invocation requires a checked termination proof\nrecursive component: first -> second -> first"
    );
    assert_eq!(error.span.get_start_offset(), source.find("{first()}").unwrap() + 1);
}

#[test]
fn long_acyclic_boolean_calls_suspend_on_the_heap() {
    let mut source = String::new();
    for index in 0..96 {
        source.push_str(&format!("machine f{index}(value:bool)->bool {{f{}(value) && true}}\n", index + 1,));
    }
    source.push_str("machine f96(value:bool)->bool {value} const VALUE:bool=f0(true) && f0(true);");
    let prepared = prepare(&source).unwrap();
    assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Bool(true)));
}

#[test]
fn a_cycle_in_a_selected_precondition_refuses_before_entering_the_body() {
    let source = "machine first()->bool requires second() reaches none crashes none terminates {true}
                  machine second()->bool requires first() reaches none crashes none terminates {false}
                  const VALUE:bool=first();";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.span.get_start_offset(), source.find("requires first()").unwrap() + "requires ".len());
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: a cyclic static invocation requires a checked termination proof\nrecursive component: first -> second -> first"
    );
}

#[test]
fn a_short_circuit_does_not_execute_the_unneeded_boolean_call() {
    let machines = "machine guarded(value:bool)->bool requires value {value}";
    for (expression, expected) in [
        ("false && guarded(false)", false),
        ("true || guarded(false)", true),
    ] {
        let source = format!("{machines} const VALUE:bool={expression};");
        let prepared = prepare(&source).unwrap();
        assert_eq!(
            prepared.constant(&prepared.typed().constants[0].declaration),
            Some(&PreparedValue::Bool(expected))
        );
    }
}

#[test]
fn boolean_static_computation_specializes_at_each_exact_owner_argument() {
    let source = "data Flag<const B:bool>{}
                  machine negate(b:bool)->bool{!b}
                  machine flip<const B:bool>()->Flag<negate(B)>{Flag::<negate(B)>{}}
                  machine from_false()->Flag<true>{flip::<false>()}
                  machine from_true()->Flag<false>{flip::<true>()}";
    let prepared = prepare(source).unwrap();
    let typed = prepared.typed();
    let signature_result = |path| {
        &typed
            .signatures
            .iter()
            .find(|signature| signature.machine.path == path)
            .unwrap()
            .result
    };
    for (argument, caller) in [(false, "from_false"), (true, "from_true")] {
        let call = typed
            .calls
            .iter()
            .find(|call| {
                call.machine.path == "flip"
                    && matches!(
                        call.type_arguments.as_slice(),
                        [omega_typer::ValueType::StaticValue { value: PreparedValue::Bool(value), .. }] if *value == argument
                    )
            })
            .expect("selected call retained the closed Boolean binder");
        assert_eq!(&call.result, signature_result(caller));
    }
    let changed = source.replace("{!b}", "{b}");
    let error = prepare(&changed).unwrap_err();
    assert!(error.message.contains("Flag<true>") && error.message.contains("Flag<false>"), "{error:?}");
}

#[test]
fn selected_scalar_blocks_bind_local_values_and_take_only_the_matching_arm() {
    let machines = "machine identity(value:bool)->bool {value}
                    machine divide(value:u8)->u8 requires value != 0 {12u8 / value}
                    machine compute(value:bool)->u8 {
                        let selected:bool=identity(value);
                        let local:u8=3;
                        let number:u8=match selected {
                            true -> divide(local),
                            false -> divide(0),
                        };
                        number +% 1
                    }
                    machine terminal(value:bool)->bool {
                        let saved:bool=value;
                        transition saved {true -> true, false -> false,}
                    }";
    let source = format!("{machines} const VALUE:u8=compute(true); const TERMINAL:bool=terminal(false);");
    let prepared = prepare(&source).unwrap();
    for constant in &prepared.typed().constants {
        let value = match constant.declaration.path.as_str() {
            "VALUE" => PreparedValue::Integer(5u8.into()),
            "TERMINAL" => PreparedValue::Bool(false),
            other => panic!("unexpected constant {other}"),
        };
        assert_eq!(prepared.constant(&constant.declaration), Some(&value));
    }

    let source = source.replace("compute(true)", "compute(false)");
    let error = prepare(&source).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"divide(0)".as_slice()));
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments\ncall chain: compute -> divide"
    );
}

#[test]
fn compound_integer_calls_are_equivalent_to_their_literal_result() {
    let machines = "data Width<const N:u8>{}
                    machine one(x:u8)->u8 requires x != 0 {x / x}
                    machine combine(x:u8)->u8 {one(x) +% one(one(x))}
                    machine witness(x:Width<combine(3)>)->Width<2>{x}";
    let source = format!("{machines} const VALUE:u8=combine(3); const NESTED:u8=one(one(3));");
    let prepared = prepare(&source).unwrap();
    assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&2u8.into()));
    assert_eq!(prepared.integer_constant(&prepared.typed().constants[1].declaration), Some(&1u8.into()));

    let source = format!("{machines} const VALUE:u8=combine(0);");
    let error = prepare(&source).unwrap_err();
    assert_eq!(error.span.get_start_offset(), source.find("one(x) +%").unwrap());
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments\ncall chain: combine -> one"
    );
}

#[test]
fn a_selected_scalar_block_cannot_ignore_an_unsupported_statement() {
    let source = "machine run(value:bool)->bool {let mut saved:bool=value; saved=false; saved} const VALUE:bool=run(true);";
    let error = prepare(source).unwrap_err();
    assert!(
        error
            .message
            .starts_with("static evaluation cannot admit this invocation:"),
        "{error:?}"
    );
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"run(true)".as_slice()));
}

#[test]
fn nested_fields_specialize_their_own_and_the_enclosing_boolean_binders() {
    let source = "data Flag<const B:bool>{}
                  machine negate(value:bool)->bool{!value}
                  data Outer<const A:bool>{data Inner<const B:bool>{
                    outer:Flag<negate(A)>;
                    inner:Flag<negate(B)>;
                  }}
                  machine outer(x:Outer<true>::Inner<false>)->Flag<false>{x.outer}
                  machine inner(x:Outer<true>::Inner<false>)->Flag<true>{x.inner}";
    prepare(source).unwrap();
    for (actuals, field, expected) in [
        ("Outer<false>::Inner<false>", "outer", "true"),
        ("Outer<true>::Inner<true>", "inner", "false"),
    ] {
        let source = source.replace("Outer<true>::Inner<false>", actuals);
        let error = prepare(&source).unwrap_err();
        assert!(error.message.contains(&format!("found app::Flag<{expected}>")), "{field}: {error:?}");
    }
}

#[test]
fn nested_integer_arguments_keep_the_enclosing_specialization() {
    let source = "machine inc(n:u64)->u64{n +% 1}
                  machine same(n:u64)->u64{n}
                  data Outer<const A:u64>{data Inner<const B:u64>{
                    outer:[u8;same(inc(A))];
                    inner:[u8;same(inc(B))];
                  }}
                  machine outer(x:Outer<2>::Inner<4>)->[u8;3]{x.outer}
                  machine inner(x:Outer<2>::Inner<4>)->[u8;5]{x.inner}";
    prepare(source).unwrap();
    for (actuals, expected) in [("Outer<3>::Inner<4>", 4), ("Outer<2>::Inner<5>", 6)] {
        let error = prepare(&source.replace("Outer<2>::Inner<4>", actuals)).unwrap_err();
        assert!(error.message.contains(&format!("found [u8; {expected}]")), "{error:?}");
    }
}
