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
    let source = "data Pair<some S>{a:@S;b:@S;} machine make(x:@u8)->Pair{Pair{a:x,b:x}} machine compare<T>(a:T,b:T)->bool{true} machine ok(p:Pair)->bool{compare(p.a,p.b)}";
    let stage = check(&[("a.omg", source)]).unwrap();
    assert_eq!(stage.packages.len(), 1);
    for (tail, msg) in [
        ("machine mismatch(x:@bool,y:@u8)->Pair{Pair{a:x,b:y}}", "inconsistent package witness"),
        (
            "machine compare<T>(a:T,b:T){} machine mismatch(p:Pair,q:Pair){compare(p.a,q.a)}",
            "no matching overload",
        ),
    ] {
        let code = format!("data Pair<some S>{{a:@S;b:@S;}} {tail}");
        let e = check(&[("a.omg", &code)]).unwrap_err();
        assert!(e.message.contains(msg), "{e:?}");
    }
}

#[test]
fn destructuring_and_projection_open_the_same_package_subject() {
    let schema = "data Pair<some S>{a:@S;b:@S;} data Wrap{p:Pair;} machine compare<T>(a:@T,b:@T)->bool{true}";
    for (body, accepted) in [
        ("let Pair{a,..} = p; compare(a,(p).b)", true),
        ("let Pair{a,..} = p; let Pair{b,..} = q; compare(a,b)", false),
        ("let Pair{a,..} = w.p; compare(a,(w.p).b)", true),
        ("let (Pair{a,..},Pair{b,..}) = (p,q); compare(a,b)", false),
        ("let (Pair{a,..},Pair{b,..}) = (p,p); compare(a,b)", true),
        ("let (Pair{a,..},) = w; compare(a,w.p.b)", true),
    ] {
        let code = format!("{schema} machine same(p:Pair,q:Pair,w:Wrap)->bool{{{body}}}");
        match check(&[("open.omg", &code)]) {
            Ok(_) => assert!(accepted, "{body}"),
            Err(error) => assert!(!accepted && error.message.contains("no matching overload"), "{body}: {error:?}"),
        }
    }
}

#[test]
fn packaged_fields_require_a_uniform_representation_at_the_declaration() {
    let wrappers = "data Cell<T>{item:T;} data View<T>{ptr:@T;} data Phantom<T>{tag:u8;} data Choice<T>{case None;case Stored(item:T);} data Alias<T> = Cell<T>; data Link<T>{next:@Link<T>;item:@T;} data Unsized<T>{item:[T];} data Nested<T>{v:Cell<T>;}";
    for field in [
        "S",
        "[S;2]",
        "Alias<S>",
        "Cell<Cell<S>>",
        "Choice<S>",
        "@[S]",
        "@Unsized<S>",
        "Nested<S>",
        "machine(x:S)->u8 reaches none crashes none",
        "machine()->Cell<S> reaches none crashes none",
    ] {
        let source = format!("data Boxed<some S>{{value:{field};}} {wrappers}");
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
        "@S",
        "[@S;2]",
        "Cell<Cell<@S>>",
        "Choice<View<S>>",
        "Phantom<S>",
        "Link<S>",
        "@Cell<S>",
        "Cell<Nested<Cell<@S>>>",
        "machine(x:View<S>)->Phantom<S> reaches none crashes none",
        "machine(x:@S,buffer:@[u8])->u8 reaches none crashes none",
    ] {
        let source = format!("data Boxed<some S>{{value:{field};}} {wrappers}");
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
    let package = "data Job<some S> [linear]{handle:Handle<S>;run:machine(h:Handle<S>)->u8 reaches none crashes none;}
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
    let schema = "data View<some S>{ptr:@S;} data Holder<T>{v:T;} data Huge{values:[u8];}";
    for (borrow, accepted) in [
        ("@[u8]", false),
        ("@[u8;2]", true),
        ("@@[u8]", true),
        ("@Holder<Huge>", false),
        ("@Holder<@Huge>", true),
    ] {
        let source = format!("{schema} machine pack(p:{borrow})->View{{View{{ptr:p}}}}");
        match check(&[("size.omg", &source)]) {
            Ok(_) => assert!(accepted, "{borrow}"),
            Err(error) => assert!(!accepted && error.message.contains("sized"), "{borrow}: {error:?}"),
        }
    }
}

#[test]
fn packaged_size_follows_inline_storage_and_stops_at_references() {
    let schema = "data View<some S>{ptr:@S;}";
    for usage in [
        "machine pack<const N:u64>(x:@[u8;N])->View{View{ptr:x}}",
        "machine pack<const N:u64>(x:@u<N>)->View requires N>0 {View{ptr:x}}",
        "machine pack(){let t=(true,0u8); let view=View{ptr:@t};}",
        "data Link{next:@Link;} machine pack(x:@Link)->View{View{ptr:x}}",
    ] {
        check(&[("size.omg", &format!("{schema} {usage}"))]).unwrap_or_else(|error| panic!("{usage}: {error:?}"));
    }
    let unbounded = format!("{schema} data Cycle<T>{{next:Cycle<Cycle<T>>;}} machine pack(x:@Cycle<u8>)->View{{View{{ptr:x}}}}");
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
    shared.push_str(" machine pack(x:@Layer28)->View{View{ptr:x}}");
    check(&[("shared.omg", &shared)]).unwrap();
}

#[test]
fn core_bracket_contracts_retain_endpoints_access_and_origin() {
    let source = "machine f(view:@mut [u8])->u8{let tail = @mut view[2..]; tail[0]=7; view[1]}";
    let resolved = support::resolve_packages(&[("game", "a.omg", source)], &[]);
    let stage = omega_typer::type_stage(&resolved).unwrap();
    assert_eq!(
        stage
            .collection_accesses
            .iter()
            .map(|a| a.machine.path.as_str())
            .collect::<Vec<_>>(),
        ["indexing::range_mut", "indexing::element_mut", "indexing::read"]
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
    for access in &stage.collection_accesses {
        assert!(
            stage
                .library
                .expect("sealed bracket declarations")
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

#[test]
fn a_package_infers_one_witness_through_array_and_callable_fields() {
    let schema = "data File {} data Socket {} data Reader<'a,some S> {views:[@'a S;2]; read:machine(state:@S)->u8 reaches none crashes none;} machine read_file(state:@File)->u8 reaches none crashes none {0}";
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
        assert!(stage.obligations.iter().any(|o|matches!(&o.kind,omega_typer::ObligationKind::RefineCallable {machine,contract,..} if machine.path=="read_file" && contract.parameters[0].to_string().contains("File"))));
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
    let text = "data Packet<some S,const N:u64>{view:@S;bytes:[u8;N];} data X{} const X:u64=2; machine f(input:Packet<X>){}";
    let stage = check(&[("packet.omg", text)]).unwrap();
    let signature = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
    assert!(
        matches!(&signature.parameters[0].1, ValueType::Applied(owner,args) if owner.path=="Packet" && matches!(args.as_slice(),[ValueType::StaticInteger {value,..}] if *value==2.into()))
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
