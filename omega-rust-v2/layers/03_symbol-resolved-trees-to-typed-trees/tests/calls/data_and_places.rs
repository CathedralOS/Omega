#[test]
fn long_field_chain_reports_the_first_invalid_receiver() {
    let source = format!("data Flag {{ value: bool; }} machine f(x: Flag) -> bool {{ x{} }}", ".value".repeat(4096));
    let err = check(&[("a.omg", &source)]).unwrap_err();
    assert_eq!(err.span.slice(source.as_bytes()).unwrap(), b"x.value.value");
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
fn parentheses_preserve_field_authority_and_collection_write_selection() {
    let cell = "data Cell {value:u8;}";
    for place in ["view.value", "(view.value)", "((view.value))", "((view).value)"] {
        for action in [
            format!("{place}=7;"),
            format!("let ref_to_field=@mut {place};"),
            format!("let output=@write {place};"),
        ] {
            let shared = format!("{cell} machine f(input:@Cell) {{let mut view:@Cell=input; {action}}}");
            let error = check(&[("place.omg", &shared)]).unwrap_err();
            assert_eq!(error.message, "place requires writable authority");
            let exclusive = format!("{cell} machine f(view:@mut Cell) {{{action}}}");
            check(&[("place.omg", &exclusive)]).unwrap();
        }
        let write_only = format!("{cell} machine f(view:@write Cell) {{{place}=7;}}");
        check(&[("place.omg", &write_only)]).unwrap();
        let read = format!("{cell} machine f(view:@write Cell)->u8 {{{place}}}");
        assert_eq!(
            check(&[("place.omg", &read)]).unwrap_err().message,
            "cannot read through a write-only reference"
        );
    }
    for site in ["view[0]", "(view[0])", "((view[0]))"] {
        for target in [format!("{site}=7;"), format!("let r=@mut {site};")] {
            let source = format!("machine f(view:@mut [u8]) {{{target}}}");
            let stage = check(&[("index.omg", &source)]).unwrap();
            let [access] = stage.collection_accesses.as_slice() else {
                panic!("expected one mutable indexing selection")
            };
            assert_eq!(access.machine.path, "indexing::element_mut");
        }
    }
    for place in ["array.len", "((array.len))"] {
        let source = format!("machine f() {{let mut array:[u8;2]; {place}=3;}}");
        assert_eq!(check(&[("metadata.omg", &source)]).unwrap_err().message, "collection metadata is read-only");
    }
}

#[test]
fn nested_runtime_callables_require_both_effect_ceilings_at_their_own_type() {
    for context in [
        "data Slot {callback:CALLBACK;}",
        "machine f(cb:CALLBACK) {}",
        "machine f() {let mut cb:CALLBACK;}",
        "machine f(x:machine(x:u8)->u8 reaches none crashes none)->CALLBACK {x}",
        "machine f<'a>(cb:@'a CALLBACK) {}",
        "machine f(cb:[CALLBACK;2]) {}",
        "machine f(cb:@CALLBACK) {}",
        "data Box<T>{item:T;} machine f(cb:Box<CALLBACK>) {}",
        "machine f(cb:machine(g:CALLBACK) reaches none crashes none) {}",
        "machine f<const F:machine(g:CALLBACK)>() {}",
        "machine f<const F:machine()->CALLBACK>() {}",
    ] {
        let closed = context.replace("CALLBACK", "machine(x:u8)->u8 reaches none crashes none");
        check(&[("callback.omg", &closed)]).unwrap();
        for (clause, missing) in [
            (" reaches none", "crash"),
            (" crashes none", "reach"),
            ("", "reach"),
        ] {
            let callback = format!("machine(x:u8)->u8{clause}");
            let source = context.replace("CALLBACK", &callback);
            let error = check(&[("callback.omg", &source)]).unwrap_err();
            assert!(error.message.contains(missing), "{error:?}");
            assert_eq!(error.span.slice(source.as_bytes()), Some(callback.as_bytes()));
        }
    }
    check(&[("static.omg", "machine invoke<const F:machine(x:u8)->u8>(x:u8)->u8{F(x)}")]).unwrap();
}

#[test]
fn a_write_only_field_loan_cannot_be_strengthened_to_readable_access() {
    let defs = "data Cell {value:u8;}";
    for place in ["cell.value", "(cell.value)"] {
        let source = format!("{defs} machine observe(cell:@write Cell)->u8 {{let view=@mut {place}; view}}");
        let error = check(&[("loan.omg", &source)]).unwrap_err();
        assert_eq!(error.message, "cannot read through a write-only reference");
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"cell.value".as_slice()));
        let allowed = source.replace("cell:@write Cell", "cell:@mut Cell");
        check(&[("loan.omg", &allowed)]).unwrap();
        let forwarded = format!("{defs} machine forward(cell:@write Cell) {{let out=@write {place};}}");
        check(&[("loan.omg", &forwarded)]).unwrap();
    }
}

