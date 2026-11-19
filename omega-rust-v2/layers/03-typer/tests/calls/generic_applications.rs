#[test]
fn runtime_indices_at_calls_bind_the_actual_subject() {
    let code = "data B<K:u8>{} machine keep(n:u8,b:B<n>)->B<n>{b} machine use_it(x:u8,b:B<x>)->B<x>{keep(x,b)} machine fixed(b:B<8>)->B<8>{keep(8,b)}";
    check(&[("subjects.omg", code)]).unwrap();
    assert!(
        check(&[("subjects.omg", &code.replace("keep(8,b)", "keep(9,b)"))])
            .unwrap_err()
            .message
            .contains("no matching overload")
    );
}

#[test]
fn source_static_calls_keep_the_machine_identity_until_the_arguments_close() {
    let defs = "const base:u8=3; machine twice(n:u8)->u8 reaches none crashes none terminates {let factor:u8=2; let doubled=n*factor; doubled} machine width(n:u8)->u8{twice(n)+base} ";
    let prefix = "data B<const K:u8>{} ";
    let program = format!(
        "{prefix}{defs} machine buffer<const N:u8>()->B<width(N)>{{B::<width(N)>{{}}}} machine f(x:B<width(4)>)->B<11>{{x}} machine g()->B<11>{{buffer::<4>()}}"
    );
    let stage = check(&[("static.omg", &program)]).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "buffer" && c.result.to_string() == "game::B<11>")
    );
    let overflow = program.replace("buffer::<4>()", "buffer::<128>()");
    assert!(
        check(&[("static.omg", &overflow)])
            .unwrap_err()
            .message
            .contains("exceeds u8")
    );
    let wrong = program.replace("f(x:B<width(4)>)->B<11>", "f(x:B<width(4)>)->B<12>");
    assert!(
        check(&[("static.omg", &wrong)])
            .unwrap_err()
            .message
            .contains("expected")
    );
}

#[test]
fn specializing_an_index_checks_an_intermediate_even_when_the_final_value_fits() {
    let bad = "data B<const K:u8>{} machine value<const N:u8>()->B<(N+1)*0>{B::<(N+1)*0>{}} machine f(){_=value::<255>();}";
    assert!(
        check(&[("i.omg", bad)])
            .unwrap_err()
            .message
            .contains("256 exceeds u8")
    );
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
        assert_eq!(product.calls.last().unwrap().result.to_string(), format!("game::B<{expected}>"));
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
        ("machine id<T>(x:T)->T{x} machine f()->u8{id::<bool>(2u8)}", "conflicting type argument"),
        ("machine id<T>(x:T)->T{x} machine f(){_ = id(1);}", "cannot infer"),
        ("data Box<T>{} machine f(x:Box){}", "expects 1"),
        ("data A = B; data B = A;", "cyclic type alias"),
    ] {
        assert!(check(&[("a.omg", code)]).unwrap_err().message.contains(message), "{code}");
    }
}

