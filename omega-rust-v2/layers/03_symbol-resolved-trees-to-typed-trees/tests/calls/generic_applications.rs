/// A generic-application refusal must point inside `region`, the first occurrence of that text.
fn assert_generic_refusal_within(
    source: &str,
    error: &omega_typer::TypeError,
    region: &str,
) {
    let at = source
        .find(region)
        .unwrap_or_else(|| panic!("missing region {region:?}"));
    let (start, end) = (error.span.get_start_offset(), error.span.get_end_offset());
    assert!(
        at <= start && start < end && end <= at + region.len(),
        "{error:?} lies outside {region:?}"
    );
}

#[test]
fn generic_overloads_select_the_exact_qualified_nominal_operand() {
    let code = "data Io{data T{} data U{}}
        machine f<T>(x:T,y:Io::T)->bool{true}
        machine f<U>(x:U,y:Io::U)->bool{false}
        machine a(x:u8,y:Io::T)->bool{f(x,y)}
        machine b(x:u8,y:Io::U)->bool{f(x,y)}";
    let typed = check(&[("selection.omg", code)]).unwrap();
    for (caller, operand) in [("a", "Io :: T"), ("b", "Io :: U")] {
        let call = typed
            .calls
            .iter()
            .find(|call| call.caller.as_ref().is_some_and(|id| id.path == caller))
            .unwrap();
        assert_eq!(call.machine.path, "f");
        assert_eq!(call.machine.operands, ["#TypeParam0", operand]);
        assert_eq!(call.type_arguments, [ValueType::Integer { signed: false, width: Some(8) }]);
    }
}

#[test]
fn runtime_indices_at_calls_bind_the_actual_subject() {
    let code = "data B<K:u8>{} machine keep(n:u8,b:B<n>)->B<n>{b} machine use_it(x:u8,b:B<x>)->B<x>{keep(x,b)} machine fixed(b:B<8>)->B<8>{keep(8,b)}";
    check(&[("subjects.omg", code)]).unwrap();
    let wrong = code.replace("keep(8,b)", "keep(9,b)");
    let error = check(&[("subjects.omg", &wrong)]).unwrap_err();
    assert_eq!(error.file, "subjects.omg");
    assert_eq!(error.message, "no matching overload for keep(9, game\\B<8>)");
    assert_eq!(error.span.get_start_offset(), wrong.find("keep(9,b)").unwrap(), "{error:?}");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"keep".as_slice()));
}

#[test]
fn nested_calls_publish_static_arguments_for_the_exact_lexical_owners() {
    let code = "data BooleanStep=machine(value:bool)->bool; machine apply<const F:BooleanStep>(value:bool)->bool {
                    step::<u8>(value)
                    machine step<T>(input:bool)->bool {
                        invoke(input)
                        machine invoke(argument:bool)->bool {F(argument)}
                    }
                }";
    let typed = check(&[("lexical.omg", code)]).unwrap();
    let [indirect] = typed.indirect_calls.as_slice() else {
        panic!("expected one indirect call")
    };
    let call = typed
        .calls
        .iter()
        .find(|call| Some(&call.machine) == indirect.caller.as_ref())
        .unwrap();
    let step = call.caller.as_ref().unwrap();
    let apply = step.enclosing_machine.as_ref().unwrap();
    assert_eq!(call.machine.enclosing_machine.as_ref(), Some(step));
    let own_formal = |owner: &omega_resolved::DeclarationId| ValueType::Parameter {
        owner: Box::new(owner.clone()),
        index: 0,
    };
    assert_eq!(
        call.enclosing_arguments,
        [
            (step.clone(), vec![own_formal(step)]),
            (apply.clone(), vec![own_formal(apply)])
        ],
    );
    assert_eq!(
        indirect.entry,
        omega_typer::InvocationTarget::StaticParameter {
            owner: apply.clone(),
            index: 0
        },
    );
}

#[test]
fn source_static_calls_keep_the_machine_identity_until_the_arguments_close() {
    let defs = "const base:u8=3; machine twice(n:u8)->u8 reaches none crashes none terminates {let factor:u8=2; let doubled=n*factor; doubled} machine width(n:u8)->u8{twice(n)+base} ";
    let prefix = "data B<const K:u8>{} ";
    let program = format!(
        "{prefix}{defs} machine buffer<const N:u8>()->B<width(N)>{{B::<width(N)>{{}}}} machine f(x:B<width(4)>)->B<11>{{x}} machine g()->B<11>{{buffer::<4>()}}"
    );
    use omega_typer::{Site, SpecializedIndexValue, StaticValueRequest, TypingEnvironment};
    let run = |source: &str, n: u8, supplied: u16| {
        let site = |expr: &str| {
            let start = source.find(expr).unwrap();
            Site {
                package: "game".into(),
                file: "static.omg".into(),
                span: omega_lexer::SourceSpan::new(start, start + expr.len()),
            }
        };
        let carrier = ValueType::Integer { signed: false, width: Some(8) };
        let closed = (
            StaticValueRequest {
                node: evaluation::node(source, source.find("width(4)").unwrap(), "width(4)", omega_parser::NodeKind::Call),
                expression: site("width(4)"),
                carrier: carrier.clone(),
            },
            omega_typer::StaticValue::Integer(11.into()),
        );
        let application = SpecializedIndexValue {
            request: omega_typer::StaticValueRequest {
                node: evaluation::node(source, source.find("width(N)").unwrap(), "width(N)", omega_parser::NodeKind::Call),
                expression: site("width(N)"),
                carrier: omega_typer::ValueType::Integer { signed: false, width: Some(8) },
            },
            arguments: vec![ValueType::StaticInteger {
                carrier: Box::new(carrier),
                value: n.into(),
            }],
            value: omega_typer::StaticValue::Integer(supplied.into()),
        };
        support::check_in(
            &[("static.omg", source)],
            &TypingEnvironment {
                evaluated_indices: &[closed],
                specialized_indices: &[application],
                ..Default::default()
            },
        )
    };
    let pending = check(&[("static.omg", &program)]).unwrap_err();
    assert_eq!(pending.file, "static.omg");
    assert_eq!(pending.message, "static expression requires an admitted evaluation result");
    let stage = run(&program, 4, 11).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|call| call.machine.path == "buffer" && call.result.to_string() == "game\\B<11>")
    );
    let overflow = program.replace("buffer::<4>()", "buffer::<128>()");
    let error = run(&overflow, 128, 259).unwrap_err();
    assert_eq!(error.file, "static.omg");
    assert_eq!(error.message, "static index 259 exceeds u8");
    // Same receipts as the healthy run: only the declared result index of f changes,
    // so the refusal is the result landing of f's body, not evaluation or selection.
    let wrong = program.replace("f(x:B<width(4)>)->B<11>", "f(x:B<width(4)>)->B<12>");
    let error = run(&wrong, 4, 11).unwrap_err();
    assert_eq!(error.file, "static.omg");
    assert_eq!(error.message, "expected game\\B<12>, found game\\B<11>");
    let body = wrong.find("->B<12>{x}").unwrap() + "->B<12>{".len();
    assert_eq!(error.span.get_start_offset(), body, "{error:?}");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"x".as_slice()));
}

