mod support;
use omega_typer::ValueType;
use support::check;

#[test]
fn selects_by_operand_and_checks_the_selected_result() {
    let declarations = ("draw.omg", include_str!("../fixtures/draw.omg"));
    let caller = ("render.omg", include_str!("../fixtures/render.omg"));
    let stage = check(&[caller, declarations]).unwrap();
    assert_eq!(stage, check(&[declarations, caller]).unwrap());
    assert_eq!(stage.calls.len(), 1);
    assert_eq!(stage.calls[0].machine.operands, ["bool"]);
    assert_eq!(stage.calls[0].result, ValueType::Bool);
    let wrong = include_str!("../fixtures/wrong_result.omg");
    let err = check(&[declarations, ("wrong.omg", wrong)]).unwrap_err();
    assert_eq!(err.span.slice(wrong.as_bytes()).unwrap(), b"draw(flag)");
    assert!(err.message.contains("expected u32, found bool"), "{err:?}");
}

#[test]
fn follows_nominal_field_types_through_nested_calls_and_shadowing() {
    let code = "data Flag { value: bool; } machine take(x: bool) -> bool { x } machine f(x: Flag) -> bool { let x = x.value; take(take(x)) }";
    let stage = check(&[("main.omg", code)]).unwrap();
    assert_eq!(stage.calls.len(), 2);
    assert!(stage.calls.iter().all(|c| c.result == ValueType::Bool));
    let err = check(&[("main.omg", "data A {} data B {} machine take(x: A) {} machine f(x: B) { take(x) }")]).unwrap_err();
    assert!(err.message.contains("no matching overload"));
}

#[test]
fn integers_land_at_the_destination_and_suffixes_keep_their_carrier() {
    let stage = check(&[("a.omg", "machine f() -> u8 { 255 }")]).unwrap();
    assert_eq!(stage.landings.len(), 1);
    for code in [
        "machine f() -> u8 { 256 }",
        "machine f() -> i1 { 1 }",
        "machine f() -> u32 { 1u8 }",
        "machine f() -> u8 { 256u8 }",
        "machine f(x: u0) {}",
    ] {
        assert!(check(&[("a.omg", code)]).is_err(), "{code}");
    }
    check(&[("a.omg", "machine f() -> u8 { 0xffu8 }")]).unwrap();
}

#[test]
fn refuses_wrong_arity_ambiguous_calls_and_unsupported_forms() {
    for (code, message) in [
        ("machine take(x: bool) {} machine f() { take() }", "no matching overload"),
        ("machine take(x: u8) {} machine take(x: u16) {} machine f() { take(1) }", "ambiguous"),
        ("machine f() -> u32 { true }", "expected u32"),
        ("machine f() -> u32 {}", "missing result"),
        ("machine f(x: u32) -> u32 { x + true }", "no matching operator"),
        ("machine f<T>(x: T) -> bool { x }", "expected bool"),
    ] {
        let err = check(&[("a.omg", code)]).unwrap_err();
        assert!(err.message.contains(message), "{err:?}");
    }
}

#[test]
fn range_is_checked_after_call_selection() {
    let err = check(&[("a.omg", "machine f(n: u8) {} machine g() { f(256) }")]).unwrap_err();
    assert!(err.message.contains("expected u8, found 256"));
    let err = check(&[("a.omg", "machine f(x: u8) {} machine f(x: u16) {} machine g() { f(256) }")]).unwrap_err();
    assert!(err.message.contains("ambiguous"));
    let stage = check(&[("a.omg", "machine f(x: u8) {} machine f(x: u16) {} machine g() { f(7u16) }")]).unwrap();
    assert_eq!(stage.calls[0].machine.operands, ["u16"]);
}

#[test]
fn identical_normalized_operand_types_reject_without_a_call() {
    let err = check(&[("a.omg", "machine f(x: u8) {} machine f(y: u08) {}")]).unwrap_err();
    assert!(err.message.contains("duplicate typed operand signature"));
}