#[test]
fn normalized_static_arguments_and_binder_occurrences() {
    let code = "data Buffer<const N:u8>{} data Eight=Buffer<4+4>; machine f(x:Eight)->Buffer<8>{x} machine forward<const N:u8>(x:Buffer<N>)->Buffer<N>{x} machine g(x:Buffer<8>)->Eight{forward::<8>(x)}";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert_eq!(stage.calls[0].result.to_string(), "game::Buffer<8>");
    for (s, msg) in [
        ("data B<const N:u8>{} machine f(x:B<256>){}", "does not land"),
        ("data B<const N:u8>{} machine f(x:B<8u16>){}", "does not land"),
        (
            "data Wrap<T> = T; machine f<T,U>(a:T,b:U){} machine f<A,B>(a:Wrap<B>,b:A){}",
            "duplicate typed operand signature",
        ),
    ] {
        let e = check(&[("a.omg", s)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn a_static_equation_recovers_the_extent_and_refuses_conflicting_indices() {
    let definition =
        "domain<const N:u64> u64::AtMost<N> requires self <= N; data Buffer<L,const N:u64> requires L == u64::AtMost::<N> { bytes:[u8;N]; length:L; }";
    let code = format!("{definition} machine bytes(x:Buffer<u64::AtMost<4>>)->[u8;4]{{x.bytes}}");
    check(&[("indices.omg", &code)]).unwrap();
    let code = format!("{definition} machine bad(x:Buffer<u64::AtMost<4>,8>){{}}");
    assert!(
        check(&[("indices.omg", &code)])
            .unwrap_err()
            .message
            .contains("conflicting type equation")
    );
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
    assert!(
        check(&[("width.omg", "machine wrong(x:u<13u8>){}")])
            .unwrap_err()
            .message
            .contains("static u64")
    );
    let zero = "machine zero<const N:u64>()->u<N>{0} machine f(){ _ = zero::<0>(); }";
    let e = check(&[("width.omg", zero)]).unwrap_err();
    assert!(e.message.contains("width must be positive"), "{e:?}");
}

#[test]
fn evaluated_indices_check_the_source_type_and_key_only_on_the_value() {
    use omega_resolver::{ParsedSource, StageSource, resolve_stage};
    use omega_typer::{StaticIntegerRequest, type_stage_with_evaluator};
    let run = |code: &str, result: i64| {
        let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
        let sources = [StageSource {
            package: "game",
            source: ParsedSource {
                logical_path: "calc.omg",
                text: code,
                tokens: &tokens,
                tree: &tree,
            },
        }];
        let resolved = resolve_stage(&sources, &[]).unwrap();
        type_stage_with_evaluator(&sources, &resolved, None, &|q: &StaticIntegerRequest| {
            assert_eq!(q.carrier.to_string(), "u64");
            Ok(result.into())
        })
    };
    let head = "data Buf<const N:u64>{array:[u8;N];} ";
    let source = format!("{head} const size:u64=8; machine f(b:Buf<size>)->Buf<8>{{b}}");
    assert_eq!(check(&[("calc.omg", &source)]).unwrap().evaluated_indices.len(), 1);
    let stage = run(&source, 8).unwrap();
    assert_eq!(stage.evaluated_indices.len(), 1);
    assert_eq!(stage.signatures[0].parameters[0].1, stage.signatures[0].result);
    assert!(run(&source, 7).unwrap_err().message.contains("expected"));
    assert!(run(&source, -1).unwrap_err().message.contains("does not land"));
    let bad = format!("{head} const flag:bool=true; machine f(b:Buf<flag>)->Buf<8>{{b}}");
    assert!(
        run(&bad, 8)
            .unwrap_err()
            .message
            .contains("expected u64, found bool")
    );
}

#[test]
fn long_chains_of_transparent_aliases_do_not_consume_the_host_call_stack() {
    let mut code = String::from("data A0 = bool;\n");
    for index in 1..1024 {
        code.push_str(&format!("data A{index} = A{};\n", index - 1));
    }
    code.push_str("machine f(x:A1023)->bool{x}");
    let stage = check(&[("chain.omg", &code)]).unwrap();
    assert_eq!(stage.signatures[0].parameters[0].1, ValueType::Bool);
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
    assert!(err.message.contains("exceeds u8"), "{err:?}");
}

#[test]
fn canonical_open_index_signatures_cannot_redeclare_the_same_overload() {
    let code = "data B<const N:u64>{} machine select<const X:u64>(b:B<X+1>){} machine select<const Y:u64>(b:B<1+Y>){}";
    assert!(
        check(&[("duplicate.omg", code)])
            .unwrap_err()
            .message
            .contains("duplicate typed operand signature")
    );
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
    let source = format!("{prefix}
        machine inferred(b:B<2>,x:u8 in Label<2>)->u8 in Label<2>{{tag(b,x)}}
        machine explicit(b:B<2>,x:u8 in Label<2>)->u8 in Label<2>{{tag::<2>(b,x)}}");
    let stage = check(&[("index.omg", &source)]).unwrap();
    let [inferred, explicit] = &stage.calls[..] else { panic!("expected both tag calls"); };
    assert_eq!(inferred.machine, explicit.machine);
    assert_eq!(inferred.type_arguments, explicit.type_arguments);
    assert_eq!(inferred.result, explicit.result);
    let wrong = format!("{prefix} machine wrong(b:B<2>,x:u8 in Label<2>)->u8 in Label<3>{{tag(b,x)}}");
    assert!(check(&[("index.omg", &wrong)]).unwrap_err().message.contains("no matching overload"));
}

#[test]
fn result_domains_retain_the_actual_runtime_index() {
    let prefix = "domain<K:u8> u8::Label<K>; machine tag(n:u8,x:u8 in Label<n>)->u8 in Label<n>{x}";
    let source = format!("{prefix}
        machine forward(k:u8,x:u8 in Label<k>)->u8 in Label<k>{{tag(k,x)}}
        machine fixed(x:u8 in Label<2>)->u8 in Label<2>{{tag(2,x)}}");
    let stage = check(&[("runtime.omg",&source)]).unwrap();
    for call in &stage.calls {
        let caller=call.caller.as_ref().unwrap();
        let signature=stage.signatures.iter().find(|signature| &signature.machine==caller).unwrap();
        assert_eq!(call.result,signature.result);
    }
    let wrong = format!("{prefix} machine wrong(k:u8,m:u8,x:u8 in Label<k>)->u8 in Label<m>{{tag(k,x)}}");
    let error = check(&[("runtime.omg",&wrong)]).unwrap_err();
    assert!(error.message.contains("no matching overload for tag"));
}

#[test]
fn runtime_domain_sets_normalize_when_call_arguments_coincide_or_change_order() {
    let prefix = "domain<K:u8> u8::Label<K>;
        machine both(a:u8,b:u8,x:u8 in Label<a> & Label<b>)->u8 in Label<a> & Label<b>{x}";
    let source = format!("{prefix}
        machine same(k:u8,x:u8 in Label<k>)->u8 in Label<k>{{both(k,k,x)}}
        machine swapped(k:u8,m:u8,x:u8 in Label<k> & Label<m>)->u8 in Label<k> & Label<m>{{both(m,k,x)}}");
    let stage=check(&[("sets.omg",&source)]).unwrap();
    let [same,swapped]=&stage.calls[..] else {panic!("expected both calls")};
    assert_eq!(same.result.domains().len(),1);
    assert_eq!(swapped.result.domains().len(),2);
    let wrong=format!("{prefix} machine drop_one(k:u8,m:u8,x:u8 in Label<k> & Label<m>)->u8 in Label<k>{{both(m,k,x)}}");
    assert!(check(&[("sets.omg",&wrong)]).unwrap_err().message.contains("no matching overload for both"));
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
    let wrong = "machine substitute(x:u8 in Five)->u8 in Six {x}";
    let error = check(&[("domains.omg", defs), ("use.omg", wrong)]).unwrap_err();
    assert_eq!(error.file, "use.omg");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"x".as_slice()));
    assert!(error.message.contains("Bound<6>") && error.message.contains("Bound<5>"), "{error:?}");
    for (target, message) in [
        ("u8::Bound", "incorrect indexed domain arity"),
        ("u8::Bound<256>", "does not land in u8"),
    ] {
        let invalid = format!("domain u8::Invalid = {target};");
        let error = check(&[("invalid.omg", &invalid), ("domains.omg", defs)]).unwrap_err();
        assert_eq!(error.file, "invalid.omg");
        assert!(error.message.contains(message), "{error:?}");
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
        assert!(check(&[("types.omg", defs), ("use.omg", &different)]).is_err());
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