#[test]
fn specializing_an_index_checks_an_intermediate_even_when_the_final_value_fits() {
    let bad = "data B<const K:u8>{} machine value<const N:u8>()->B<(N+1)*0>{B::<(N+1)*0>{}} machine f(){_=value::<255>();}";
    let error = check(&[("i.omg", bad)]).unwrap_err();
    assert_eq!(error.file, "i.omg");
    assert_eq!(error.message, "static index 256 exceeds u8");
    check(&[("i.omg", &bad.replace("255", "254"))]).unwrap();
}

#[test]
fn wrapping_and_saturating_indices_keep_their_arithmetic_policy() {
    let head = "data B<const K:i8>{} ";
    for (expr, n, expected) in [
        ("N +% 100", "100", "-56"),
        ("N +| 100", "100", "127"),
        ("N - 2", "4", "2"),
        ("(N +| 100) +| -100", "100", "27"),
        ("N +| (100i8 +| -100)", "100", "100"),
    ] {
        let source = format!("{head} machine value<const N:i8>()->B<{expr}>{{B::<{expr}>{{}}}} machine call()->B<{expected}>{{value::<{n}>()}}");
        let product = check(&[("index.omg", &source)]).unwrap();
        assert_eq!(product.calls.last().unwrap().result.to_string(), format!("game\\B<{expected}>"));
    }
}

#[test]
fn typed_generics_and_transparent_aliases() {
    let code = "data Box<T> { value: T; } data Byte = u8; data Bytes = Box<Byte>; machine get<T>(x: Box<T>) -> T { x.value } machine id<T>(x:T) -> T {x} machine f(x:Bytes) -> u8 { id::<Byte>(get(x)) }";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert_eq!(
        stage
            .calls
            .iter()
            .map(|c| c.machine.path.as_str())
            .collect::<Vec<_>>(),
        ["get", "id"]
    );
    assert!(
        stage
            .calls
            .iter()
            .all(|c| c.type_arguments == [ValueType::Integer { signed: false, width: Some(8) }])
    );
    for (code, message) in [
        ("machine id<T>(x:T)->T{x} machine f()->u8{id::<bool>(2u8)}", "conflicting type argument bool"),
        ("machine id<T>(x:T)->T{x} machine f(){_ = id(1);}", "cannot infer all generic arguments"),
        ("data Box<T>{} machine f(x:Box){}", "Box expects 1 type arguments, found 0"),
        ("data A = B; data B = A;", "cyclic type alias"),
    ] {
        let error = check(&[("a.omg", code)]).unwrap_err();
        assert_eq!(error.file, "a.omg");
        if message.starts_with("cyclic") {
            // Either the definition order or the alias catalog may name the cycle first.
            assert!(error.message.starts_with(message), "{code}: {error:?}");
        } else {
            assert_eq!(error.message, message, "{code}");
        }
    }
}

fn assert_call_refusal(
    source: &str,
    site: &str,
    reason: &str,
) {
    let start = source.find(site).unwrap();
    assert_eq!(source.rfind(site), Some(start), "ambiguous test site: {source}");
    let error = check(&[("call.omg", source)]).unwrap_err();
    assert_eq!(error.file, "call.omg");
    assert_eq!(error.span, omega_lexer::SourceSpan::new(start, start + site.len()), "{source}");
    assert_eq!(error.message, reason, "{source}");
}

#[test]
fn wrong_arity_overloads_preserve_the_conflicting_type_argument_site() {
    let generic = "machine id<T>(x:T){}";
    let binary = "machine id(x:u8,y:u8){}";
    for (before, after) in [(generic, ""), (generic, binary), (binary, generic)] {
        let source = format!("{before} machine f(){{id::<bool>(2u8);}} {after}");
        assert_call_refusal(&source, "bool", "conflicting type argument bool");
        let good = source.replace("::<bool>", "::<u8>");
        check(&[("call.omg", &good)]).unwrap();
        if !after.is_empty() {
            let both = good.replace("(2u8);", "(2u8); id(1u8,2u8);");
            let stage = check(&[("call.omg", &both)]).unwrap();
            assert_eq!(stage.calls[0].machine.operands.len(), 1);
            assert_eq!(stage.calls[1].machine.operands.len(), 2);
        }
    }
}

#[test]
fn wrong_arity_trait_requirements_preserve_the_conflicting_type_argument_site() {
    let generic = "machine id<T>(x:T);";
    let binary = "machine id(x:u8,y:u8);";
    for (first, second) in [(generic, binary), (binary, generic)] {
        let source = format!("trait Ops{{{first} {second}}} machine f<const E:Ops>(){{E::id::<bool>(2u8); E::id(1u8,2u8);}}");
        assert_call_refusal(&source, "bool", "conflicting type argument bool");
        let good = source.replace("::<bool>", "::<u8>");
        let stage = check(&[("call.omg", &good)]).unwrap();
        assert_eq!(stage.calls.len(), 2);
        assert!(stage.calls.iter().all(|call| call.evidence.is_some()));
    }
}

#[test]
fn result_domain_excluded_overload_preserves_the_conflicting_type_argument_site() {
    let plain = "machine pick<T>(x:T)->u8{0u8}";
    let wire = "machine pick<T>(x:T)->u8 in Wire{0u8 as +Wire}";
    for (first, second) in [(plain, wire), (wire, plain)] {
        let source = format!("domain u8::Wire; {first} {second} machine f()->u8{{pick::<bool>(2u8)}}");
        assert_call_refusal(&source, "bool", "conflicting type argument bool");
        let good = source.replace("::<bool>", "::<u8>");
        let stage = check(&[("call.omg", &good)]).unwrap();
        assert!(stage.calls[0].result.domains().is_empty());
    }
}

#[test]
fn conflicting_type_argument_is_not_attributed_to_one_of_two_eligible_signatures() {
    let left = "machine pick<T>(x:T,y:u8){}";
    let right = "machine pick<T>(x:u8,y:T){}";
    for (first, second) in [(left, right), (right, left)] {
        let source = format!("{first} {second} machine f(){{pick::<bool>(1u8,2u8);}}");
        assert_call_refusal(&source, "pick::<bool>", "no matching overload for pick(u8, u8)");
        for operands in ["true,2u8", "1u8,true"] {
            check(&[("call.omg", &source.replace("1u8,2u8", operands))]).unwrap();
        }
    }
}

#[test]
fn source_generic_application_error_precedes_a_speculative_operand_conflict() {
    let generic = "machine id<T>(x:T){}";
    let concrete = "machine id(x:u16){}";
    for (first, second) in [(generic, concrete), (concrete, generic)] {
        let source = format!("{first} {second} machine f(){{id::<bool>(2u8);}}");
        assert_call_refusal(&source, "id::<bool>", "excess generic arguments");
        check(&[("call.omg", &source.replace("::<bool>", "::<u8>"))]).unwrap();
    }
}

