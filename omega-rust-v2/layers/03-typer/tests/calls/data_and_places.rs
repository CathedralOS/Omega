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