#[test]
fn composed_place_rewrites_preserve_the_loan_and_its_authority() {
    // A grouping can occur at the root, between selectors, or around the whole loan target.
    for root in ["view", "(view)"] {
        for receiver in [format!("{root}.inner"), format!("({root}.inner)")] {
            let field = format!("{receiver}.value");
            for place in [field.clone(), format!("(({field}))")] {
                for (prefix, access) in [
                    ("@mut", omega_typer::Access::Mut),
                    ("@write", omega_typer::Access::Write),
                ] {
                    let source = format!("data Cell{{value:u8;}} data Outer{{inner:Cell;}} machine f(view:@mut Outer){{let loan={prefix} {place};}}");
                    let stage = check(&[("rewrite.omg", &source)]).unwrap();
                    let loans: Vec<_> = stage
                        .obligations
                        .iter()
                        .filter_map(|obligation| match &obligation.kind {
                            omega_typer::ObligationKind::Borrow { access, place } => Some((access, place)),
                            _ => None,
                        })
                        .collect();
                    let [(actual, target)] = loans.as_slice() else {
                        panic!("expected one loan for {source}")
                    };
                    assert_eq!(**actual, access);
                    assert_eq!(target.span.slice(source.as_bytes()), Some(field.as_bytes()));
                    let shared = source.replace("view:@mut Outer", "view:@Outer");
                    assert_eq!(check(&[("rewrite.omg", &shared)]).unwrap_err().message, "place requires writable authority");
                }
            }
        }
    }
}

#[test]
fn composing_type_constructors_preserves_the_inner_callable_ceiling() {
    // Try each ordered pair, including a constructor nested inside itself.
    let containers = ["@TYPE", "[TYPE;2]", "[TYPE]", "Box<TYPE>"];
    for outer in containers {
        for inner in containers {
            let annotation = outer.replace("TYPE", inner);
            let program = format!("data Box<T>{{item:T;}} machine f(cb:{annotation}){{}}");
            for (suffix, diagnostic) in [
                (" reaches none crashes none", None),
                (" reaches none", Some("runtime callable requires an explicit crash ceiling")),
                (" crashes none", Some("runtime callable requires an explicit reach ceiling")),
                ("", Some("runtime callable requires explicit reach and crash ceilings")),
            ] {
                let callable = format!("machine(x:u8)->u8{suffix}");
                let source = program.replace("TYPE", &callable);
                let result = check(&[("rewrite.omg", &source)]);
                if let Some(message) = diagnostic {
                    let error = result.unwrap_err();
                    assert_eq!(error.message, message, "{source}");
                    assert_eq!(error.span.slice(source.as_bytes()), Some(callable.as_bytes()), "{source}");
                } else {
                    result.unwrap_or_else(|error| panic!("{source}: {error:?}"));
                }
            }
        }
    }
}