#[test]
fn normalized_static_arguments_and_binder_occurrences() {
    let code = "data Buffer<const N:u8>{} data Eight=Buffer<4+4>; machine f(x:Eight)->Buffer<8>{x} machine forward<const N:u8>(x:Buffer<N>)->Buffer<N>{x} machine g(x:Buffer<8>)->Eight{forward::<8>(x)}";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert_eq!(stage.calls[0].result.to_string(), "game\\Buffer<8>");
    for (s, site) in [
        ("data B<const N:u8>{} machine f(x:B<256>){}", "256"),
        ("data B<const N:u8>{} machine f(x:B<8u16>){}", "8u16"),
    ] {
        let e = check(&[("a.omg", s)]).unwrap_err();
        assert_eq!(e.file, "a.omg");
        assert!(
            e.message.starts_with("const argument ") && e.message.ends_with(" does not land in u8"),
            "{e:?}"
        );
        assert_eq!(e.span.slice(s.as_bytes()), Some(site.as_bytes()), "{e:?}");
    }
    let duplicate = "data Wrap<T> = T; machine f<T,U>(a:T,b:U){} machine f<A,B>(a:Wrap<B>,b:A){}";
    let e = check(&[("a.omg", duplicate)]).unwrap_err();
    assert_eq!(e.file, "a.omg");
    assert_eq!(e.message, "duplicate typed operand signature for f");
}

#[test]
fn a_static_equation_recovers_the_extent_and_refuses_conflicting_indices() {
    let definition =
        "domain<const N:u64> u64::AtMost<N> requires self <= N; data Buffer<L,const N:u64> requires L == u64::AtMost::<N> { bytes:[u8;N]; length:L; }";
    let code = format!("{definition} machine bytes(x:Buffer<u64::AtMost<4>>)->[u8;4]{{x.bytes}}");
    check(&[("indices.omg", &code)]).unwrap();
    let code = format!("{definition} machine bad(x:Buffer<u64::AtMost<4>,8>){{}}");
    let error = check(&[("indices.omg", &code)]).unwrap_err();
    assert_eq!(error.file, "indices.omg");
    assert_eq!(error.message, "conflicting type equation for Buffer");
    assert_generic_refusal_within(&code, &error, "Buffer<u64::AtMost<4>,8>");
}

#[test]
fn symbolic_widths_and_array_extents_infer_and_specialize_to_the_operand() {
    let source = "machine sum<const N:u64>(x:u<N>,y:u<N>)->u<N> requires N>0{x+%y} machine head<T [copy],const N:u64>(xs:@[T;N])->T{xs[0]} machine f(xs:@[u13;4])->u13{sum(head(xs),17u13)}";
    let stage = check(&[("width.omg", source)]).unwrap();
    let sum = stage.calls.iter().find(|c| c.machine.path == "sum").unwrap();
    assert_eq!(sum.type_arguments[0].to_string(), "13");
    let head = stage.calls.iter().find(|c| c.machine.path == "head").unwrap();
    assert_eq!(head.type_arguments[1].to_string(), "4");
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, omega_typer::ObligationKind::PositiveWidth(_)))
    );
    assert_eq!(sum.result.to_string(), "u13");
    let suffixed = "machine wrong(x:u<13u8>){}";
    let e = check(&[("width.omg", suffixed)]).unwrap_err();
    assert_eq!(e.file, "width.omg");
    assert_eq!(e.message, "integer width requires a static u64");
    assert_eq!(e.span.slice(suffixed.as_bytes()), Some(b"13u8".as_slice()));
    let zero = "machine zero<const N:u64>()->u<N>{0} machine f(){ _ = zero::<0>(); }";
    let e = check(&[("width.omg", zero)]).unwrap_err();
    assert_eq!(e.file, "width.omg");
    assert_eq!(e.message, "integer width must be positive", "{e:?}");
}

#[test]
fn evaluated_indices_check_the_source_type_and_key_only_on_the_value() {
    use omega_typer::{Site, StaticValueRequest, TypingEnvironment};
    let run = |code: &str, result: i64| {
        let start = code.find("b:Buf<").unwrap() + "b:Buf<".len();
        let end = start + code[start..].find('>').unwrap();
        let receipt = (
            StaticValueRequest {
                node: evaluation::node(
                    code,
                    start,
                    &code[start..end],
                    if code[start..end].ends_with(')') {
                        omega_parser::NodeKind::Call
                    } else {
                        omega_parser::NodeKind::PathType
                    },
                ),
                expression: Site {
                    package: "game".into(),
                    file: "calc.omg".into(),
                    span: omega_lexer::SourceSpan::new(start, end),
                },
                carrier: ValueType::Integer {
                    signed: false,
                    width: Some(64),
                },
            },
            omega_typer::StaticValue::Integer(result.into()),
        );
        support::check_in(
            &[("calc.omg", code)],
            &TypingEnvironment {
                evaluated_indices: &[receipt],
                ..Default::default()
            },
        )
    };
    let head = "data Buf<const N:u64>{array:[u8;N];} ";
    let source = format!("{head} const size:u64=8; machine f(b:Buf<size>)->Buf<8>{{b}}");
    assert_eq!(check(&[("calc.omg", &source)]).unwrap().evaluated_indices.len(), 1);
    let stage = run(&source, 8).unwrap();
    assert_eq!(stage.evaluated_indices.len(), 1);
    assert_eq!(stage.signatures[0].parameters[0].1, stage.signatures[0].result);
    let error = run(&source, 7).unwrap_err();
    assert_eq!(error.file, "calc.omg");
    assert_eq!(error.message, "evaluation result disagrees with the primitive index");
    let computed = source
        .replace("const size:u64=8;", "machine size()->u64{8}")
        .replace("b:Buf<size>", "b:Buf<size()>");
    assert!(
        run(&computed, -1)
            .unwrap_err()
            .message
            .contains("does not land")
    );
    let bad = format!("{head} const flag:bool=true; machine f(b:Buf<flag>)->Buf<8>{{b}}");
    let error = run(&bad, 8).unwrap_err();
    assert_eq!(error.file, "calc.omg");
    assert!(error.message.starts_with("expected u64, found bool"), "{error:?}");
}

#[test]
fn long_chains_of_transparent_aliases_do_not_consume_the_host_call_stack() {
    let mut code = String::from("data A0 = bool;\n");
    for index in 1..1024 {
        code.push_str(&format!("data A{index} = A{};\n", index - 1));
    }
    code.push_str("machine f(x:A1023)->bool{x}");
    let reversed = code.lines().rev().collect::<Vec<_>>().join("\n");
    for source in [&code, &reversed] {
        let stage = check(&[("chain.omg", source)]).unwrap();
        assert_eq!(stage.signatures[0].parameters[0].1, ValueType::Bool);
    }
}

#[test]
fn open_exact_indices_keep_source_operations_and_normalize_on_specialization() {
    let head = "data Buffer<const K:u8>{} ";
    let source = format!(
        "{head} machine plus<const N:u8>(b:Buffer<N + 1>)->Buffer<1 + N>{{b}} machine neutral<const N:u8>(b:Buffer<N + 0>)->Buffer<N>{{b}} machine f(b:Buffer<8>)->Buffer<8>{{plus::<7>(b)}}"
    );
    let stage = check(&[("indices.omg", &source)]).unwrap();
    assert!(
        stage
            .operations
            .iter()
            .any(|op| op.token == "+" && op.result.to_string() == "u8")
    );
    let call = stage.calls.iter().find(|c| c.machine.path == "plus").unwrap();
    assert!(call.result.to_string().ends_with("Buffer<8>"));
    let rejected = format!("{head} machine make<const N:u8>()->Buffer<N + 1>{{Buffer::<N + 1>{{}}}} machine bad(){{ _ = make::<255>(); }}");
    let err = check(&[("indices.omg", &rejected)]).unwrap_err();
    assert_eq!(err.file, "indices.omg");
    assert_eq!(err.message, "static index 256 exceeds u8", "{err:?}");
}

