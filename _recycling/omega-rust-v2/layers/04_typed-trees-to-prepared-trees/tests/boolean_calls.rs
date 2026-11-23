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
fn local_declarations_follow_both_terminal_body_forms() {
    let source = "machine tail(value:bool)->bool {step(value) machine step(input:bool)->bool {input}}
                  machine dispatch(value:bool)->bool {
                      transition value {true -> step(true), false -> step(false),}
                      machine step(input:bool)->bool {input}
                  }
                  const TAIL:bool=tail(true);
                  const DISPATCH:bool=dispatch(false);";
    let prepared = prepare(source).unwrap();
    for (name, result) in [("TAIL", true), ("DISPATCH", false)] {
        let constant = prepared
            .typed()
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(result)));
    }
}

#[test]
fn a_nested_helper_uses_the_enclosing_application_each_time() {
    let source = "machine always(value:bool)->bool {true}
                  machine never(value:bool)->bool {false}
                  data BoolStep=machine(value:bool)->bool;
                  machine apply<const F:BoolStep>(value:bool)->bool {
                      step(value)
                      machine step(input:bool)->bool {
                          invoke(input)
                          machine invoke(argument:bool)->bool {F(argument)}
                      }
                  }
                  const FIRST:bool=apply::<always>(false);
                  const SECOND:bool=apply::<never>(true);";
    let prepared = prepare(source).unwrap();
    let typed = prepared.typed();
    let indirect = &typed.indirect_calls[0];
    let omega_typed::InvocationTarget::StaticParameter { owner, index } = &indirect.entry else {
        panic!("expected the enclosing machine's const callback");
    };
    let helper = typed
        .calls
        .iter()
        .find(|call| Some(&call.machine) == indirect.caller.as_ref())
        .unwrap();
    assert_eq!(
        helper.generic_argument(owner, *index),
        Some(&omega_typed::ValueType::Parameter {
            owner: Box::new(owner.clone()),
            index: *index
        }),
    );
    for (name, result) in [("FIRST", true), ("SECOND", false)] {
        let constant = typed
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(result)));
    }
}

#[test]
fn nested_helpers_read_their_exact_static_arguments() {
    let source = "machine outer<const FLAG:bool>(input:bool)->bool {
                      helper(input)
                      machine helper(value:bool)->bool {FLAG}
                  }
                  machine pair<const OUTER:bool>()->bool {
                      inner::<false>()
                      machine inner<const INNER:bool>()->bool {
                          helper::<true>()
                          machine helper<const OWN:bool>()->bool {OUTER && !INNER && OWN}
                      }
                  }
                  const FIRST:bool=outer::<true>(false);
                  const SECOND:bool=outer::<false>(true);
                  const THREE_SCOPES_TRUE:bool=pair::<true>();
                  const THREE_SCOPES_FALSE:bool=pair::<false>();";
    let prepared = prepare(source).unwrap();
    let typed = prepared.typed();
    for (name, result) in [
        ("FIRST", true),
        ("SECOND", false),
        ("THREE_SCOPES_TRUE", true),
        ("THREE_SCOPES_FALSE", false),
    ] {
        let constant = typed
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(result)));
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
                        [omega_typer::ValueType::StaticValue { value: omega_typer::StaticValue::Bool(value), .. }] if *value == argument
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
fn block_arms_keep_their_locals_and_return_the_selected_result() {
    let source = "machine port(local:bool)->u8 {
            let first:u8=match local {
                true -> {let port:u8=1; port},
                false -> {let port:u8=2; port},
            };
            let mutable extra:u8=0;
            match local {true -> {extra=3;}, false -> {}};
            transition local {
                true -> {let port:u8=4; first +% port +% extra},
                false -> {let port:u8=5; first +% port +% extra},
            }
        }
        machine copy_port(port:u8)->u8 {port}
        machine allowed(value:bool)->bool reaches none crashes none terminates {value}
        machine guarded(value:bool)->u8 requires match value {
            true -> {let ready:bool=true; allowed(ready)}, false -> false,
        } {6}
        const FACT:u8=guarded(true);
        const INITIAL:u8=match true {true -> {let port:u8=9; copy_port(port)}, false -> 0};
        const LOCAL:u8=port(true);
        const REMOTE:u8=port(false);";
    let prepared = prepare(source).unwrap();
    assert_eq!(prepared.typed().constants.len(), 4);
    for constant in &prepared.typed().constants {
        let expected = match constant.declaration.path.as_str() {
            "FACT" => 6u8,
            "INITIAL" => 9u8,
            "LOCAL" => 8u8,
            "REMOTE" => 7u8,
            other => panic!("unexpected constant {other}"),
        };
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Integer(expected.into())));
    }
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
fn discarding_a_local_preserves_its_bool_binding() {
    let source = "machine run(value:bool)->bool {let saved:bool=value; _ = saved; saved} const VALUE:bool=run(true);";
    let prepared = prepare(source).unwrap();
    assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Bool(true)));
}

