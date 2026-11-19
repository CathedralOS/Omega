#[test]
fn selected_operation_and_conformance_rows_retain_machine_identity() {
    let code = "domain u8::Distance; machine + add(a:u8 in Distance,b:u8 in Distance)->u8 in Distance { ((a as -Distance) +% (b as -Distance)) as +Distance } machine sum(a:u8 in Distance,b:u8 in Distance)->u8 in Distance{ a+b } trait Logger<T>{machine write(x:T);} machine log(x:u8){} Sink satisfies Logger<u8>{ Logger::write = log; }";
    let stage = check(&[("a.omg", code)]).unwrap();
    assert_eq!(stage.calls[0].machine.path, "add");
    let row = &stage.conformance_rows[0];
    assert_eq!(row.requirement.declaration.path, "Logger::write");
    assert_eq!(row.implementation.path, "log");
    assert!(stage.obligations.iter().any(|o| o.site.file == "a.omg"
        && o.site.span.slice(code.as_bytes()) == Some(b"Logger::write = log;")
        && matches!(&o.kind, omega_typer::ObligationKind::RefineRequirement { requirement, implementation }
            if requirement == &row.requirement && implementation == &row.implementation)));
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
            .any(|r| r.requirement.declaration.path == "Parent::get" && r.requirement.trait_arguments[0].to_string() == "u8")
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
fn static_member_uses_the_normalized_evidence_bound() {
    let declarations = "trait Parent<T>{machine ensures(x:T)->T;} trait Child<U>:Parent<U>{} data Alias<V> = Child<V>; trait Other{machine missing();}";
    let call = "machine call<const E:Alias<u8>>(x:u8)->u8{E::ensures(x)}";
    let code = format!("{declarations} {call}");
    let stage = check(&[("alias.omg", &code)]).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|call| call.machine.path == "Parent::ensures" && call.evidence.is_some())
    );
    let generic = "trait Convert {machine keep<T>(x:T)->T;} data Alias = Convert; machine call<const E:Alias>(u8:bool,x:u8)->u8{E::keep::<u8>(x)}";
    let misplaced = generic.replace("E::keep::<u8>", "E::<u8>::keep");
    let error = check(&[("misplaced.omg", &misplaced)]).unwrap_err();
    assert!(error.message.contains("evidence binder"), "{error:?}");
    let generic = check(&[("generic.omg", generic)]).unwrap();
    assert!(
        generic
            .calls
            .iter()
            .any(|c| c.machine.path == "Convert::keep" && c.type_arguments.iter().map(ToString::to_string).eq(["u8"]))
    );
    let bad = code.replace("E::ensures(x)", "E::missing()");
    let error = check(&[("alias.omg", &bad)]).unwrap_err();
    assert!(error.message.contains("no requirement"), "{error:?}");
}

#[test]
fn only_an_eligible_static_binder_competes_with_a_visible_namespace() {
    let items = "trait Ops {machine run();} data OpsAlias = Ops;";
    for (bound, member, ambiguous) in [
        ("u8", "run", false),
        ("OpsAlias", "run", true),
        ("OpsAlias", "absent", true),
    ] {
        let source = format!("{items} machine f<const X:{bound}>(){{ X::{member}() }}");
        let result = check(&[("main.omg", &source), ("X/run.omg", "machine run() {}")]);
        if ambiguous {
            let error = result.unwrap_err();
            assert!(error.message.contains("ambiguous static root"), "{error:?}");
            assert_eq!(error.span.slice(source.as_bytes()), Some(b"X".as_slice()));
        } else {
            assert_eq!(result.unwrap().calls[0].machine.path, "X::run");
        }
    }
    for (import, file, declarations) in [
        ("use nested::X;", "nested/X/items.omg", "machine run() {}"),
        ("use nested::X;", "nested/items.omg", "trait X {machine run();}"),
    ] {
        let source = format!("{import} {items} machine f<const X:OpsAlias>(){{ X::run() }}");
        let error = check(&[("main.omg", &source), (file, declarations)]).unwrap_err();
        assert!(error.message.contains("ambiguous static root"), "{file}: {error:?}");
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"X".as_slice()));
    }
    let source = "domain u32::Small; machine f<const u32:u8>(x:u32 in u32::Small) {}";
    check(&[("domain.omg", source)]).unwrap();
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
    assert!(stage.obligations.iter().any(|o| o.site.file == "refine.omg"
        && o.site.span.slice(fine.as_bytes()) == Some(b"use_it::<Quiet>()")
        && matches!(&o.kind, omega_typer::ObligationKind::RefineEvidence {binder,implementation}
            if binder.declaration.path == "Local" && implementation.path == "Quiet")));
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