#[test]
fn canonical_open_index_signatures_cannot_redeclare_the_same_overload() {
    let code = "data B<const N:u64>{} machine select<const X:u64>(b:B<X+1>){} machine select<const Y:u64>(b:B<1+Y>){}";
    let error = check(&[("duplicate.omg", code)]).unwrap_err();
    assert_eq!(error.file, "duplicate.omg");
    assert_eq!(error.message, "duplicate typed operand signature for select");
    // Distinct spellings of distinct indices remain separate overloads.
    check(&[("duplicate.omg", &code.replace("B<1+Y>", "B<2+Y>"))]).unwrap();
}

#[test]
fn generic_calls_preserve_the_inferred_result_domain() {
    let prefix = "domain u8::A; domain u8::B; machine id<T>(x:T)->T{x} machine take(x:u8 in A){}";
    let source = format!("{prefix} machine direct(x:u8 in A)->u8 in A{{id(x)}} machine nested(x:u8 in A){{take(id(x))}}");
    let stage = check(&[("a.omg", &source)]).unwrap();
    let identities: Vec<_> = stage.calls.iter().filter(|c| c.machine.path == "id").collect();
    assert_eq!(identities.len(), 2);
    for call in identities {
        assert_eq!(call.type_arguments, std::slice::from_ref(&call.result));
        assert_eq!(call.result.domains()[0].declaration.path, "u8::A");
    }
    for destination in ["u8", "u8 in B"] {
        let source = format!("{prefix} machine wrong(x:u8 in A)->{destination}{{id(x)}}");
        let error = check(&[("a.omg", &source)]).unwrap_err();
        assert!(error.message.contains("no matching overload for id"), "{error:?}");
        assert_eq!(error.span.slice(source.as_bytes()).unwrap(), b"id");
    }
}

#[test]
fn indexed_result_domains_match_after_inferring_the_index_from_operands() {
    let prefix = "domain<const N:u8> u8::Label<N>; data B<const N:u8>{}
        machine tag<const N:u8>(b:B<N>,x:u8 in Label<N>)->u8 in Label<N>{x}";
    let source = format!(
        "{prefix}
        machine inferred(b:B<2>,x:u8 in Label<2>)->u8 in Label<2>{{tag(b,x)}}
        machine explicit(b:B<2>,x:u8 in Label<2>)->u8 in Label<2>{{tag::<2>(b,x)}}"
    );
    let stage = check(&[("index.omg", &source)]).unwrap();
    let [inferred, explicit] = &stage.calls[..] else {
        panic!("expected both tag calls");
    };
    assert_eq!(inferred.machine, explicit.machine);
    assert_eq!(inferred.type_arguments, explicit.type_arguments);
    assert_eq!(inferred.result, explicit.result);
    let wrong = format!("{prefix} machine wrong(b:B<2>,x:u8 in Label<2>)->u8 in Label<3>{{tag(b,x)}}");
    let error = check(&[("index.omg", &wrong)]).unwrap_err();
    assert_eq!(error.file, "index.omg");
    assert_eq!(error.message, "no matching overload for tag(game\\B<2>, u8 in game\\u8::Label<2>)");
    assert_eq!(error.span.get_start_offset(), wrong.rfind("tag(b,x)").unwrap(), "{error:?}");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"tag".as_slice()));
}

#[test]
fn result_domains_retain_the_actual_runtime_index() {
    let prefix = "domain<K:u8> u8::Label<K>; machine tag(n:u8,x:u8 in Label<n>)->u8 in Label<n>{x}";
    let source = format!(
        "{prefix}
        machine forward(k:u8,x:u8 in Label<k>)->u8 in Label<k>{{tag(k,x)}}
        machine fixed(x:u8 in Label<2>)->u8 in Label<2>{{tag(2,x)}}"
    );
    let stage = check(&[("runtime.omg", &source)]).unwrap();
    for call in &stage.calls {
        let caller = call.caller.as_ref().unwrap();
        let signature = stage
            .signatures
            .iter()
            .find(|signature| &signature.machine == caller)
            .unwrap();
        assert_eq!(call.result, signature.result);
    }
    let wrong = format!("{prefix} machine wrong(k:u8,m:u8,x:u8 in Label<k>)->u8 in Label<m>{{tag(k,x)}}");
    let error = check(&[("runtime.omg", &wrong)]).unwrap_err();
    assert_eq!(error.file, "runtime.omg");
    assert!(error.message.starts_with("no matching overload for tag("), "{error:?}");
    assert_eq!(error.span.get_start_offset(), wrong.rfind("tag(k,x)").unwrap(), "{error:?}");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"tag".as_slice()));
}

#[test]
fn runtime_domain_sets_normalize_when_call_arguments_coincide_or_change_order() {
    let prefix = "domain<K:u8> u8::Label<K>;
        machine both(a:u8,b:u8,x:u8 in Label<a> & Label<b>)->u8 in Label<a> & Label<b>{x}";
    let source = format!(
        "{prefix}
        machine same(k:u8,x:u8 in Label<k>)->u8 in Label<k>{{both(k,k,x)}}
        machine swapped(k:u8,m:u8,x:u8 in Label<k> & Label<m>)->u8 in Label<k> & Label<m>{{both(m,k,x)}}"
    );
    let stage = check(&[("sets.omg", &source)]).unwrap();
    let [same, swapped] = &stage.calls[..] else { panic!("expected both calls") };
    assert_eq!(same.result.domains().len(), 1);
    assert_eq!(swapped.result.domains().len(), 2);
    let wrong = format!("{prefix} machine drop_one(k:u8,m:u8,x:u8 in Label<k> & Label<m>)->u8 in Label<k>{{both(m,k,x)}}");
    let error = check(&[("sets.omg", &wrong)]).unwrap_err();
    assert_eq!(error.file, "sets.omg");
    assert!(error.message.starts_with("no matching overload for both("), "{error:?}");
    assert_eq!(error.span.get_start_offset(), wrong.rfind("both(m,k,x)").unwrap(), "{error:?}");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"both".as_slice()));
}

