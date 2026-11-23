#[test]
fn schema_numbers_are_exact_and_unique_among_live_and_retired_members() {
    let accepted = "data Packet {#0xffff_ffff_ffff_ffff word:u8; retired #0;}";
    let typed = check(&[("schema.omg", accepted)]).unwrap();
    let (_, scope) = typed
        .schema_numbering
        .iter()
        .find(|(id, _)| id.path == "Packet")
        .unwrap();
    assert_eq!(scope.members.len(), 1);
    assert_eq!(scope.members[0].number, u64::MAX);
    assert_eq!(scope.members[0].declaration_index, 0);
    assert_eq!(scope.retired, [0]);
    for (text, site, reason) in [
        ("data R {#7 a:u8; b:u8;}", "b:u8;", "every member"),
        ("data R {a:u8; retired #9;}", "a:u8;", "every member"),
        ("data R {#1 a:u8; #0x1 b:u8;}", "#0x1", "duplicate stable schema number #1"),
        ("data R {retired #1; #0b1 a:u8;}", "#0b1", "duplicate stable schema number #1"),
        ("data R {#1 a:u8; retired #0o1;}", "#0o1", "duplicate stable schema number #1"),
        ("data R {retired #0; retired #00;}", "#00", "duplicate stable schema number #0"),
        ("data S {#2 case A; #2 case B;}", "#2", "duplicate stable schema number #2"),
        ("data S {case A(#2 x:u8, #2 y:u8);}", "#2", "duplicate stable schema number #2"),
        ("data S {case A(retired #2, x:u8);}", "x:u8", "every member"),
        ("data R {#18446744073709551616 a:u8;}", "#18446744073709551616", "stable schema number must"),
        ("data R {#256u8 a:u8;}", "#256u8", "stable schema number must"),
    ] {
        let error = check(&[("schema.omg", text)]).unwrap_err();
        assert!(error.message.starts_with(reason), "{text}: {error:?}");
        assert_eq!(error.span.get_start_offset(), text.rfind(site).unwrap(), "{text}: {error:?}");
    }
}

#[test]
fn shorthand_checks_the_local_against_the_named_field() {
    let program = |parameter| format!("data Point {{ x: u8; }} machine point(x: {parameter}) -> Point {{ Point {{ x }} }}");
    let accepted = program("u8");
    check(&[("point.omg", &accepted)]).unwrap();
    let refused = program("bool");
    let error = check(&[("point.omg", &refused)]).unwrap_err();
    assert!(error.message.contains("expected u8"), "{error:?}");
    assert_eq!(error.span.slice(refused.as_bytes()), Some(&b"x"[..]));
}

#[test]
fn long_field_chain_reports_the_first_invalid_receiver() {
    let source = format!("data Flag {{ value: bool; }} machine f(x: Flag) -> bool {{ x{} }}", ".value".repeat(4096));
    let err = check(&[("a.omg", &source)]).unwrap_err();
    assert_eq!(err.span.slice(source.as_bytes()).unwrap(), b"x.value.value");
}

#[test]
fn a_local_named_mutable_acquires_write_access_only_with_the_marker() {
    let editable = "machine f(){let mutable mutable:u8=1; _ = @mutable mutable;}";
    check(&[("scope.omg", editable)]).unwrap();

    let fixed = "machine f(){let mutable:u8=1; _ = @mutable mutable;}";
    let error = check(&[("scope.omg", fixed)]).unwrap_err();
    assert!(error.message.contains("writable authority"), "{error:?}");
}