#[test]
fn duplicate_rows_are_checked_after_selecting_the_requirement_overload() {
    for extra in ["", "machine write(x:bool);"] {
        let code = format!(
            "trait Logger{{machine write(x:u8); {extra}}} machine write_log(x:u8){{}} Sink satisfies Logger{{Logger::write=write_log; Logger::write=write_log;}}"
        );
        let err = check(&[("rows.omg", &code)]).unwrap_err();
        assert_eq!(err.message, "requirement is supplied twice");
        assert_eq!(err.span.get_start_offset(), code.rfind("Logger::write").unwrap());
    }
}

#[test]
fn trait_applications_check_the_same_arguments_in_every_type_position() {
    let definitions = "data Byte=u8; public boundary trait Marker<T:core::Type<1>> {}";
    for usage in [
        "Supplied satisfies ARG {}",
        "boundary trait Derived:ARG {}",
        "machine f<const E:ARG>() {}",
        "machine f(x:ARG) {}",
        "machine f() reaches ARG {}",
        "machine f() invokes ARG {}",
        "data Schema=ARG;",
    ] {
        let compile = |arg: &str| {
            let usage = usage.replace("ARG", arg);
            check(&[("definitions.omg", definitions), ("use.omg", &usage)])
        };
        for valid in ["Marker<Byte>", "Marker<core::Type<0>>"] {
            let stage = compile(valid).unwrap_or_else(|e| panic!("{usage} / {valid}: {e:?}"));
            assert!(
                stage
                    .obligations
                    .iter()
                    .any(|o| o.site.file == "use.omg" && matches!(o.kind, omega_typer::ObligationKind::UniverseMembership { .. })),
                "{usage} / {valid}"
            );
        }
        for (arg, reason) in [
            ("Marker", "expects 1 trait arguments"),
            ("Marker<u8,u8>", "expects 1 trait arguments"),
            ("Marker<core::Type<1>>", "exact successor universe"),
        ] {
            let error = compile(arg).unwrap_err();
            assert!(error.message.contains(reason), "{usage} / {arg}: {error:?}");
        }
    }
}

#[test]
fn inherited_operation_records_keep_the_originating_slot_through_projection() {
    let schema = "trait Base<T>{machine read(x:T)->T;} trait Child<U>:Base<bool>{} trait Other{machine read(x:bool)->bool;}";
    let code = "machine apply(x:bool)->bool{x} const action=(apply satisfies Child<u8>::read); const codec:Child<u8> = Child{read:action}; machine use_it(x:bool)->bool{codec.read(x)}";
    let run = |body: &str| check(&[("a.omg", body), ("z.omg", schema)]);
    let stage = run(code).unwrap();
    let [binding] = stage.conformance_rows.as_slice() else {
        panic!("expected one explicit binding")
    };
    assert!(binding.conformance.is_none());
    assert_eq!(binding.requirement.declaration.path, "Base::read");
    assert_eq!(binding.requirement.trait_arguments, [ValueType::Bool]);
    assert_eq!(binding.implementation.path, "apply");
    let [call] = stage.indirect_calls.as_slice() else {
        panic!("expected a call through the projected field")
    };
    assert_eq!(call.contract.requirement.as_ref(), Some(&binding.requirement));
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,omega_typer::ObligationKind::RefineRequirement{requirement,..} if requirement==&binding.requirement))
    );
    for wrong in [
        code.replace("Child<u8>::read", "Other::read"),
        code.replace("read:action", "read:apply"),
        code.replace("read:action", ".."),
        code.replace("apply(x:bool)->bool", "apply(x:u8)->u8"),
    ] {
        assert!(run(&wrong).is_err(), "{wrong}");
    }
}