#[test]
fn an_explicit_discard_evaluates_and_drops_a_copy_value() {
    let source = "data Pair [copy] {first:u8; flag:bool;}
                  const SEED:u8=9u8;
                  machine leaf(x:u8)->u8{x}
                  machine make(x:u8)->Pair{Pair{first:x,flag:true}}
                  machine caller(x:u8)->u8{_ = leaf(x); x}
                  machine aggregate(x:u8)->u8{_ = make(x); _ = SEED; x}
                  machine run(value:bool)->bool {let saved:bool=value; _ = saved; _ = !saved; saved}
                  const V:u8=caller(3u8);
                  const RECORD:u8=aggregate(5u8);
                  const FLAG:bool=run(true);";
    let prepared = prepare(source).unwrap();
    let typed = prepared.typed();
    let find = |name| {
        &typed
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap()
            .declaration
    };
    assert_eq!(prepared.constant(find("V")), Some(&PreparedValue::Integer(3u8.into())));
    assert_eq!(prepared.constant(find("RECORD")), Some(&PreparedValue::Integer(5u8.into())));
    assert_eq!(prepared.constant(find("FLAG")), Some(&PreparedValue::Bool(true)));
    // The discarded read is still an input of the constant that performed it.
    let inputs = prepared
        .dependencies()
        .inputs(&omega_preparation::PreparedValueOrigin::Constant(find("RECORD").clone()))
        .unwrap();
    spec_corpus::observe_handoff!("discarded-constant-reads/producer", assert_eq!(inputs.constants.as_slice(), std::slice::from_ref(find("SEED"))));
}

#[test]
fn a_discarded_operand_still_refuses_its_failed_precondition() {
    let machines = "machine divide(value:u8)->u8 requires value != 0 {12u8 / value}
                    machine caller(x:u8)->u8{_ = divide(x); x}";
    let source = format!("{machines} const V:u8=caller(3u8);");
    let prepared = prepare(&source).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(3u8.into()))
    );

    let source = format!("{machines} const V:u8=caller(0u8);");
    let error = prepare(&source).unwrap_err();
    spec_corpus::observe_handoff!("discarded-constant-reads/refusal", assert_eq!(error.span.slice(source.as_bytes()), Some(b"divide(x)".as_slice())));
    assert_eq!(
        error.message,
        "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments\ncall chain: caller -> divide"
    );
}

#[test]
fn an_explicit_discard_does_not_dispose_of_a_linear_value() {
    let source = "data Handle [linear] {}
                  machine make()->Handle{Handle{}}
                  machine run(x:u8)->u8{_ = make(); x}
                  const V:u8=run(1u8);";
    let error = prepare(source).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"make()".as_slice()));
    assert!(
        error
            .message
            .contains("the complete constant type must permit copying"),
        "{error:?}"
    );
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
        spec_corpus::observe_handoff!("owner-arguments-and-nested-constant-bindings/refusal", assert!(error.message.contains(&format!("found app\\Flag<{expected}>")), "{field}: {error:?}"));
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

#[test]
fn writes_only_the_selected_local_in_each_fresh_invocation() {
    let machines = "machine invert(value:bool)->bool {!value}
                    machine run(value:bool)->bool {
                        let original:bool=value;
                        let mutable saved:bool=value;
                        saved=invert(saved);
                        saved != original && saved
                    }";
    let source = format!("{machines} const FALSE:bool=run(true); const TRUE:bool=run(false);");
    let prepared = prepare(&source).unwrap();
    for constant in &prepared.typed().constants {
        let expected = constant.declaration.path == "TRUE";
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(expected)));
    }

    let immutable = source.replace("let mutable saved", "let saved");
    let error = prepare(&immutable).unwrap_err();
    assert_eq!(error.message, "place requires writable authority");
    assert_eq!(error.span.slice(immutable.as_bytes()), Some(b"saved".as_slice()));
}

#[test]
fn constant_match_arms_bind_values_through_nested_matches_and_calls() {
    for (expression, expected) in [
        ("match true { flag -> flag, }", true),
        ("match false { flag -> keep(flag), }", false),
        ("match true { flag -> match false { inner -> flag, }, }", true),
        ("match true { flag -> match false { flag -> flag, }, }", false),
    ] {
        let source = format!("machine keep(value:bool)->bool {{value}} const OUT:bool={expression};");
        let prepared = prepare(&source).unwrap();
        assert_eq!(
            prepared.constant(&prepared.typed().constants[0].declaration),
            Some(&PreparedValue::Bool(expected)),
            "{expression}"
        );
    }
}