#[test]
fn indexed_domain_aliases_preserve_each_constituent_and_its_arguments() {
    let defs = "domain<const N:u8> u8::Bound<N>; domain u8::Five = u8::Bound<5>; domain u8::Six = u8::Bound<6>;
        domain<const M:u8> u8::Pair<M> = u8::Five & u8::Bound<M>; domain u8::Both = u8::Pair<6>;";
    let good = "machine expand(x:u8 in Both)->u8 in Bound<5> & Bound<6>{x}
        machine attach(x:u8)->u8 in Both {x as +Both}
        machine remove(x:u8 in Both)->u8 {x as -Both}";
    let stage = check(&[("use.omg", good), ("domains.omg", defs)]).unwrap();
    let machine = stage
        .signatures
        .iter()
        .find(|s| s.machine.path == "expand")
        .unwrap();
    assert_eq!(machine.parameters[0].1, machine.result);
    assert_eq!(
        machine
            .result
            .domains()
            .iter()
            .map(|a| a.arguments[0].to_string())
            .collect::<Vec<_>>(),
        ["5", "6"]
    );
    let unused = check(&[("domains.omg", defs), ("arrays.omg", "data Bytes = [u8; 4];")]).unwrap();
    let (id, members) = unused
        .domain_alias_members
        .iter()
        .find(|(id, _)| id.path == "u8::Both")
        .unwrap();
    assert_eq!(id.package, machine.machine.package);
    spec_corpus::observe_handoff!("type-and-domain-aliases/producer", assert_eq!(members, machine.result.domains()));
    let (_, array) = unused
        .type_alias_targets
        .iter()
        .find(|(id, _)| id.path == "Bytes")
        .unwrap();
    assert_eq!(*array, ValueType::Array(Box::new(ValueType::Integer { signed: false, width: Some(8) }), 4));
    let wrong = "machine substitute(x:u8 in Five)->u8 in Six {x}";
    let error = check(&[("domains.omg", defs), ("use.omg", wrong)]).unwrap_err();
    assert_eq!(error.file, "use.omg");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"x".as_slice()));
    spec_corpus::observe_handoff!("type-and-domain-aliases/refusal", assert!(error.message.contains("Bound<6>") && error.message.contains("Bound<5>"), "{error:?}"));
    for (target, message) in [
        ("u8::Bound", "incorrect indexed domain arity"),
        ("u8::Bound<256>", "const argument 256 does not land in u8"),
    ] {
        let invalid = format!("domain u8::Invalid = {target};");
        let error = check(&[("invalid.omg", &invalid), ("domains.omg", defs)]).unwrap_err();
        assert_eq!(error.file, "invalid.omg");
        assert_eq!(error.message, message, "{error:?}");
        assert_generic_refusal_within(&invalid, &error, target);
    }
}

#[test]
fn indexed_domain_type_arguments_normalize_under_both_spellings() {
    let defs = "data Byte = u8; domain<Frame> u8::Coordinates<Frame>;";
    for (input, output) in [
        ("u8::Coordinates<Byte>", "u8 in Coordinates<u8>"),
        ("u8 in Coordinates<u8>", "u8::Coordinates<Byte>"),
    ] {
        let same = format!("machine take(x:{input})->{output} {{x}}");
        check(&[("types.omg", defs), ("use.omg", &same)]).unwrap();
        let different = same.replace("Coordinates<Byte>", "Coordinates<bool>");
        let error = check(&[("types.omg", defs), ("use.omg", &different)]).unwrap_err();
        assert_eq!(error.file, "use.omg");
        let (expected, found) = if input.contains("Byte") { ("u8", "bool") } else { ("bool", "u8") };
        assert_eq!(
            error.message,
            format!("expected u8 in game\\u8::Coordinates<{expected}>, found u8 in game\\u8::Coordinates<{found}>")
        );
        assert_eq!(error.span.get_start_offset(), different.rfind("{x}").unwrap() + 1, "{error:?}");
    }
}

#[test]
fn domain_alias_indices_are_checked_after_substitution_at_each_use() {
    let defs = "domain<const N:u8> u8::Bound<N>; domain<const M:u8> u8::Next<M> = u8::Bound<M+1>;";
    for source in [
        "machine value(x:u8 in Next<254>)->u8 in Bound<255> {x}",
        "machine cast(x:u8) {let y=x as +Next<254>;}",
        "machine member(x:u8)->bool {x in Next::<254>}",
    ] {
        check(&[("domains.omg", defs), ("use.omg", source)]).unwrap();
        let overflow = source.replace("254", "255");
        let error = check(&[("domains.omg", defs), ("use.omg", &overflow)]).unwrap_err();
        assert_eq!(error.file, "use.omg");
        assert_eq!(error.message, "static index 256 exceeds u8");
    }
}

// A builtin or bound root of a qualified domain path owns its written argument
// list. Typing forms it exactly as the same carrier written alone: u<8> lands as
// u8, and an argument list on u8 or a type binder is refused in both spellings.
#[test]
fn applied_carrier_roots_of_domain_paths_form_like_direct_carriers() {
    let generic = "domain<T> T::Ready;";
    for (defs, source) in [
        (generic, "machine f(x:u8 in u<8>::Ready)->u8 in u8::Ready {x}"),
        (generic, "machine f(x:i16 in i<16>::Ready)->i16 in Ready {x}"),
        (generic, "machine f(x:u8)->u8 in u<8>::Ready {x as +u<8>::Ready}"),
        ("domain<T> T::Ready; machine take<const A:core\\Type<0>>(){}", "machine f(){take::<u<8>::Ready>();}"),
        ("domain<const K:u8> u8::Limit<K>; domain u8::Limit<2>::Pair;", "machine f(x:u8 in u8::Limit<2>::Pair){}"),
    ] {
        let stage = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        if let Some(call) = stage.calls.iter().find(|call| call.machine.path == "take") {
            assert_eq!(call.type_arguments[0].carrier().to_string(), "u8");
            assert!(matches!(call.type_arguments[0].domains(), [domain] if domain.declaration.path == "T::Ready"));
        }
    }
    let arguments = "generic arguments require an applicable type declaration";
    let invalid_u8 = "builtin type u8 does not take arguments";
    for (defs, source, site, message) in [
        (generic, "machine f(x:u8<1>){}", "<1>", invalid_u8),
        ("domain u8::Ready;", "machine f(x:u8 in u8<1>::Ready){}", "<1>", invalid_u8),
        (generic, "machine f(x:u8 in u8<1>::Ready){}", "<1>", invalid_u8),
        ("domain<const K:u8> u8::Limit<K>; domain u8::Limit<2>::Pair;", "machine f(x:u8 in u8<1>::Limit<2>::Pair){}", "<1>", invalid_u8),
        ("domain<const N:u8> u8::Bound<N>;", "domain u8::Five = u8<1>::Bound<5>;", "<1>", invalid_u8),
        (generic, "machine f(x:u8){let y = x as +u8<1>::Ready;}", "<1>", invalid_u8),
        (generic, "machine f<T>(x:T<3>){}", "<3>", arguments),
        (generic, "machine f<T>(x:T in T<3>::Ready){}", "<3>", arguments),
        (generic, "machine f(x:u16 in u<8>::Ready){}", "u<8>::Ready", "domain qualifier names u8, found u16"),
    ] {
        let error = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_err();
        assert_eq!(error.file, "use.omg", "{source}: {error:?}");
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()), "{source}: {error:?}");
        assert_eq!(error.message, message, "{source}");
    }
    // The explicit argument reaches the same carrier formation through call selection.
    let explicit = "machine f(){take::<u8<1>::Ready>();}";
    let error = check(&[("domains.omg", "domain<T> T::Ready; machine take<const A:core\\Type<0>>(){}"), ("use.omg", explicit)]).unwrap_err();
    assert_eq!(error.file, "use.omg", "{error:?}");
    assert_eq!(error.span.slice(explicit.as_bytes()), Some(b"<1>".as_slice()), "{error:?}");
    assert_eq!(error.message, invalid_u8);
}