#[test]
fn long_field_chain_reports_the_first_invalid_receiver() {
    let source = format!("data Flag {{ value: bool; }} machine f(x: Flag) -> bool {{ x{} }}", ".value".repeat(4096));
    let err = check(&[("a.omg", &source)]).unwrap_err();
    assert_eq!(err.span.slice(source.as_bytes()).unwrap(), b"x.value.value");
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
fn typed_collections_records_and_mutability() {
    let code = "data Point { x: u8; } machine f(p:Point)->[u8;2] { let mut q = Point {x:p.x}; q.x = 7; let r = @q; [q.x,r.x] }";
    check(&[("a.omg", code)]).unwrap();
    for (code, message) in [
        ("machine f() { let x: u8 = 1; _ = @mut x; }", "writable authority"),
        ("data P { a:bool; } machine f()->P{P{}}", "missing field a"),
        ("data P { a:bool; } machine f()->P{P{a:true,a:false}}", "duplicate field a"),
        ("machine f()->[u8;2]{[1u8,2u32]}", "expected u8"),
        ("machine f()->[bool;1]{[true,false]}", "expected [bool; 1]"),
    ] {
        let err = check(&[("a.omg", code)]).unwrap_err();
        assert!(err.message.contains(message), "{err:?}");
    }
}

#[test]
fn arbitrary_precision_and_fractional_intermediates() {
    let stage = check(&[(
        "a.omg",
        "machine f() -> u257 {340282366920938463463374607431768211456} machine g()->u8{((5/2)*2) as u8} machine d()->f32{1.25e-1} machine min()->i8 {-128}",
    )])
    .unwrap();
    assert_eq!(stage.landings.len(), 4);
    for (expr, reason) in [
        ("(5/2)*2", "fractional intermediate"),
        ("(5/2) as u8", "found 5/2"),
        ("1/0", "division by zero"),
        ("1%2", "already typed"),
        ("-1", "expected u8"),
        ("256+1", "found 257"),
    ] {
        let code = format!("machine f()->u8{{ {expr} }}");
        assert!(check(&[("a.omg", &code)]).unwrap_err().message.contains(reason), "{expr}");
    }
}

#[test]
fn result_domains_and_contracts() {
    let code = "domain u8::Small requires self < 8; machine make()->u8 {9} machine make()->u8 in Small {1 as +Small} machine bare()->u8{make()} machine selected()->u8 in Small {make()}";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert!(stage.calls[0].machine.result_domains.is_empty());
    assert!(!stage.calls[1].machine.result_domains.is_empty());
    for (source, msg) in [
        ("domain u8::A; domain u8::B; machine f(x:u8 in A & B)->u8 in A { x as =A }", "cast omits domain"),
        ("domain u8::A; machine f(x:u8 in A)->u8 in A {x as +A}", "invalid +"),
        ("domain u8::A; machine f()->u8 in A {3}", "expected u8 in"),
        ("machine f(n:u8) requires n { }", "expected bool"),
        ("machine f(n:u8)->u8 ensures result {n}", "expected bool"),
    ] {
        let e = check(&[("a.omg", source)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn dispatch_checks_every_arm_and_preserves_case_field_types() {
    let source = "data Reply<T>{case Ok(value:T); case Empty;} machine read(x:Reply<u8>)->u8 {match x {Reply::Ok{value} -> value, Reply::Empty -> 0,}} machine f(b:bool)->bool {transition b { true -> false, false -> true, }}";
    check(&[("a.omg", source)]).unwrap();
    for (s, msg) in [
        ("machine f(b:bool)->u8 {match b {true -> 1,}}", "does not cover"),
        ("machine f(b:bool)->u8 {match b {true -> 1, false -> true,}}", "expected u8"),
        ("data S{case A;case B;} machine f(x:S)->bool{match x {S::A->true,}}", "does not cover"),
        ("data S{case A(a:bool);case B;} machine f()->S{S::A}", "requires its payload"),
    ] {
        let e = check(&[("a.omg", s)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn selected_operation_and_conformance_rows_retain_machine_identity() {
    let code = "domain u8::Distance; machine + add(a:u8 in Distance,b:u8 in Distance)->u8 in Distance { ((a as -Distance) +% (b as -Distance)) as +Distance } machine sum(a:u8 in Distance,b:u8 in Distance)->u8 in Distance{ a+b } trait Logger<T>{machine write(x:T);} machine log(x:u8){} Sink satisfies Logger<u8>{ Logger::write = log; }";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert_eq!(stage.calls[0].machine.path, "add");
    let row = &stage.conformance_rows[0];
    assert_eq!(row.requirement.path, "Logger::write");
    assert_eq!(row.implementation.path, "log");
    for (code, message) in [
        ("trait T{machine f(x:u8);} C satisfies T{}", "missing conformance row"),
        ("trait T{machine f(x:u8);} machine g(x:u32){} C satisfies T{T::f=g;}", "no matching signature"),
        ("machine + add(a:u8,b:u8)->u8{a}", "compiler-owned primitive"),
    ] {
        let e = check(&[("a.omg", code)]).unwrap_err();
        assert!(e.message.contains(message), "{e:?}");
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
fn borrow_authority_and_lifetime() {
    for (code, message) in [
        ("machine f()->@u8 {let x:u8=1; @x}", "invocation-local"),
        ("machine f()->@u8 {let x:u8=1; let view=@x; view}", "invocation-local"),
        ("data P{x:u8;} machine f(p:@P){let mut q=p; q.x=1;}", "writable authority"),
        ("machine f(x:@mut u8){} machine g(x:u8){f(x)}", "no matching overload"),
    ] {
        let e = check(&[("a.omg", code)]).unwrap_err();
        assert!(e.message.contains(message), "{e:?}");
    }
    check(&[(
        "a.omg",
        "machine read(x:@u8){} machine f(x:u8){read(x)} machine forward<'a>(x:@'a u8)->@'a u8{x}",
    )])
    .unwrap();
}

#[test]
fn inferred_pause_and_argument_domains() {
    let code = "machine wait() blocks {} machine wrap(){block wait()} machine entry(){block wrap()}";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert!(stage.machine_effects.iter().all(|m| m.pause.block));
    for (code, message) in [
        (
            "machine wait() blocks {} machine wrap(){block wait()} machine entry(){wrap()}",
            "block acknowledgement",
        ),
        ("machine f(x:u8)->u8{x} machine g()->u8{f(suspend f(0))}", "suspension position"),
        ("machine wait() blocks {} public machine f(){block wait()}", "blocking ceiling"),
    ] {
        let e = check(&[("a.omg", code)]).unwrap_err();
        assert!(e.message.contains(message), "{e:?}");
    }
    let stage = check(&[(
        "a.omg",
        "domain u8::D; machine make()->u8{1} machine make()->u8 in D{1 as +D} machine take(x:u8 in D){} machine f(){take(make())}",
    )])
    .unwrap();
    assert!(!stage.calls[0].machine.result_domains.is_empty());
}

#[test]
fn integer_landing_matches_independent_endpoint_oracle() {
    for width in [1, 2, 7, 8, 13, 24, 31, 63, 64, 104] {
        for signed in [false, true] {
            let limit = 1i128 << (width - i32::from(signed));
            let low = if signed { -limit } else { 0 };
            let high = limit - 1;
            for value in [low - 1, low, 0, high, high + 1] {
                let carrier = if signed { "i" } else { "u" };
                let source = format!("machine test()->{carrier}{width}{{ {value} }}");
                assert_eq!(check(&[("oracle.omg", &source)]).is_ok(), (low..=high).contains(&value), "{source}");
            }
        }
    }
}

#[test]
fn long_binary_chain_types_in_source_order_without_recursive_descent() {
    let source = format!("machine f(x:u8)->u8 {{ x{} }}", " +% 1".repeat(4096));
    let stage = check(&[("deep.omg", &source)]).unwrap();
    assert_eq!(stage.operations.len(), 4096);
    assert!(
        stage
            .operations
            .windows(2)
            .all(|pair| pair[0].site.span.get_end_offset() < pair[1].site.span.get_end_offset())
    );
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
fn existential_records_keep_witnesses_private_to_the_subject() {
    let source = "data Pair<some S>{a:S;b:S;} machine make(x:u8)->Pair{Pair{a:x,b:x}} machine compare<T>(a:T,b:T)->bool{true} machine ok(p:Pair)->bool{compare(p.a,p.b)}";
    let stage = check(&[("a.omg", source)]).unwrap();
    assert_eq!(stage.packages.len(), 1);
    for (tail, msg) in [
        ("machine mismatch()->Pair{Pair{a:true,b:1u8}}", "inconsistent package witness"),
        (
            "machine compare<T>(a:T,b:T){} machine mismatch(p:Pair,q:Pair){compare(p.a,q.a)}",
            "no matching overload",
        ),
    ] {
        let code = format!("data Pair<some S>{{a:S;b:S;}} {tail}");
        let e = check(&[("a.omg", &code)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn core_bracket_contracts_retain_endpoints_access_and_origin() {
    let source = "machine f(view:@mut [u8])->u8{let tail = @mut view[2..]; tail[0]=7; view[1]}";
    let stage = check(&[("a.omg", source)]).unwrap();
    assert_eq!(
        stage
            .collection_accesses
            .iter()
            .map(|a| a.machine.path.as_str())
            .collect::<Vec<_>>(),
        ["indexing::range_mut", "indexing::element_mut", "indexing::read"]
    );
    assert!(matches!(
        &stage.collection_accesses[0].selectors,
        omega_typer::CollectionSelectors::Range {
            start: Some(_),
            end: None,
            inclusive: false
        }
    ));
    for access in &stage.collection_accesses {
        assert!(
            omega_typer::collection_contracts()
                .signatures
                .iter()
                .any(|s| s.machine == access.machine)
        );
    }
    for (code, msg) in [
        ("machine f(s:@[u8]){ _ = @mut s[..]; }", "mutable authority"),
        ("data R{} machine f(s:@[R])->R{s[0]}", "copy element"),
        ("machine f(s:@mut [u8]){let r:@mut [u8]=s[..];}", "expected @mut"),
    ] {
        let e = check(&[("a.omg", code)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn forwarded_nominal_callback_publishes_its_originating_slot() {
    let source = "trait Step<T>{machine apply(v:T);} machine via<T,const F:Step<T>::apply>(v:T){F(v)} machine wrap<T,const G:machine>(v:T){via::<T,G>(v)}";
    let stage = check(&[("callbacks.omg", source)]).unwrap();
    assert_eq!(stage.callable_bounds.len(), 2);
    assert!(stage.callable_bounds.iter().all(|(_, _, c)| {
        c.requirement
            .as_ref()
            .is_some_and(|r| r.declaration.path == "Step::apply")
    }));
    let code = format!("{source} machine fake(v:bool){{}} machine bad(){{wrap::<bool,fake>(true)}}");
    let e = check(&[("callbacks.omg", &code)]).unwrap_err();
    assert!(e.message.contains("satisfaction edge"), "{e:?}");
}
#[test]
fn target_instruction_operands_are_checked_against_the_admitted_target() {
    use omega_resolver::{AssemblyVocabulary, ParsedSource, StageSource, resolve_stage_for_target};
    use omega_typer::{AssemblyTypes, type_stage_for_target};
    let code = "asm machine copy(n:u8) registers r0=n clobbers none { mov r0, 7; }";
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "game",
        source: ParsedSource {
            logical_path: "asm.omg",
            text: code,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let resolved = resolve_stage_for_target(
        &sources,
        &[],
        &AssemblyVocabulary {
            instructions: &["mov"],
            registers: &["r0"],
        },
    )
    .unwrap();
    let byte = ValueType::Integer { width: Some(8), signed: false };
    let mut target = AssemblyTypes {
        target: "test8".into(),
        registers: [("r0".into(), byte.clone())].into(),
        instructions: [("mov".into(), vec![vec![byte.clone(), byte]])].into(),
    };
    let stage = type_stage_for_target(&sources, &resolved, &target).unwrap();
    assert_eq!(stage.instructions.len(), 1);
    assert_eq!(stage.instructions[0].target, "test8");
    target.instructions.get_mut("mov").unwrap()[0][1] = ValueType::Bool;
    assert!(
        type_stage_for_target(&sources, &resolved, &target)
            .unwrap_err()
            .message
            .contains("no matching instruction operands")
    );
}

#[test]
fn tuples_mathematical_prefixes_and_write_only_places() {
    check(&[(
        "a.omg",
        "machine f(x:bool,y:bool)->bool{match (x,y) {(true,_)->true,(false,true)->true,(false,false)->false,}}",
    )])
    .unwrap();
    check(&[("a.omg", "let first(x:u8,y:bool):u8=x; let partial(x:u8):bool->u8=first(x);")]).unwrap();
    check(&[("a.omg", "data P { v:u8; } machine set(p:@write P){p.v=7;}")]).unwrap();
    let e = check(&[("a.omg", "data P { v:u8; } machine get(p:@write P)->u8{p.v}")]).unwrap_err();
    assert!(e.message.contains("write-only"));
}
#[test]
fn explicit_evidence_binds_the_exact_trait_member() {
    let declarations = "trait Render<T>{machine paint(x:T)->bool;} Renderer satisfies Render<u8>{machine paint(x:u8)->bool{true}} machine use_map<T,const E:Render<T>>(x:T)->bool{E::paint(x)}";
    let code = format!("{declarations} machine f()->bool{{use_map::<u8,Renderer>(1)}}");
    let stage = check(&[("evidence.omg", &code)]).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "Render::paint" && c.evidence.is_some())
    );
    let bad = format!("{declarations} machine f()->bool{{use_map::<bool,Renderer>(true)}}");
    let e = check(&[("evidence.omg", &bad)]).unwrap_err();
    assert!(e.message.contains("wrong trait"), "{e:?}");
}
#[test]
fn inherited_rows_use_the_declaring_traits_applied_parameters() {
    let traits = "trait Parent<T>{machine get(x:T)->T;} trait Child<U>:Parent<U>{machine flag()->bool;}";
    let impls = "Impl satisfies Child<u8>{machine get(x:u8)->u8{x} machine flag()->bool{true}}";
    let use_it = "machine call<T,const E:Child<T>>(x:T)->T{E::get(x)}";
    let stage = check(&[("a.omg", &format!("{traits} {impls} {use_it}"))]).unwrap();
    assert!(
        stage
            .conformance_rows
            .iter()
            .any(|r| r.requirement.path == "Parent::get" && r.trait_arguments[0].to_string() == "u8")
    );
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "Parent::get" && c.evidence.is_some())
    );
    let e = check(&[("a.omg", &format!("{traits} Impl satisfies Child<u8>{{machine flag()->bool{{true}}}}"))]).unwrap_err();
    assert!(e.message.contains("missing conformance row for Parent::get"), "{e:?}");
}

#[test]
fn trait_token_requires_one_applicable_evidence_binder() {
    let head = "trait Add<T>{machine + combine(x:T,y:T)->T;} data P {v:bool;}";
    let ok = format!("{head} machine sum<T,const E:Add<T>>(x:T,y:T)->T{{x+y}}");
    let stage = check(&[("a.omg", &ok)]).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "Add::combine" && c.evidence.is_some())
    );
    let ambiguous = format!("{head} machine sum<T,const A:Add<T>,const B:Add<T>>(x:T,y:T)->T{{x+y}}");
    let e = check(&[("a.omg", &ambiguous)]).unwrap_err();
    assert!(e.message.contains("ambiguous operator"), "{e:?}");
    let no_binder = format!("{head} Impl satisfies Add<P>{{machine combine(x:P,y:P)->P{{x}}}} machine sum(x:P,y:P)->P{{x+y}}");
    assert!(check(&[("a.omg", &no_binder)]).is_err());
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
fn projecting_owned_storage_and_borrowing_metadata_preserves_place_authority() {
    for code in [
        "data P{n:u8;} machine f(p:P)->@u8{@p.n}",
        "machine f(p:[u8;4])->@u8{@p[0]}",
        "machine f(v:@mut [u8]){ _ = @mut v.len; }",
    ] {
        assert!(check(&[("borrow.omg", code)]).is_err(), "{code}");
    }
    check(&[("borrow.omg", "data P{n:u8;} machine f(p:@P)->@u8{@p.n}")]).unwrap();
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
    assert!(
        check(&[("calc.omg", &source)])
            .unwrap_err()
            .message
            .contains("requires evaluation")
    );
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
fn cleanup_is_an_exact_owned_call_and_conflicting_domain_plans_reject() {
    let source = "data R dropped by end {v:u8;} machine end(r:R){} domain R::Live dropped by end; machine read(r:R in Live){}";
    let stage = check(&[("resource.omg", source)]).unwrap();
    assert_eq!(stage.cleanups.len(), 2);
    let conflicting = source.replace("::Live dropped by end", "::Live dropped by other") + " machine other(r:R){}";
    assert!(
        check(&[("resource.omg", &conflicting)])
            .unwrap_err()
            .message
            .contains("conflicting cleanup")
    );
    let wrong = source.replace("end(r:R)", "end(r:@R)");
    assert!(
        check(&[("resource.omg", &wrong)])
            .unwrap_err()
            .message
            .contains("owned carrier")
    );
}
#[test]
fn checks_satisfaction_even_when_the_machine_is_never_called() {
    let head = "trait Op<T>{machine act(x:T)->T;}";
    check(&[("sat.omg", &format!("{head} machine fine(x:u8)->u8 satisfies Op<u8>::act {{x}}"))]).unwrap();
    let bad = format!("{head} machine wrong(x:bool)->bool satisfies Op<u8>::act {{x}}");
    assert!(
        check(&[("sat.omg", &bad)])
            .unwrap_err()
            .message
            .contains("incompatible requirement signature")
    );
}
#[test]
fn crash_causes_and_checked_arithmetic_keep_their_proof_sites() {
    let source = "machine risky(a:u8,b:u8)->u8 crashes Trap{a+!b} public machine use_it(a:u8)->u8{risky(a,1)}";
    let stage = check(&[("effects.omg", source)]).unwrap();
    assert!(
        stage
            .machine_effects
            .iter()
            .any(|e| e.machine.path == "risky" && e.crash_ceiling == Some(vec![omega_typer::CrashCause::Trap]))
    );
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,omega_typer::ObligationKind::CrashCoverage{machine,cause:omega_typer::CrashCause::Trap} if machine.path=="use_it"))
    );
    let conflicting = "machine wrong() crashes none crashes Abort{}";
    assert!(
        check(&[("effects.omg", conflicting)])
            .unwrap_err()
            .message
            .contains("conflicts")
    );
}
#[test]
fn arm_membership_and_borrow_provenance_are_scoped_to_the_taken_path() {
    let source = "data R{case Ok(v:u8); case Empty;} machine f(x:R)->u8{match x {R::Ok{..}->x.v,R::Empty->0,}}";
    check(&[("arms.omg", source)]).unwrap();
    assert!(
        check(&[("arms.omg", &source.replace("Empty->0", "Empty->x.v"))])
            .unwrap_err()
            .message
            .contains("no field")
    );
    let escaped = "machine f(x:u8,y:@u8,b:bool)->@u8{match b{true->@x,false->y,}}";
    assert!(
        check(&[("arms.omg", escaped)])
            .unwrap_err()
            .message
            .contains("invocation-local")
    );
}
#[test]
fn transparent_restrictions_check_the_selected_base_rows() {
    let base = "trait Log{machine put() blocks;} trait Local=Log{machine * blocks false reaches none terminates;} Quiet satisfies Log{machine put(){}} Busy satisfies Log{machine put() blocks {}} machine use_it<const E:Local>(){E::put()}";
    let fine = format!("{base} machine f(){{use_it::<Quiet>()}}");
    let stage = check(&[("refine.omg", &fine)]).unwrap();
    assert!(stage.refinements[0].terminates);
    let e = check(&[("refine.omg", &fine.replace("::<Quiet>", "::<Busy>"))]).unwrap_err();
    assert!(e.message.contains("exceeds the refined contract"), "{e:?}");
    let false_target = "trait Base{} trait Refined=Base{machine * reaches none;} Wrong satisfies Refined{}";
    assert!(
        check(&[("refine.omg", false_target)])
            .unwrap_err()
            .message
            .contains("conformance target")
    );
}
#[test]
fn authored_brackets_borrow_only_the_collection_operand() {
    let head = "data B{n:u8;} machine [] at(b:@mut B,i:u64)->@mut u8{@mut b.n} machine [..] between(b:@B,start:u64,end:u64)->u8{b.n}";
    let stage = check(&[("custom.omg", &format!("{head} machine f(v:B)->u8{{let mut b=v; b[0]=1; b[0..2]}}"))]).unwrap();
    assert!(stage.calls.iter().any(|c| c.machine.path == "at") && stage.calls.iter().any(|c| c.machine.path == "between"));
    let bad = format!("{head} machine f(b:B){{b[0]=1;}}");
    assert!(
        check(&[("custom.omg", &bad)])
            .unwrap_err()
            .message
            .contains("no admitted")
    );
    let ambiguous = "data B{n:u8;} machine [] one(b:B,i:u8)->u8{b.n} machine [] two(b:@B,i:u8)->u8{b.n} machine f(b:B)->u8{b[1]}";
    assert!(
        check(&[("custom.omg", ambiguous)])
            .unwrap_err()
            .message
            .contains("ambiguous operator")
    );
}
#[test]
fn a_runtime_callback_carries_its_reach_into_the_calling_machine() {
    let prefix = "boundary trait Sensor{} ";
    let accepted = format!("{prefix} machine run(f:machine() reaches Sensor crashes none terminates) reaches Sensor {{f()}}");
    let stage = check(&[("effects.omg", &accepted)]).unwrap();
    assert!(stage.indirect_calls[0].contract.terminates);
    let refused = format!("{prefix} machine run(f:machine() reaches Sensor crashes none) reaches none {{f()}}");
    assert!(
        check(&[("effects.omg", &refused)])
            .unwrap_err()
            .message
            .contains("callback exceeds")
    );
}
#[test]
fn generic_rows_keep_binder_kinds_even_if_value_signatures_are_identical() {
    let head = "trait Trait{machine f<T,const N:u64>(v:T)->T;}";
    check(&[("binders.omg", &format!("{head} Good satisfies Trait{{machine f<U,const X:u64>(v:U)->U{{v}}}}"))]).unwrap();
    let bad = format!("{head} Bad satisfies Trait{{machine f<U,V>(v:U)->U{{v}}}}");
    assert!(
        check(&[("binders.omg", &bad)])
            .unwrap_err()
            .message
            .contains("no matching signature")
    );
}
#[test]
fn permuting_files_preserves_the_whole_typed_stage() {
    let a = ("alpha.omg", "data Byte=u8; trait R<T>{machine read(x:T)->T;}");
    let b = (
        "beta.omg",
        "Impl satisfies R<Byte>{machine read(x:Byte)->Byte{x}} machine get<T,const E:R<T>>(x:T)->T{E::read(x)}",
    );
    let c = ("gamma.omg", "machine f()->u8{get::<Byte,Impl>(1)}");
    let expected = check(&[a, b, c]).unwrap();
    for input in [[a, c, b], [b, a, c], [b, c, a], [c, b, a], [c, a, b]] {
        assert_eq!(check(&input).unwrap(), expected);
    }
}
#[test]
fn governed_domains_require_the_admitted_policy_requirement() {
    use omega_resolver::{ParsedSource, StageSource, resolve_stage};
    use omega_typer::{TypingEnvironment, type_stage_in_environment};
    let code = "data Schema{} data Plan{} trait AccessPolicy{machine plan(s:Schema)->Plan;} trait OtherPolicy{machine plan(s:Schema)->Plan;} Rule satisfies AccessPolicy{machine plan(s:Schema)->Plan terminates{Plan{}}} data C{} domain C::Locked access Rule;";
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "game",
        source: ParsedSource {
            logical_path: "policy.omg",
            text: code,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let resolved = resolve_stage(&sources, &[]).unwrap();
    let run = |name| {
        type_stage_in_environment(
            &sources,
            &resolved,
            &TypingEnvironment {
                access_plan_requirement: resolved.declarations.iter().find(|d| d.path == name),
                ..Default::default()
            },
        )
    };
    let stage = run("AccessPolicy::plan").unwrap();
    assert_eq!(stage.access_policies[0].plan_machine.path, "Rule::plan");
    assert!(
        run("OtherPolicy::plan")
            .unwrap_err()
            .message
            .contains("does not supply")
    );
    assert!(run("").unwrap_err().message.contains("requires the admitted"));
}
#[test]
fn admitted_ranking_views_check_the_subject_arguments_and_rank_bounds() {
    use omega_resolver::{ParsedSource, StageSource, resolve_stage};
    use omega_typer::{RankingView, TypingEnvironment, type_stage_in_environment};
    let run = |code: &str| {
        let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
        let inputs = [StageSource {
            package: "game",
            source: ParsedSource {
                logical_path: "rank.omg",
                text: code,
                tokens: &tokens,
                tree: &tree,
            },
        }];
        let resolved = resolve_stage(&inputs, &[]).unwrap();
        let byte = ValueType::Integer { signed: false, width: Some(8) };
        let views = [RankingView {
            declaration: resolved
                .declarations
                .iter()
                .find(|d| d.path == "Down")
                .unwrap()
                .clone(),
            subject: byte.clone(),
            parameters: vec![],
            rank: byte,
            canonical_default: true,
        }];
        type_stage_in_environment(
            &inputs,
            &resolved,
            &TypingEnvironment {
                ranking_views: &views,
                ..Default::default()
            },
        )
    };
    let source = "data Down{} machine f(x:u8) terminates by x -> Down in 0..=255 {} machine g(x:u8) terminates by x {}";
    let stage = run(source).unwrap();
    assert_eq!(stage.ranking_witnesses.len(), 2);
    assert_eq!(stage.ranking_witnesses[0].bounds.len(), 2);
    assert!(stage.ranking_witnesses[0].inclusive_upper);
    assert!(
        run(&source.replace("..=255", "..=256"))
            .unwrap_err()
            .message
            .contains("expected u8")
    );
    assert!(
        run(&source.replace("f(x:u8)", "f(x:bool)"))
            .unwrap_err()
            .message
            .contains("expected u8")
    );
}
#[test]
fn v1_crash_route_guards_type_as_total_facts_about_the_entry_value() {
    let source = "machine forward(value:i64)->i64 crashes Trap value == 0 {value}";
    let stage = check(&[("route.omg", source)]).unwrap();
    assert_eq!(stage.crash_routes[0].cause, omega_typer::CrashCause::Trap);
    assert_eq!(
        stage.crash_routes[0]
            .guard
            .as_ref()
            .unwrap()
            .span
            .slice(source.as_bytes())
            .unwrap(),
        b"value == 0"
    );
    for (guard, diagnostic) in [
        ("value", "expected bool"),
        ("value +! 1 == 0", "trapping arithmetic"),
    ] {
        assert!(
            check(&[("route.omg", &source.replace("value == 0", guard))])
                .unwrap_err()
                .message
                .contains(diagnostic)
        );
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
fn synchronous_invocation_distinguishes_two_callbacks_with_the_same_type() {
    let ty = "machine() reaches none crashes none";
    let head = format!("machine run(f:{ty}) invokes f {{f()}}");
    let source = format!("{head} machine wrap(left:{ty},right:{ty}) invokes right {{run(right)}}");
    let stage = check(&[("invokes.omg", &source)]).unwrap();
    let effect = stage
        .machine_effects
        .iter()
        .find(|e| e.machine.path == "wrap")
        .unwrap();
    assert!(matches!(
        effect.invokes.as_slice(),
        [omega_typer::InvocationTarget::RuntimeParameter { index: 1, .. }]
    ));
    let err = check(&[("invokes.omg", &source.replace("run(right)", "run(left)"))]).unwrap_err();
    assert!(err.message.contains("synchronous invocation ceiling"));
}

#[test]
fn mathematical_generic_arguments_and_chained_prefixes_keep_the_checked_term() {
    let declarations = "let first<T>(x:T, y:bool):T=x; let identity<T>(x:T):T=x;";
    for expression in [
        "first::<u8>(5)(true)",
        "first(5u8,true)",
        "identity::<u8>(5)",
        "identity(5)",
    ] {
        let source = format!("{declarations} let term():u8={expression};");
        let stage = check(&[("math.omg", &source)]).unwrap();
        assert!(stage.obligations.iter().any(|o| matches!(&o.kind,
            omega_typer::ObligationKind::MathematicalApplication {definition:Some(_), generic_arguments, ..}
            if generic_arguments.len()==1 && generic_arguments[0].to_string()=="u8")));
        assert!(stage.indirect_calls.is_empty());
    }
    let source = format!("{declarations} let term():u8->bool->u8=first::<u8>;");
    check(&[("math.omg", &source)]).unwrap();
    for (expression, message) in [
        ("first::<u8>(true,true)", "expected u8"),
        ("first::<u8,bool>(5,true)", "excess generic arguments"),
        ("first::<u8>(5,true,false)", "excess mathematical arguments"),
        ("first(5u8)(7u8)", "expected bool"),
    ] {
        let source = format!("{declarations} let term():u8={expression};");
        let err = check(&[("math.omg", &source)]).unwrap_err();
        assert!(err.message.contains(message), "{expression}: {err:?}");
    }
}
