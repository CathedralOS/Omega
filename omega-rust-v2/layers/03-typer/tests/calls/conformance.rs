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