// `id::<u8 in P>(pick(x))` is the explicit spelling of the type argument that
// `let v: u8 in P = pick(x); id(v)` infers. Both must select the `u8 in P`
// overload of `pick` and record the same qualified type argument. The negative
// twin supplies the bare carrier `u8`, so no `id` returns the qualified result.
#[test]
fn an_explicit_qualified_type_argument_selects_the_qualified_result_overload() {
    let defs = "domain u8::P; data Box<T> { v: T; } machine pick(x: u8) -> u8 { x } machine pick(x: u8) -> u8 in P { x as +P } machine id<T>(v: T) -> T { v }";
    let qualified = "u8 in game\\u8::P";
    for (caller, id_site) in [
        ("machine c(x: u8) -> u8 in P { id::<u8 in P>(pick(x)) }", "id::<u8 in P>(pick(x))"),
        ("machine c(x: u8) -> u8 in P { let v: u8 in P = pick(x); id(v) }", "id(v)"),
    ] {
        let stage = check(&[("defs.omg", defs), ("use.omg", caller)]).unwrap();
        let call = |site: &str| {
            stage
                .calls
                .iter()
                .find(|c| c.site.file == "use.omg" && c.site.span.slice(caller.as_bytes()) == Some(site.as_bytes()))
                .unwrap_or_else(|| panic!("{site} in {caller}"))
        };
        let pick = call("pick(x)");
        assert_eq!(pick.result.to_string(), qualified, "{caller}");
        let id = call(id_site);
        assert_eq!(id.machine.path, "id");
        assert_eq!(
            id.type_arguments
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [qualified],
            "{caller}"
        );
        assert_eq!(id.result.to_string(), qualified, "{caller}");
    }
    let bare = "machine c(x: u8) -> u8 in P { id::<u8>(pick(x)) }";
    let error = check(&[("defs.omg", defs), ("use.omg", bare)]).unwrap_err();
    assert_eq!(error.file, "use.omg");
    assert_eq!(error.span.slice(bare.as_bytes()), Some(&b"id::<u8>"[..]), "{error:?}");
    assert_eq!(error.message, "no matching overload for id(u8)");
    let wrong_head = "machine c<const N:u8>(b:Box<N in P>) {}";
    let error = check(&[("defs.omg", defs), ("use.omg", wrong_head)]).unwrap_err();
    assert_eq!(error.file, "use.omg");
    // A value binder cannot be the carrier of the qualified type argument.
    assert_eq!(error.message, "expected a type declaration or type parameter", "{error:?}");
    assert_eq!(error.span.get_start_offset(), wrong_head.find("N in P").unwrap(), "{error:?}");
    assert_eq!(error.span.slice(wrong_head.as_bytes()), Some(b"N".as_slice()));
    // The same qualified argument in a type: the field reads back qualified.
    check(&[
        ("defs.omg", defs),
        ("use.omg", "machine c(b: Box<u8 in P>) -> u8 in P { b.v }"),
    ])
    .unwrap();
    let error = check(&[
        ("defs.omg", defs),
        ("use.omg", "machine c(b: Box<u8>) -> u8 in P { b.v }"),
    ])
    .unwrap_err();
    assert_eq!(error.message, "expected u8 in game\\u8::P, found u8");
}

#[test]
fn closed_results_belong_to_one_occurrence_and_carrier() {
    use omega_typer::{Site, StaticValueRequest, TypingEnvironment};
    let source = "const size:u8=3; data B<const N:u8>{} machine f(x:B<size>){}";
    let site = source.rfind("size").unwrap();
    let key = StaticValueRequest {
        node: evaluation::node(source, site, "size", omega_parser::NodeKind::PathType),
        expression: Site {
            package: "game".into(),
            file: "a.omg".into(),
            span: omega_lexer::SourceSpan::new(site, site + 4),
        },
        carrier: ValueType::Integer { signed: false, width: Some(8) },
    };
    let run = |receipts: &[_]| {
        support::check_in(
            &[("a.omg", source)],
            &TypingEnvironment {
                evaluated_indices: receipts,
                ..Default::default()
            },
        )
    };
    let good = (key.clone(), omega_typer::StaticValue::Integer(3.into()));
    assert_eq!(
        run(&[good.clone(), good.clone()])
            .unwrap()
            .evaluated_indices
            .as_slice(),
        std::slice::from_ref(&good)
    );
    let conflict = run(&[good, (key.clone(), omega_typer::StaticValue::Integer(4.into()))]).unwrap_err();
    assert_eq!(conflict.file, "a.omg");
    assert_eq!(conflict.message, "conflicting evaluation results for a closed expression");
    for wrong in [
        StaticValueRequest {
            carrier: ValueType::Integer {
                signed: false,
                width: Some(16),
            },
            ..key.clone()
        },
        StaticValueRequest {
            expression: Site {
                file: "absent.omg".into(),
                ..key.expression.clone()
            },
            ..key.clone()
        },
        StaticValueRequest {
            expression: Site {
                span: omega_lexer::SourceSpan::new(6, 10),
                ..key.expression.clone()
            },
            ..key
        },
    ] {
        let error = run(&[(wrong.clone(), omega_typer::StaticValue::Integer(3.into()))]).unwrap_err();
        assert_eq!(error.message, "evaluation result has no matching static expression and carrier");
        assert_eq!((error.file, error.span), (wrong.expression.file, wrong.expression.span));
    }
}

#[test]
fn specialized_results_require_a_deferred_index_of_the_same_carrier_and_arity() {
    use omega_typer::{Site, SpecializedIndexValue, StaticValueRequest, TypingEnvironment};
    let source = "data B<const K:u8>{} machine width(n:u8)->u8{n+1} machine buf<const N:u8>()->B<width(N)>{B::<width(N)>{}}";
    let u8_type = ValueType::Integer { signed: false, width: Some(8) };
    let argument = ValueType::StaticInteger {
        value: 3.into(),
        carrier: Box::new(u8_type.clone()),
    };
    for (expression, carrier, arguments, valid) in [
        ("width(N)", u8_type.clone(), vec![argument.clone()], true),
        ("width(N)", u8_type.clone(), vec![], false),
        (
            "width(N)",
            ValueType::Integer {
                signed: false,
                width: Some(16),
            },
            vec![argument.clone()],
            false,
        ),
        ("n+1", u8_type, vec![argument], false),
    ] {
        let offset = source.find(expression).unwrap();
        let request = StaticValueRequest {
            node: evaluation::node(
                source,
                offset,
                expression,
                if expression == "n+1" {
                    omega_parser::NodeKind::Binary
                } else {
                    omega_parser::NodeKind::Call
                },
            ),
            expression: Site {
                package: "game".into(),
                file: "requests.omg".into(),
                span: omega_lexer::SourceSpan::new(offset, offset + expression.len()),
            },
            carrier,
        };
        let value = SpecializedIndexValue {
            request,
            arguments,
            value: omega_typer::StaticValue::Integer(4.into()),
        };
        let checked = support::check_in(
            &[("requests.omg", source)],
            &TypingEnvironment {
                specialized_indices: std::slice::from_ref(&value),
                ..Default::default()
            },
        );
        if valid {
            checked.unwrap();
        } else {
            let e = checked.unwrap_err();
            assert_eq!(e.file, "requests.omg");
            assert_eq!(e.span, value.request.expression.span);
            assert_eq!(e.message, "evaluation result has no matching specialized expression, carrier and owner arity");
        }
    }
}

#[test]
fn qualified_argument_head_uses_the_receiving_parameter_of_its_application() {
    let source = "data Sort=core\\Type<0>; domain u8::Small; data X=u8; data Pair<A,B:Sort>{v:B;} data Alias<const X:u8> = Pair<bool,X in Small>; machine f(x:Alias<0>)->u8 in Small{x.v}";
    let stage = check(&[("qualified.omg", source)]).unwrap();
    assert_eq!(stage.signatures[0].result.to_string(), "u8 in game\\u8::Small");
}