#[test]
fn typed_collections_records_and_mutability() {
    let code = "data Point { x: u8; } machine f(p:Point)->[u8;2] { let mutable q = Point {x:p.x}; q.x = 7; let r = @q; [q.x,r.x] }";
    check(&[("a.omg", code)]).unwrap();
    for (code, message) in [
        ("machine f() { let x: u8 = 1; _ = @mutable x; }", "writable authority"),
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
        ("data P{x:u8;} machine f(p:@P){let mutable q=p; q.x=1;}", "writable authority"),
        ("machine f(x:@mutable u8){} machine g(x:u8){f(x)}", "no matching overload"),
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
    let source = "data Pair<'a,some S>{a:@'a S;b:@'a S;} machine make<'a>(x:@'a u8)->Pair<'a>{Pair{a:x,b:x}} machine compare<T>(a:T,b:T)->bool{true} machine ok<'a>(p:Pair<'a>)->bool{compare(p.a,p.b)}";
    let stage = check(&[("a.omg", source)]).unwrap();
    assert_eq!(stage.packages.len(), 1);
    for (tail, msg) in [
        (
            "machine mismatch<'a>(x:@'a bool,y:@'a u8)->Pair<'a>{Pair{a:x,b:y}}",
            "inconsistent package witness",
        ),
        (
            "machine compare<T>(a:T,b:T){} machine mismatch<'a>(p:Pair<'a>,q:Pair<'a>){compare(p.a,q.a)}",
            "no matching overload",
        ),
    ] {
        let code = format!("data Pair<'a,some S>{{a:@'a S;b:@'a S;}} {tail}");
        let e = check(&[("a.omg", &code)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn destructuring_and_projection_open_the_same_package_subject() {
    let schema = "data Pair<'a,some S>{a:@'a S;b:@'a S;} data Wrap<'a>{p:Pair<'a>;} machine compare<T>(a:@T,b:@T)->bool{true}";
    for (body, accepted) in [
        ("let Pair{a,..} = p; compare(a,(p).b)", true),
        ("let Pair{a,..} = p; let Pair{b,..} = q; compare(a,b)", false),
        ("let Pair{a,..} = w.p; compare(a,(w.p).b)", true),
        ("let (Pair{a,..},Pair{b,..}) = (p,q); compare(a,b)", false),
        ("let (Pair{a,..},Pair{b,..}) = (p,p); compare(a,b)", true),
        ("let (Pair{a,..},) = w; compare(a,w.p.b)", true),
    ] {
        let code = format!("{schema} machine same<'a>(p:Pair<'a>,q:Pair<'a>,w:Wrap<'a>)->bool{{{body}}}");
        match check(&[("open.omg", &code)]) {
            Ok(_) => assert!(accepted, "{body}"),
            Err(error) => assert!(!accepted && error.message.contains("no matching overload"), "{body}: {error:?}"),
        }
    }
}

#[test]
fn packaged_fields_require_a_uniform_representation_at_the_declaration() {
    let wrappers = "data Cell<T>{item:T;} data View<'a,T>{ptr:@'a T;} data Phantom<T>{tag:u8;} data Choice<T>{case None;case Stored(item:T);} data Alias<T> = Cell<T>; data Link<'a,T>{next:@'a Link<'a,T>;item:@'a T;} data Unsized<T>{item:[T];} data Nested<T>{v:Cell<T>;}
        data TakeValue<T> = machine(x:T)->u8 reaches none crashes none;
        data MakeCell<T> = machine()->Cell<T> reaches none crashes none;
        data InspectView<'a,T> = machine(x:View<'a,T>)->Phantom<T> reaches none crashes none;
        data InspectBuffer<T> = machine(x:@T,buffer:@[u8])->u8 reaches none crashes none;";
    for field in [
        "S",
        "[S;2]",
        "Alias<S>",
        "Cell<Cell<S>>",
        "Choice<S>",
        "@'a [S]",
        "@'a Unsized<S>",
        "Nested<S>",
        "TakeValue<S>",
        "MakeCell<S>",
    ] {
        let source = format!("data Boxed<'a,some S>{{value:{field};}} {wrappers}");
        let error = check(&[("layout.omg", &source)]).unwrap_err();
        assert!(error.message.contains("witness-dependent representation"), "{field}: {error:?}");
        assert!(
            error
                .span
                .slice(source.as_bytes())
                .unwrap()
                .starts_with(b"value:"),
            "{error:?}"
        );
    }
    for field in [
        "@'a S",
        "[@'a S;2]",
        "Cell<Cell<@'a S>>",
        "Choice<View<'a,S>>",
        "Phantom<S>",
        "Link<'a,S>",
        "@'a Cell<S>",
        "Cell<Nested<Cell<@'a S>>>",
        "InspectView<'a,S>",
        "InspectBuffer<S>",
    ] {
        let source = format!("data Boxed<'a,some S>{{value:{field};}} {wrappers}");
        check(&[("layout.omg", &source)]).unwrap_or_else(|error| panic!("{field}: {error:?}"));
    }
    let cases = "data Choice<some S>{case Known(value:u8);case Hidden(value:S);}";
    assert!(
        check(&[("layout.omg", cases)])
            .unwrap_err()
            .message
            .contains("witness-dependent representation")
    );
}

#[test]
fn library_carriers_can_keep_the_packaged_type_in_a_domain_index() {
    let carriers = "domain<T> u8::State<T>; data Handle<T> [linear]{tag:u8 in State<T>;} data File{}";
    let package = "data RunHandle<S> = machine(h:Handle<S>)->u8 reaches none crashes none;
        data Job<some S> [linear]{handle:Handle<S>;run:RunHandle<S>;}
        machine run_file(h:Handle<File>)->u8 reaches none crashes none {0}
        machine pack(h:Handle<File>)->Job{Job{handle:h,run:run_file}}";
    let stage = check(&[("job.omg", package), ("handle.omg", carriers)]).unwrap();
    let [constructed] = stage.packages.as_slice() else {
        panic!("expected one package");
    };
    assert!(matches!(constructed.witnesses.as_slice(), [omega_typer::ValueType::Nominal(file)] if file.path == "File"));
    let inline = carriers.replace("tag:u8 in State<T>", "tag:T");
    let error = check(&[("handle.omg", &inline), ("job.omg", package)]).unwrap_err();
    assert_eq!(error.file, "job.omg");
    assert!(error.message.contains("witness-dependent representation"), "{error:?}");
}

#[test]
fn packaged_reference_requires_a_sized_concrete_witness() {
    let schema = "data View<'a,some S>{ptr:@'a S;} data Holder<T>{v:T;} data Huge{values:[u8];}";
    for (borrow, accepted) in [
        ("@'a [u8]", false),
        ("@'a [u8;2]", true),
        ("@'a @[u8]", true),
        ("@'a Holder<Huge>", false),
        ("@'a Holder<@Huge>", true),
    ] {
        let source = format!("{schema} machine pack<'a>(p:{borrow})->View<'a>{{View{{ptr:p}}}}");
        match check(&[("size.omg", &source)]) {
            Ok(_) => assert!(accepted, "{borrow}"),
            Err(error) => assert!(!accepted && error.message.contains("sized"), "{borrow}: {error:?}"),
        }
    }
}

#[test]
fn packaged_size_follows_inline_storage_and_stops_at_references() {
    let schema = "data View<'a,some S>{ptr:@'a S;}";
    for usage in [
        "machine pack<'a,const N:u64>(x:@'a [u8;N])->View<'a>{View{ptr:x}}",
        "machine pack<'a,const N:u64>(x:@'a u<N>)->View<'a> requires N>0 {View{ptr:x}}",
        "machine pack(){let t=(true,0u8); let view=View{ptr:@t};}",
        "data Link<'a>{next:@'a Link<'a>;} machine pack<'a>(x:@'a Link<'a>)->View<'a>{View{ptr:x}}",
    ] {
        check(&[("size.omg", &format!("{schema} {usage}"))]).unwrap_or_else(|error| panic!("{usage}: {error:?}"));
    }
    let unbounded = format!("{schema} data Cycle<T>{{next:Cycle<Cycle<T>>;}} machine pack<'a>(x:@'a Cycle<u8>)->View<'a>{{View{{ptr:x}}}}");
    assert!(
        check(&[("size.omg", &unbounded)])
            .unwrap_err()
            .message
            .contains("must be sized")
    );

    // The external value has 2^28 leaves, but only 29 distinct field declarations.
    let mut shared = format!("{schema} data Layer0{{x:u8;}}");
    for n in 1..=28 {
        let child = n - 1;
        shared.push_str(&format!(" data Layer{n}{{a:Layer{child};b:Layer{child};}}"));
    }
    shared.push_str(" machine pack<'a>(x:@'a Layer28)->View<'a>{View{ptr:x}}");
    check(&[("shared.omg", &shared)]).unwrap();
}

#[test]
fn core_bracket_contracts_retain_endpoints_access_and_origin() {
    let source = "machine f(view:@mutable [u8])->u8{let tail = @mutable view[2..]; tail[0]=7; view[1]}";
    let resolved = support::resolve_packages(&[("game", "a.omg", source)], &[]);
    let stage = omega_typer::type_stage(&resolved).unwrap();
    assert_eq!(
        stage
            .collection_accesses
            .iter()
            .map(|a| a.machine.path.as_str())
            .collect::<Vec<_>>(),
        ["indexing\\range_mut", "indexing\\element_mut", "indexing\\read"]
    );
    let syntax = resolved.trees().next().unwrap().file;
    let range = syntax.nodes[stage.collection_accesses[0].node.node].children[1];
    assert!(matches!(
        syntax.range_bounds(range),
        Some(omega_parser::RangeBounds {
            start: Some(_),
            end: None,
            inclusive: false
        })
    ));
    use omega_program_contracts::CollectionPrimitive::{ElementMut, RangeMut, Read};
    let library = stage.library.expect("sealed bracket declarations");
    for (access, primitive) in stage
        .collection_accesses
        .iter()
        .zip([RangeMut, ElementMut, Read])
    {
        assert_eq!(access.primitive, primitive);
        let signature = library
            .signatures
            .iter()
            .find(|s| s.machine == access.machine)
            .unwrap();
        assert!(
            access
                .selector_carriers
                .iter()
                .eq(signature.parameters[1..].iter().map(|(_, carrier)| carrier))
        );
    }
    for (code, msg) in [
        ("machine f(s:@[u8]){ _ = @mutable s[..]; }", "mutable authority"),
        ("data R{} machine f(s:@[R])->R{s[0]}", "copy element"),
        ("machine f(s:@mutable [u8]){let r:@mutable [u8]=s[..];}", "expected @mutable"),
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
        "machine f(v:@mutable [u8]){ _ = @mutable v.length; }",
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
    let head = "data B{n:u8;} machine [] at(b:@mutable B,i:u64)->@mutable u8{@mutable b.n} machine [..] between(b:@B,start:u64,end:u64)->u8{b.n}";
    let stage = check(&[("custom.omg", &format!("{head} machine f(v:B)->u8{{let mutable b=v; b[0]=1; b[0..2]}}"))]).unwrap();
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
fn writing_requires_a_storage_location_then_writable_authority() {
    let declarations = "data Pair{field:u8;} machine value()->u8{1u8} machine pair()->Pair{Pair{field:1u8}}";
    for (target, reason) in [
        ("value()", "computed value does not name writable storage"),
        ("1u8", "computed value does not name writable storage"),
        ("pair().field", "computed value does not name writable storage"),
        ("item.field", "place requires writable authority"),
    ] {
        for use_site in [format!("{target}=2u8;"), format!("_=@mutable {target};")] {
            let source = format!("{declarations} machine f(item:Pair){{{use_site}}}");
            let error = check(&[("write.omg", &source)]).unwrap_err();
            assert_eq!(error.message, reason, "{source}");
            assert_eq!(error.span.slice(source.as_bytes()), Some(target.as_bytes()), "{source}");
        }
    }
    let good = format!("{declarations} machine f(){{let mutable item=pair(); item.field=2u8; let mutable cell=value(); cell=2u8;}}");
    check(&[("write.omg", &good)]).unwrap();
}

#[test]
fn parentheses_preserve_field_authority_and_collection_write_selection() {
    let cell = "data Cell {value:u8;}";
    for place in ["view.value", "(view.value)", "((view.value))", "((view).value)"] {
        for action in [
            format!("{place}=7;"),
            format!("let ref_to_field=@mutable {place};"),
            format!("let output=@write {place};"),
        ] {
            let shared = format!("{cell} machine f(input:@Cell) {{let mutable view:@Cell=input; {action}}}");
            let error = check(&[("place.omg", &shared)]).unwrap_err();
            assert_eq!(error.message, "place requires writable authority");
            let exclusive = format!("{cell} machine f(view:@mutable Cell) {{{action}}}");
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
        for target in [format!("{site}=7;"), format!("let r=@mutable {site};")] {
            let source = format!("machine f(view:@mutable [u8]) {{{target}}}");
            let stage = check(&[("index.omg", &source)]).unwrap();
            let [access] = stage.collection_accesses.as_slice() else {
                panic!("expected one mutable indexing selection")
            };
            assert_eq!(access.machine.path, "indexing\\element_mut");
        }
    }
    for place in ["array.length", "((array.length))"] {
        let source = format!("machine f() {{let mutable array:[u8;2]; {place}=3;}}");
        assert_eq!(check(&[("metadata.omg", &source)]).unwrap_err().message, "collection metadata is read-only");
    }
}

#[test]
fn nested_runtime_callables_require_both_effect_ceilings_at_their_own_type() {
    for context in [
        "data Slot {callback:Callback;}",
        "machine f(cb:Callback) {}",
        "machine f() {let mutable cb:Callback;}",
        "data Closed = machine(x:u8)->u8 reaches none crashes none; machine f(x:Closed)->Callback {x}",
        "machine f<'a>(cb:@'a Callback) {}",
        "machine f(cb:[Callback;2]) {}",
        "machine f(cb:@Callback) {}",
        "data Box<T>{item:T;} machine f(cb:Box<Callback>) {}",
        "data Apply = machine(g:Callback) reaches none crashes none; machine f(cb:Apply) {}",
        "data Apply = machine(g:Callback); machine f<const F:Apply>() {}",
        "data Make = machine()->Callback; machine f<const F:Make>() {}",
    ] {
        let closed = format!("data Callback = machine(x:u8)->u8 reaches none crashes none; {context}");
        check(&[("callback.omg", &closed)]).unwrap();
        for (clause, missing) in [
            (" reaches none", "crash"),
            (" crashes none", "reach"),
            ("", "reach"),
        ] {
            let source = format!("data Callback = machine(x:u8)->u8{clause}; {context}");
            let error = check(&[("callback.omg", &source)]).unwrap_err();
            assert!(error.message.contains(missing), "{error:?}");
            assert_eq!(error.span.slice(source.as_bytes()), Some(b"Callback".as_slice()));
        }
    }
    check(&[("static.omg", "data Work = machine(x:u8)->u8; machine invoke<const F:Work>(x:u8)->u8{F(x)}")]).unwrap();
}

#[test]
fn a_write_only_field_loan_cannot_be_strengthened_to_readable_access() {
    let defs = "data Cell {value:u8;}";
    for place in ["cell.value", "(cell.value)"] {
        let source = format!("{defs} machine observe(cell:@write Cell)->u8 {{let view=@mutable {place}; $view}}");
        let error = check(&[("loan.omg", &source)]).unwrap_err();
        assert_eq!(error.message, "cannot read through a write-only reference");
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"cell.value".as_slice()));
        let allowed = source.replace("cell:@write Cell", "cell:@mutable Cell");
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
                    ("@mutable", omega_typer::Access::Mut),
                    ("@write", omega_typer::Access::Write),
                ] {
                    let source = format!("data Cell{{value:u8;}} data Outer{{inner:Cell;}} machine f(view:@mutable Outer){{let loan={prefix} {place};}}");
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
                    let shared = source.replace("view:@mutable Outer", "view:@Outer");
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
            let annotation = outer.replace("TYPE", inner).replace("TYPE", "Callback");
            for (suffix, diagnostic) in [
                (" reaches none crashes none", None),
                (" reaches none", Some("runtime callable requires an explicit crash ceiling")),
                (" crashes none", Some("runtime callable requires an explicit reach ceiling")),
                ("", Some("runtime callable requires explicit reach and crash ceilings")),
            ] {
                let source = format!("data Callback = machine(x:u8)->u8{suffix}; data Box<T>{{item:T;}} machine f(cb:{annotation}){{}}");
                let result = check(&[("rewrite.omg", &source)]);
                if let Some(message) = diagnostic {
                    let error = result.unwrap_err();
                    assert_eq!(error.message, message, "{source}");
                    assert_eq!(error.span.slice(source.as_bytes()), Some(b"Callback".as_slice()), "{source}");
                } else {
                    result.unwrap_or_else(|error| panic!("{source}: {error:?}"));
                }
            }
        }
    }
}

#[test]
fn a_package_infers_one_witness_through_array_and_callable_fields() {
    let schema = "data File {} data Socket {} data ReadState<S> = machine(state:@S)->u8 reaches none crashes none; data Reader<'a,some S> {views:[@'a S;2];read:ReadState<S>;} machine read_file(state:@File)->u8 reaches none crashes none {0}";
    for fields in [
        "views:[view,view],read:read_file",
        "read:read_file,views:[view,view]",
    ] {
        let source = format!("{schema} machine pack<'a>(view:@'a File) {{ let reader:Reader<'a> = Reader::<'a>{{{fields}}}; }}");
        let stage = check(&[("reader.omg", &source)]).unwrap();
        let [package] = stage.packages.as_slice() else {
            panic!("expected one construction")
        };
        assert!(matches!(package.witnesses.as_slice(),[ValueType::Nominal(id)] if id.path=="File"));
        assert!(stage.obligations.iter().any(|o|matches!(&o.kind,omega_typer::ObligationKind::RefineCallable {selection,contract,..} if selection.machine.path=="read_file" && contract.parameters[0].to_string().contains("File"))));
        let wrong = source.replace("read_file(state:@File)", "read_file(state:@Socket)");
        let error = check(&[("reader.omg", &wrong)]).unwrap_err();
        let site = error.span.slice(wrong.as_bytes()).unwrap();
        assert!(matches!(site, b"read_file" | b"view"), "{error:?}");
        assert!(error.message.contains("witness") || error.message.contains("Socket"), "{error:?}");
    }
}

#[test]
fn local_record_annotations_preserve_distinct_external_lifetimes() {
    let schema = "data Pair<'a,'b>{left:@'a u8;right:@'b u8;} data Alias<'x,'y> =Pair<'y,'x>;";
    let source = format!("{schema} machine pack<'a,'b>(a:@'a u8,b:@'b u8)->Pair<'a,'b>{{let p:Alias=Pair{{left:a,right:b}};p}}");
    check(&[("main.omg", &source)]).unwrap();
    // In a local annotation the input value supplies the erased arguments.
    let forward = format!("{schema} machine f<'a,'b>(p:Pair<'a,'b>)->Pair<'a,'b>{{let q:Alias=p;q}}");
    check(&[("main.omg", &forward)]).unwrap();
    let local = "data View<'a>{ref:@'a u8;} machine leak<'a>()->View<'a>{let n:u8=1; View{ref:@n}}";
    assert_eq!(
        check(&[("main.omg", local)]).unwrap_err().message,
        "returned reference borrows invocation-local storage"
    );
    let exact = source.replace("let p:Alias=Pair", "let p:Alias<'b,'a> =Pair");
    check(&[("main.omg", &exact)]).unwrap();
    let wrong = exact.replace("let p:Alias<'b,'a>", "let p:Alias<'a,'b>");
    assert!(check(&[("main.omg", &wrong)]).is_err());
    let scalar = "const N:u8=1; data Pair<'a,'b>{left:@'a u8;right:@'b u8;} machine f<'a>(x:@'a u8){let p:Pair<N> =Pair{left:x,right:x};}";
    assert!(
        check(&[("main.omg", scalar)])
            .unwrap_err()
            .message
            .contains("lifetime argument")
    );
}

#[test]
fn a_packaged_hidden_type_leaves_the_value_argument_in_its_external_slot() {
    let text = "data Packet<'a,some S,const N:u64>{view:@'a S;bytes:[u8;N];} data X{} const X:u64=2; machine f<'a>(input:Packet<'a,X>){}";
    let stage = check(&[("packet.omg", text)]).unwrap();
    let signature = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
    assert!(
        matches!(&signature.parameters[0].1, ValueType::Applied(owner,args) if owner.path=="Packet" && matches!(args.as_slice(),[ValueType::Parameter {..},ValueType::StaticInteger {value,..}] if *value==2.into()))
    );
}

#[test]
fn two_input_regions_constrain_one_elided_region_at_the_field_initializers() {
    let text = "data Views<'a>{a:@'a u8;b:@'a u8;} machine f<'x,'y>(x:@'x u8,y:@'y u8){let pair=Views{a:x,b:y};}";
    let stage = check(&[("regions.omg", text)]).unwrap();
    let ty = &stage
        .expressions
        .iter()
        .find(|e| e.site.span.slice(text.as_bytes()) == Some(b"Views{a:x,b:y}"))
        .unwrap()
        .ty;
    let ValueType::Applied(id, args) = ty else {
        panic!("expected an applied record")
    };
    assert_eq!(id.path, "Views");
    let [region @ ValueType::InferredLifetime { index: 0, .. }] = args.as_slice() else {
        panic!("expected the common use region")
    };
    let sites: Vec<_> = stage
        .obligations
        .iter()
        .filter_map(|o| match &o.kind {
            omega_typer::ObligationKind::BindLifetime {
                reference: ValueType::AtLifetime(_, source),
                destination: ValueType::AtLifetime(_, required),
            } if required.as_ref() == region => Some((o.site.span.slice(text.as_bytes()).unwrap(), source.as_ref())),
            _ => None,
        })
        .collect();
    assert!(matches!(
        sites.as_slice(),
        [
            (b"x", ValueType::Parameter { index: 0, .. }),
            (b"y", ValueType::Parameter { index: 1, .. })
        ]
    ));
}

// A sum case names its carrier as a record head does, so a case literal with no
// destination infers the sum's lifetime from its payload: the lifetime the
// payload names, or a region of this construction for a reference that names
// none. A case without a payload still forms its carrier.
#[test]
fn a_case_literal_infers_its_sum_lifetime_from_the_payload_as_a_record_does() {
    let text = "data Reply<'a>{case Ready(view:@'a u8);case Empty;} machine build<'x>(x:@'x u8,y:@u8){let named=Reply::Ready{view:x};let local=Reply::Ready{view:y};let empty=Reply::Empty;}";
    let stage = check(&[("reply.omg", text)]).unwrap();
    let lifetime = |literal: &str| {
        let ty = &stage
            .expressions
            .iter()
            .find(|e| e.site.span.slice(text.as_bytes()) == Some(literal.as_bytes()))
            .unwrap_or_else(|| panic!("no expression {literal}"))
            .ty;
        let ValueType::Applied(id, args) = ty else {
            panic!("{literal}: expected an applied sum, found {ty}")
        };
        assert_eq!(id.path, "Reply");
        let [argument] = args.as_slice() else {
            panic!("{literal}: expected one lifetime argument")
        };
        argument.clone()
    };
    assert!(matches!(lifetime("Reply::Ready{view:x}"), ValueType::Parameter { owner, index: 0 } if owner.path == "build"));
    let region = lifetime("Reply::Ready{view:y}");
    assert!(matches!(region, ValueType::InferredLifetime { index: 0, .. }), "{region:?}");
    assert!(stage.obligations.iter().any(|o| {
        o.site.span.slice(text.as_bytes()) == Some(b"y".as_slice())
            && matches!(&o.kind, omega_typer::ObligationKind::BindLifetime { reference: ValueType::Reference(..), destination: ValueType::AtLifetime(_, required) }
                if required.as_ref() == &region)
    }));
    assert!(matches!(lifetime("Reply::Empty"), ValueType::InferredLifetime { index: 0, .. }));
}

#[test]
fn a_record_predicate_can_only_read_its_own_declared_field() {
    let defs = "data Other{value:bool;}";
    let own = "data Record requires value {value:bool;}";
    check(&[("other.omg", defs), ("record.omg", own)]).unwrap();
    let wrong = own.replace("requires value", "requires Other::value");
    let error = check(&[("other.omg", defs), ("record.omg", &wrong)]).unwrap_err();
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"Other::value".as_slice()));
}

#[test]
fn positional_rest_checks_only_written_patterns_and_covers_the_omitted_positions() {
    for source in [
        "data Pair { left:u8; right:u8; } machine left(p:Pair)->u8 { let (first, ..) = p; first }",
        "data Pair { left:u8; right:u8; } machine waive(p:Pair) { let (..) = p; }",
        "machine f(a:u8, b:bool)->u8 { let (first, ..) = (a,b); first }",
        "machine f(a:bool, b:bool, c:u8)->u8 { match (a,b,c) { (true, ..) -> 1, (false, true, ..) -> 2, (false, false, value) -> value, } }",
    ] {
        check(&[("rest.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    }
    for (source, message) in [
        (
            "machine f(a:u8, b:bool) { let (first, second, extra, ..) = (a,b); }",
            "tuple pattern arity mismatch",
        ),
        (
            "machine f(a:bool, b:u8)->u8 { match (a,b) { (true, ..) -> 1, } }",
            "dispatch does not cover its subject",
        ),
        (
            "data Pair { left:u8; right:u8; } machine left(p:@write Pair)->u8 { let (first, ..) = p; first }",
            "cannot read through a write-only reference",
        ),
    ] {
        let error = check(&[("rest.omg", source)]).unwrap_err();
        assert!(error.message.contains(message), "{source}: {error:?}");
    }
}

#[test]
fn tuple_dispatch_checks_case_coverage_per_reachable_column_branch() {
    let schema = "data Mode { case On; case Off(payload: bool); }";
    let cases = "match (n, x) { (0, _) -> false, (_, Mode::On) -> true, (_, Mode::Off { payload as _ }) -> false, }";
    let mixed = "match (b, x) { (true, _) -> true, (false, Mode::On) -> false, (false, Mode::Off { payload as _ }) -> true, }";
    let source = |dispatch: &str| format!("{schema}\nmachine pick(n: u8, b: bool, x: Mode) -> bool {{\n    {dispatch}\n}}");

    let nested = "match ((n, x), b) { ((_, Mode::On), _) -> true, ((_, Mode::Off { payload as _ }), _) -> false, }";
    let whole_tuple = "match ((n, x), b) { (_, true) -> true, ((_, Mode::On), false) -> true, ((_, Mode::Off { payload as _ }), false) -> false, }";
    for dispatch in [cases, mixed, nested, whole_tuple] {
        let text = source(dispatch);
        let stage = check(&[("coverage.omg", &text)]).unwrap();
        assert!(stage.expressions.iter().any(|e| {
            e.site.file == "coverage.omg"
                && e.site.span.slice(text.as_bytes()) == Some(dispatch.as_bytes())
                && e.ty == ValueType::Bool
        }), "missing Bool expression at {dispatch}");
    }
    for dispatch in [
        cases.replace(", (_, Mode::Off { payload as _ }) -> false", ""),
        mixed.replace("payload as _", "payload: true"),
        nested.replace(", ((_, Mode::Off { payload as _ }), _) -> false", ""),
        whole_tuple.replace("((_, Mode::Off { payload as _ }), false)", "((_, Mode::Off { payload as _ }), true)"),
        mixed.replace("(false, Mode::Off", "(true, Mode::Off"),
    ] {
        let text = source(&dispatch);
        let error = check(&[("coverage.omg", &text)]).unwrap_err();
        assert_eq!(error.file, "coverage.omg");
        assert_eq!(error.message, "dispatch does not cover its subject");
        assert_eq!(error.reason(), Some(omega_typer::DispatchError::UncoveredSubject.into()));
        assert_eq!(error.span.slice(text.as_bytes()), Some(dispatch.as_bytes()));
    }
}

#[test]
fn runtime_construction_publishes_each_applied_payload_without_a_static_request() {
    let code = "data Box<T> { case Full(value:T); } machine build() { let a = Box::<u8>::Full {value:7}; let b = Box::<bool>::Full {value:true}; }";
    let stage = check(&[("box.omg", code)]).unwrap();
    assert!(stage.constants.is_empty());
    let mut count = 0;
    for (carrier, shape) in &stage.static_data_shapes {
        let Some((owner, [arg])) = carrier.nominal_parts() else { continue };
        if owner.path != "Box" {
            continue;
        }
        let [(case, fields)] = shape.cases.as_slice() else {
            panic!("expected the Full case")
        };
        assert_eq!(case.path, "Box::Full");
        assert_eq!(fields.as_slice(), std::slice::from_ref(arg));
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn an_unused_formal_demands_exactly_its_applied_record_schema() {
    spec_corpus::observe_handoff!("unused-formal-schema/producer", {
        for (formal, width) in [("Buffer<u32>", Some(32)), ("Buffer<u16>", Some(16)), ("u32", None)] {
            let source = format!("data Buffer<T> {{ unused:bool; used:T; }} machine f(b:{formal}) {{}}");
            let stage = check(&[("formal.omg", &source)]).unwrap();
            let mut applications = stage.static_data_shapes.iter().filter_map(|(carrier, shape)| {
                let (declaration, [argument]) = carrier.nominal_parts()? else { return None };
                (declaration.path == "Buffer").then_some((argument, &shape.fields))
            });
            if let Some(width) = width {
                let field_type = ValueType::Integer { signed: false, width: Some(width) };
                let (argument, fields) = applications.next().expect("the exact formal application");
                assert_eq!(argument, &field_type);
                assert_eq!(fields.as_slice(), [ValueType::Bool, field_type]);
            }
            assert!(applications.next().is_none(), "no unrelated Buffer application for {formal}");
        }
    });
}

#[test]
fn equal_named_cases_publish_their_exact_declaring_sum() {
    let code = "data Alpha { case Same(w:u8, v:u8); case Other; } data Beta { case Empty; case Same(v:u8, w:u8); }";
    let stage = check(&[("cases.omg", code)]).unwrap();
    let mut rows: Vec<_> = stage
        .case_owners
        .iter()
        .filter(|(_, owner)| matches!(owner.path.as_str(), "Alpha" | "Beta"))
        .map(|(case, owner)| (case.path.as_str(), owner.path.as_str()))
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        [("Alpha::Other", "Alpha"), ("Alpha::Same", "Alpha"), ("Beta::Empty", "Beta"), ("Beta::Same", "Beta")]
    );
    let owner_of = |path: &str| {
        let (case, owner) = stage.case_owners.iter().find(|(case, _)| case.path == path).unwrap();
        assert!(stage.record_fields.contains_key(case), "{path} keeps its payload roster");
        (case.clone(), owner.clone())
    };
    let (alpha_same, alpha) = owner_of("Alpha::Same");
    let (beta_same, beta) = owner_of("Beta::Same");
    assert_ne!(alpha_same, beta_same);
    assert_ne!(alpha, beta);
    // Sums are owners, never cases of themselves or of each other.
    assert!(!stage.case_owners.contains_key(&alpha));
    assert!(!stage.case_owners.contains_key(&beta));
}

#[test]
fn a_closed_array_field_and_an_owned_formal_publish_their_distinct_payloads() {
    let stage = check(&[("field.omg", "data Item<T> { case Full(value:T); } data Owner { field:[Item<u64>;2]; } machine observe(v:Item<u16>) {}")]).unwrap();
    let mut widths = Vec::new();
    for (carrier,shape) in &stage.static_data_shapes {
        let Some((owner, [arg])) = carrier.nominal_parts() else { continue };
        if owner.path != "Item" { continue; }
        let [(case, fields)] = shape.cases.as_slice() else { panic!("Full case") };
        assert_eq!(case.path, "Item::Full");
        assert_eq!(fields.as_slice(), std::slice::from_ref(arg));
        let ValueType::Integer {signed:false, width:Some(width)} = arg else { panic!("unsigned field") };
        widths.push(*width);
    }
    widths.sort();
    assert_eq!(widths,[16,64]);
}