#[test]
fn qualified_calls_check_the_receivers_bounds_before_typing_the_member() {
    let defs = "boundary trait Read<T:core::Type<1>>{machine read<U>(x:U)->U;}";
    let caller = "machine f(x:bool)->bool{Read::<core::Type<0>>::read::<bool>(x)}";
    let compile = |caller: &str| check(&[("schema.omg", defs), ("caller.omg", caller)]);
    let stage = compile(caller).unwrap();
    let [call] = stage.calls.as_slice() else {
        panic!("expected one boundary call")
    };
    assert_eq!(call.type_arguments, [ValueType::Bool]);
    let [(owner, arguments)] = call.enclosing_arguments.as_slice() else {
        panic!("expected the applied receiver")
    };
    assert_eq!(owner.path, "Read");
    assert!(matches!(arguments.as_slice(), [ValueType::Universe { .. }]));
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| o.site.file == "caller.omg" && matches!(o.kind, omega_typer::ObligationKind::UniverseMembership { .. }))
    );
    for (caller, reason) in [
        (caller.replace("Type<0>", "Type<1>"), "exact successor universe"),
        (caller.replace("::<core::Type<0>>", ""), "expects 1 trait arguments"),
    ] {
        let error = compile(&caller).unwrap_err();
        assert!(error.message.contains(reason), "{caller}: {error:?}");
    }
}

#[test]
fn forwarding_invocation_effects_substitutes_both_trait_and_machine_parameters() {
    use omega_typer::InvocationTarget;
    let definitions = "trait Handler<A,B>{machine enter();} boundary trait Dispatch<T>{machine run<U>() invokes Handler<T,U>;}";
    let caller = "machine outer() invokes Handler<u8,bool>{Dispatch::<u8>::run::<bool>()}";
    let stage = check(&[("defs.omg", definitions), ("calls.omg", caller)]).unwrap();
    let effect = stage
        .machine_effects
        .iter()
        .find(|e| e.machine.path == "outer")
        .unwrap();
    assert!(
        matches!(effect.invokes.as_slice(), [InvocationTarget::Declaration(s)] if s.declaration.path == "Handler" && s.trait_arguments.len() == 2 && s.trait_arguments[0].to_string() == "u8" && s.trait_arguments[1] == ValueType::Bool)
    );
    let wrong = caller.replace("Handler<u8,bool>", "Handler<bool,u8>");
    let error = check(&[("defs.omg", definitions), ("calls.omg", &wrong)]).unwrap_err();
    assert!(error.message.contains("synchronous invocation ceiling"), "{error:?}");
}

#[test]
fn a_row_selects_the_inherited_requirement_from_an_imported_module() {
    let caller = "use views; use views::Child::read; machine copy(x:numbers::Word)->numbers::Word{x} Adapter satisfies Child { Child::read = copy; }";
    let typed = check(&[
        ("main.omg", caller),
        ("numbers/types.omg", "public data Word {}"),
        ("api/base.omg", "use numbers::Word; public trait Parent {machine read(x:Word)->Word;}"),
        ("views/child.omg", "use api::Parent; public trait Child:Parent {}"),
    ])
    .unwrap();
    let [row] = typed.conformance_rows.as_slice() else {
        panic!("expected one inherited selection")
    };
    assert_eq!(row.requirement.declaration.path, "api::Parent::read");
    assert_eq!(row.requirement.declaration.signature_owners, [("game".into(), "numbers::Word".into())]);
    assert_eq!(row.implementation.path, "copy");
}

#[test]
fn a_selection_row_uses_the_machine_when_a_mathematical_definition_shares_its_name() {
    let source = "trait R {machine run(x:u8)->u8;} let mapping(x:u8):u8=x; machine mapping(x:u8)->u8{x} I satisfies R {R::run=mapping;}";
    let typed = check(&[("selected.omg", source)]).unwrap();
    let [row] = typed.conformance_rows.as_slice() else {
        panic!("expected one implementation")
    };
    assert_eq!(row.implementation.path, "mapping");
    assert_eq!(row.implementation.kind, omega_parser::NodeKind::Machine);
}