#[test]
fn inferred_tuple_arguments_retain_their_components_ownership_restrictions() {
    let defs = "data A{} data L [linear]{} machine copies<T [copy]>(x:T){} machine consumes<T [linear]>(x:T){}";
    let compile = |body: &str| check(&[("types.omg", defs), ("use.omg", body)]);
    compile("machine f(){copies((true,1u8))}").unwrap();
    for tuple in ["(a,true)", "(true,a)", "(true,(a,true))", "(l,a)", "(a,l)"] {
        let code = format!("machine f(a:A,l:L){{copies({tuple})}}");
        let error = compile(&code).unwrap_err();
        assert_eq!(error.file, "use.omg");
        assert!(
            error.message.starts_with("type argument (") && error.message.ends_with(") does not satisfy Copy for copies"),
            "{code}: {error:?}"
        );
    }
    for tuple in ["(a,l)", "(l,a)", "(true,(l,true))"] {
        let code = format!("machine f(a:A,l:L){{consumes({tuple})}}");
        let stage = compile(&code).unwrap();
        assert!(matches!(stage.calls[0].type_arguments.as_slice(), [ValueType::Tuple(_)]));
    }
}

#[test]
fn each_applied_binder_selects_its_namespace_from_the_same_visible_names() {
    let code = "machine pick<T,N:u32>() {} machine caller(u32:u32) {pick::<u32,u32>()}";
    let stage = check(&[("names.omg", code)]).unwrap();
    let call = stage.calls.iter().find(|c| c.machine.path == "pick").unwrap();
    spec_corpus::observe_handoff!(
        "generic-arguments/consumer",
        assert!(
            matches!(call.type_arguments.as_slice(), [ty, ValueType::Subject{declaration, ..}] if ty.to_string()=="u32" && declaration.span.get_start_offset()==code.find("u32:u32").unwrap())
        )
    );
}

#[test]
fn a_public_generic_argument_can_only_select_a_visible_name_in_its_namespace() {
    let declarations = "data Key{} public const Key:u8=1; public data Tag<T>{} public data Number<const N:u8>{}";
    let good = format!("{declarations} public machine read(x:Number<Key>){{}}");
    check(&[("visibility.omg", &good)]).unwrap();
    let denied = good.replace("Number<Key>", "Tag<Key>");
    let error = check(&[("visibility.omg", &denied)]).unwrap_err();
    spec_corpus::observe_handoff!("generic-arguments/refusal", assert_eq!(error.span.get_start_offset(), denied.rfind("Key").unwrap()));
}

#[test]
fn a_field_width_uses_the_data_arguments_even_when_no_field_is_read() {
    let data = ("types.omg", "data Word<const N:u64>{value:u<N-1>;} data Alias<const K:u64> = Word<K>;");
    for (value, admits) in [(2, true), (1, false)] {
        let source = format!("machine consume(x:Alias<{value}>){{}}");
        let caller = ("caller.omg", source.as_str());
        for files in [[data, caller], [caller, data]] {
            let result = check(&files);
            if admits {
                let stage = result.unwrap();
                assert!(
                    stage
                        .obligations
                        .iter()
                        .any(|obligation| matches!(&obligation.kind, omega_typer::ObligationKind::PositiveWidth(_))
                            && obligation.site.span.slice(data.1.as_bytes()) == Some(b"u<N-1>"))
                );
            } else {
                assert_eq!(result.unwrap_err().message, "integer width must be positive");
            }
        }
    }
}

#[test]
fn runtime_indices_cannot_determine_a_storage_width() {
    for width in ["N", "N+1"] {
        let code = format!("machine runtime<N:u64>(x:u<{width}>){{}}");
        let error = check(&[("width.omg", &code)]).unwrap_err();
        assert_eq!(error.file, "width.omg");
        assert_eq!(error.message, "static index cannot depend on a runtime parameter");
        assert_eq!(error.span.get_start_offset(), code.find("u<N").unwrap() + 2, "{error:?}");
        assert_eq!(error.span.slice(code.as_bytes()), Some(b"N".as_slice()));
        // The same width over a static binder is admitted.
        check(&[("width.omg", &code.replace("<N:u64>", "<const N:u64>"))]).unwrap();
    }
}

#[test]
fn a_closed_data_application_checks_its_field_indices_in_either_declaration_order() {
    let data = "data Item<const X:u8>{} data Buffer<const N:u8>{field:Item<N+1>;}";
    let application = "machine use_it(x:Buffer<255>){}";
    for (first, last) in [(data, application), (application, data)] {
        let code = format!("{first} {last}");
        let error = check(&[("buffer.omg", &code)]).unwrap_err();
        assert_eq!(error.message, "static index 256 exceeds u8");
    }
}

#[test]
fn evaluation_of_a_field_index_is_keyed_by_the_applied_data_declaration() {
    use omega_typer::{Site, SpecializedIndexValue, StaticValueRequest, TypingEnvironment};
    let code = "machine size(n:u64)->u64{n} data Word<const N:u64>{value:i<size(N)>;} machine caller(x:Word<3>){}";
    let span = code.find("size(N)").unwrap();
    let carrier = ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    let run = |result: u64| {
        let value = SpecializedIndexValue {
            request: StaticValueRequest {
                node: evaluation::node(code, span, "size(N)", omega_parser::NodeKind::Call),
                expression: Site {
                    package: "game".into(),
                    file: "word.omg".into(),
                    span: omega_lexer::SourceSpan::new(span, span + "size(N)".len()),
                },
                carrier: carrier.clone(),
            },
            arguments: vec![ValueType::StaticInteger {
                carrier: Box::new(carrier.clone()),
                value: 3.into(),
            }],
            value: omega_typer::StaticValue::Integer(result.into()),
        };
        support::check_in(
            &[("word.omg", code)],
            &TypingEnvironment {
                specialized_indices: &[value],
                ..Default::default()
            },
        )
    };
    let pending = check(&[("word.omg", code)]).unwrap_err();
    assert_eq!(pending.file, "word.omg");
    assert_eq!(pending.message, "specialized static expression in Word needs an admitted evaluation result");
    let stage = run(7).unwrap();
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, omega_typer::ObligationKind::PositiveWidth(_)))
    );
    assert_eq!(run(0).unwrap_err().message, "integer width must be positive");
}

#[test]
fn forwarding_a_runtime_index_to_a_static_parameter_rejects_at_the_argument() {
    for argument in ["N", "N+1"] {
        let code = format!("data Slot<const K:u64>{{}} machine use_it<N:u64>(x:Slot<{argument}>){{}}");
        let error = check(&[("index.omg", &code)]).unwrap_err();
        assert_eq!(error.file, "index.omg");
        assert_eq!(error.message, "static index cannot depend on a runtime parameter");
        assert_eq!(error.span.get_start_offset(), code.find("Slot<N").unwrap() + "Slot<".len(), "{error:?}");
        assert_eq!(error.span.slice(code.as_bytes()), Some(b"N".as_slice()));
        check(&[("index.omg", &code.replace("const K:u64", "K:u64"))]).unwrap();
    }
}

