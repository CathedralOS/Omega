mod support;
use support::check;

#[test]
fn open_alias_admits_a_static_selection_with_inferred_effects() {
    let source = "
        data Predicate<T> = machine(value:T)->bool;
        data Filter<T> = Predicate<T>;
        machine keep(value:u8)->bool crashes Trap value == 0 {true}
        machine matches<T,const Accept:Filter<T>>(value:T)->bool {Accept(value)}
        machine filter(value:u8)->bool {matches::<u8,keep>(value)}
    ";
    let facts = check(&[("filter.omg", source)]).unwrap();
    let [call] = &facts.indirect_calls[..] else {
        panic!("expected the static callback call")
    };
    assert!(call.contract.reach.is_none());
    assert!(call.contract.crash_ceiling.is_none());
}

#[test]
fn runtime_storage_rejects_the_alias_use_and_names_the_missing_ceiling() {
    for (clause, missing) in [
        ("", "explicit reach and crash ceilings"),
        ("reaches none", "an explicit crash ceiling"),
        ("crashes none", "an explicit reach ceiling"),
    ] {
        let callbacks = format!("public data Predicate<T> = machine(value:T)->bool {clause};");
        let source = "use callbacks\\Predicate; data Stored<'a> { test:@'a Predicate<u8>; }";
        let error = check(&[
            ("callbacks/predicates.omg", &callbacks),
            ("filters.omg", source),
        ])
        .unwrap_err();
        assert_eq!(error.message, format!("runtime callable requires {missing}"));
        assert_eq!(error.file, "filters.omg");
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"Predicate<u8>".as_slice()));
    }
}

#[test]
fn substituting_an_open_alias_cannot_create_a_runtime_callback() {
    let source = "
        data Predicate = machine(value:u8)->bool;
        machine apply<T>(value:T)->T {value}
        machine bad<const Accept:Predicate>() { _ = apply::<Predicate>(Accept); }
    ";
    let error = check(&[("apply.omg", source)]).unwrap_err();
    assert!(error.message.contains("explicit reach and crash ceilings"), "{error:?}");
}

#[test]
fn closed_aliases_preserve_explicit_outer_type_and_lifetime_parameters() {
    let alias = "public data Reader<'a,T> = machine(value:@'a T)->@'a T reaches none crashes none;";
    let source = "use callbacks\\Reader; machine read<'a>(load:Reader<'a,u8>, value:@'a u8)->@'a u8 { load(value) }";
    let resolved = support::resolve_packages(&[("game", "reader.omg", source), ("game", "callbacks/definitions.omg", alias)], &[]);
    let typed = omega_typer::type_stage(&resolved).unwrap();
    let [call] = typed.indirect_calls.as_slice() else { panic!("expected the callback call") };
    let (tree, node) = typed.callable_origin(&call.contract.contract).expect("callable alias RHS");
    let file = tree.resolved().file;
    assert_eq!(file.path, "callbacks/definitions.omg");
    assert_eq!(file.nodes[node].kind, omega_parser::NodeKind::MachineType);
    assert_eq!(file.span(node), call.contract.contract.span);
}

#[test]
fn a_static_callback_cannot_take_an_open_callback_as_a_runtime_operand() {
    let source = "
        data Predicate = machine(value:u8)->bool;
        data Apply<T> = machine(callback:T);
        machine invalid<const Run:Apply<Predicate>>() {}
    ";
    let error = check(&[("apply.omg", source)]).unwrap_err();
    assert_eq!(error.message, "runtime callable requires explicit reach and crash ceilings");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"Predicate".as_slice()));
}

#[test]
fn a_generic_local_checks_its_concrete_storage_type_through_forwarding_calls() {
    let schemas = "
        data Predicate = machine(value:u8)->bool;
        machine forward<T>() { reserve::<T>(); }
    ";
    let allowed = format!("{schemas} machine reserve<T>() {{}} machine good() {{forward::<Predicate>();}}");
    check(&[("reserve.omg", &allowed)]).unwrap();
    let refused = allowed.replace("reserve<T>() {}", "reserve<T>() {let mutable slot:T;}");
    let error = check(&[("reserve.omg", &refused)]).unwrap_err();
    assert_eq!(error.message, "runtime callable requires explicit reach and crash ceilings");
    assert_eq!(error.span.slice(refused.as_bytes()), Some(b"forward::<Predicate>()".as_slice()));
}

#[test]
fn temporary_collections_cannot_erase_an_open_static_selection() {
    let headers = "data Predicate = machine(value:u8)->bool; data Stored<T> { field:T; }";
    for expression in ["[Test]", "(Test,)", "Stored::<Predicate> {field:Test}"] {
        let source = format!("{headers} machine filter<const Test:Predicate>() {{ _ = {expression}; }}");
        let error = check(&[("filter.omg", &source)]).unwrap_err();
        assert_eq!(error.message, "runtime callable requires explicit reach and crash ceilings");
    }
}