#[test]
fn nested_field_evaluation_uses_both_enclosing_and_local_arguments() {
    use omega_typer::{Site, SpecializedIndexValue, StaticValueRequest, TypingEnvironment};
    let code = "machine width(a:u64,b:u64)->u64{a+b} data Outer<const A:u64>{data Inner<const B:u64>{v:i<width(A,B)>;}} machine consume(x:Outer<1>::Inner<2>,y:Outer<2>::Inner<1>){}";
    let index = "width(A,B)";
    let start = code.find(index).unwrap();
    let carrier = ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    let receipt = |a: u64, b: u64, value: u64| SpecializedIndexValue {
        request: StaticValueRequest {
            node: evaluation::node(code, start, index, omega_parser::NodeKind::Call),
            expression: Site {
                package: "game".into(),
                file: "nested.omg".into(),
                span: omega_lexer::SourceSpan::new(start, start + index.len()),
            },
            carrier: carrier.clone(),
        },
        arguments: [a, b]
            .map(|n| ValueType::StaticInteger {
                carrier: Box::new(carrier.clone()),
                value: n.into(),
            })
            .to_vec(),
        value: omega_typer::StaticValue::Integer(value.into()),
    };
    let run = |values: &[_]| {
        support::check_in(
            &[("nested.omg", code)],
            &TypingEnvironment {
                specialized_indices: values,
                ..Default::default()
            },
        )
    };
    let exact_request = receipt(1, 2, 3).request;
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    let package = omega_toolchain::resolve_source_package(&[omega_resolver::StageSource {
        package: "game",
        source: omega_resolver::ParsedSource {
            logical_path: "nested.omg",
            text: code,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
    .unwrap();
    let omega_typer::TypingProgress::Pending(pending) = omega_typer::type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap()
    else {
        panic!("nested field width requires closed preparation");
    };
    let demands: Vec<_> = pending
        .demands()
        .iter()
        .filter(|demand| demand.request() == &exact_request)
        .collect();
    assert_eq!(demands.len(), 2, "each closed application must reach preparation");
    for (outer, inner) in [(1, 2), (2, 1)] {
        let expected = receipt(outer, inner, 3);
        assert!(
            demands.iter().any(|demand| matches!(demand,
                omega_typer::StaticValueDemand::Specialized { arguments, .. } if arguments == &expected.arguments
            )),
            "missing width request for Outer<{outer}>::Inner<{inner}>"
        );
    }
    let scopes = &pending.trees().static_request_scopes[&exact_request.node];
    spec_corpus::observe_handoff!(
        "collection-layout-and-indirect-call-selections/producer",
        assert_eq!(
            scopes
                .iter()
                .map(|(owner, arity)| (owner.path.as_str(), *arity))
                .collect::<Vec<_>>(),
            [("Outer", 1), ("Outer::Inner", 1)]
        )
    );
    let completed = run(&[receipt(1, 2, 3), receipt(2, 1, 3)]).unwrap();
    assert_eq!(completed.static_request_scopes[&exact_request.node], *scopes);
    let missing = run(&[receipt(1, 2, 3)]).unwrap_err();
    assert_eq!(missing.file, "nested.omg");
    assert_eq!(missing.message, "specialized static expression in Outer::Inner needs an admitted evaluation result");
    let zero = run(&[receipt(1, 2, 3), receipt(2, 1, 0)]).unwrap_err();
    assert_eq!(zero.file, "nested.omg");
    assert_eq!(zero.message, "integer width must be positive");
}

#[test]
fn checking_a_static_call_also_checks_the_static_calls_in_its_arguments() {
    let code = "machine inner(n:u64)->u64{n} machine outer<const N:u64>()->u64{N} data Word<const K:u64>{v:u<outer::<inner(K)>()>;}";
    let stage = check(&[("nested_index.omg", code)]).unwrap();
    for callee in ["inner", "outer"] {
        let calls: Vec<_> = stage
            .calls
            .iter()
            .filter(|call| call.machine.path == callee)
            .collect();
        assert_eq!(calls.len(), 1, "{callee}");
        assert_eq!(calls[0].use_site, omega_typer::CallUse::StaticArgument);
    }
    let impure = code.replace("inner(n:u64)->u64{n}", "inner(n:u64)->u64 suspends {n}");
    let error = check(&[("nested_index.omg", &impure)]).unwrap_err();
    assert_eq!(error.file, "nested_index.omg");
    assert_eq!(error.message, "fact or static argument requires an effect-free computation");
    assert_generic_refusal_within(&impure, &error, "outer::<inner(K)>()");
}

#[test]
fn an_enclosing_application_checks_constraints_reached_through_another_record() {
    let declarations = "data Inner<const N:u64>{v:u<N>;} data Alias<const M:u64> = Inner<M>; data Outer<const K:u64>{inner:Alias<K>;}";
    for (use_site, accepted) in [("Outer<1>", true), ("Outer<0>", false)] {
        let code = format!("machine caller(x:{use_site}){{}} {declarations}");
        let result = check(&[("nested.omg", &code)]);
        if accepted {
            result.unwrap();
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.file, "nested.omg");
            assert_eq!(error.message, "integer width must be positive");
        }
    }
}

#[test]
fn recursive_fields_with_the_same_arguments_check_the_width_once() {
    let decl = "data Link<'a,const N:u64>{next:@'a Link<'a,N>; value:u<N>;}";
    check(&[("link.omg", &format!("{decl} machine ok<'a>(x:Link<'a,1>){{}}"))]).unwrap();
    let error = check(&[("link.omg", &format!("{decl} machine bad<'a>(x:Link<'a,0>){{}}"))]).unwrap_err();
    assert_eq!(error.file, "link.omg");
    assert_eq!(error.message, "integer width must be positive");
}

#[test]
fn changing_recursive_widths_retain_the_next_application_for_verification() {
    let code = "data Link<'a,const N:u64>{next:@'a Link<'a,N+1>;value:u<N>;} machine caller<'a>(x:Link<'a,3>){}";
    let stage = check(&[("recursive.omg", code)]).unwrap();
    assert!(stage.obligations.iter().any(|proof|
        proof.site.span.slice(code.as_bytes()) == Some(b"Link<'a,3>") &&
        matches!(&proof.kind, omega_typer::ObligationKind::FormRecursiveApplication{declaration,active_arguments,arguments} if declaration.path=="Link" && active_arguments[1].to_string()=="3" && arguments[1].to_string()=="4")));
    let zero = "data Link<'a,const N:u64>{next:@'a Link<'a,N-1>;value:u<N>;} machine caller<'a>(x:Link<'a,1>){}";
    let error = check(&[("recursive.omg", zero)]).unwrap_err();
    assert_eq!(error.file, "recursive.omg");
    // The next application subtracts from zero in a u64 argument.
    assert_eq!(error.message, "static index -1 exceeds u64");
    let plain = "data Link<'a,T>{next:@'a Link<'a,@'a T>;} machine caller<'a>(x:Link<'a,u8>){}";
    let stage = check(&[("recursive.omg", plain)]).unwrap();
    assert!(
        !stage
            .obligations
            .iter()
            .any(|o| matches!(o.kind, omega_typer::ObligationKind::FormRecursiveApplication { .. }))
    );
}

#[test]
fn finite_alternatives_require_each_literal_to_fit_the_index_carrier() {
    let source = "trait Scan { machine go<Width:u8>() requires Width == 3 || Width == 300; }";
    let error = check(&[("width.omg", source)]).unwrap_err();
    assert_eq!(error.message, "expected u8, found 300");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"300".as_slice()));
}
