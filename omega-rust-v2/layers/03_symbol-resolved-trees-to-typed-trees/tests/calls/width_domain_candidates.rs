// `u8` and `u<8>` are one carrier (numeric_values.md, width-indexed integers), and
// carrier-qualified domain lookup must match the exact resolved carrier (modules.md,
// domains.md). L2 binds every visible domain of the name on a same-family builtin
// carrier when the written or implicit carrier is applied; typing selects by the
// normalized carrier. These cases never reach an L2 refusal: `check` panics on one.

fn width_domain_atoms(
    stage: &omega_typer::TypedStage,
    machine: &str,
) -> Vec<String> {
    let signature = stage
        .signatures
        .iter()
        .find(|s| s.machine.path == machine)
        .unwrap_or_else(|| panic!("missing signature {machine}"));
    // Every fixture puts the qualified subject last, after any shadowing parameter.
    signature.parameters.last().expect("missing qualified subject")
        .1
        .domains()
        .iter()
        .map(|a| a.declaration.path.clone())
        .collect()
}

#[test]
fn compact_paths_select_the_same_domain_id_when_another_applied_width_is_visible() {
    let narrow = "domain u<8>::Ready;";
    let wide = "domain u<16>::Ready;";
    let pair = format!("{wide} {narrow}");
    let selected = |declarations: &str, usage: &str| {
        let stage = check(&[("domains.omg", declarations), ("use.omg", usage)]).unwrap();
        let signature = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
        let [domain] = signature.parameters[0].1.domains() else {
            panic!("expected one domain")
        };
        domain.declaration.clone()
    };
    let narrow_use = "machine f(x:u8 in u8::Ready){}";
    let wide_use = "machine f(x:u16 in u16::Ready){}";
    let narrow_id = selected(narrow, narrow_use);
    let wide_id = selected(wide, wide_use);
    assert_ne!(narrow_id, wide_id);
    assert_eq!(selected(&pair, narrow_use), narrow_id);
    assert_eq!(selected(&pair, wide_use), wide_id);
}

#[test]
fn explicit_operator_homes_select_by_written_width_in_either_order() {
    let narrow = "domain u<8>::Ready; machine + u8::Ready::add(a:u8 in u8::Ready,b:u8 in u8::Ready)->u8 in u8::Ready{a}";
    let wide = "domain u<16>::Ready; machine + u16::Ready::add(a:u16 in u16::Ready,b:u16 in u16::Ready)->u16 in u16::Ready{b}";
    let calls = "machine f(x:u8 in u8::Ready)->u8 in u8::Ready{x+x} machine g(y:u16 in u16::Ready)->u16 in u16::Ready{y+y}";
    for definitions in [format!("{narrow} {wide}"), format!("{wide} {narrow}")] {
        let stage = check(&[("ops.omg", &definitions), ("use.omg", calls)]).unwrap();
        for (width, path, expression) in [(8, "u8::Ready::add", "x+x"), (16, "u16::Ready::add", "y+y")] {
            // The two domain IDs have the same path `u::Ready`. Compare full IDs.
            let carrier = ValueType::Integer { signed: false, width: Some(width) };
            let (owner, _) = stage.domain_carriers.iter().find(|(_, ty)| **ty == carrier).unwrap();
            let signature = stage.signatures.iter().find(|s| s.machine.path == path).unwrap();
            assert_eq!(signature.operator.as_ref().unwrap().home.as_ref(), Some(owner));
            let call = stage
                .calls
                .iter()
                .find(|c| c.site.file == "use.omg" && c.site.span.slice(calls.as_bytes()) == Some(expression.as_bytes()))
                .unwrap();
            assert_eq!(call.machine, signature.machine);
        }
    }
}

#[test]
fn singleton_operator_home_must_match_the_written_carrier() {
    let source = "machine + u8::Ready::add(a:u8,b:u8)->u8{a}";
    let typed = check(&[("defs.omg", "domain u<8>::Ready;"), ("op.omg", source)]).unwrap();
    let [signature] = typed.signatures.as_slice() else {
        panic!("expected one operator")
    };
    let home = signature.operator.as_ref().unwrap().home.as_ref().unwrap();
    assert_eq!(home.path, "u::Ready");
    assert_eq!(typed.domain_carriers[home], ValueType::Integer { signed: false, width: Some(8) });

    let error = check(&[("defs.omg", "domain u<16>::Ready;"), ("op.omg", source)]).unwrap_err();
    assert_eq!(error.file, "op.omg");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"+".as_slice()));
    assert_eq!(error.message, "no visible domain on the written carrier");
}

#[test]
fn explicit_operator_home_does_not_fall_back_to_the_operand_domain() {
    let defs = "domain u<8>::Ready; domain u<16>::Ready;";
    let source = "machine + u32::Ready::add(a:u8 in u8::Ready,b:u8 in u8::Ready){}";
    let error = check(&[("defs.omg", defs), ("ops.omg", source)]).unwrap_err();
    assert_eq!(error.file, "ops.omg");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"+".as_slice()));
    assert_eq!(error.message, "no visible domain on the written carrier");
}

#[test]
fn explicit_operator_home_still_reports_two_matches_on_the_written_width() {
    let source = "use east; use west; machine + u8::Ready::add(a:u8,b:u8)->u8{a}";
    let error = check(&[
        ("east/defs.omg", "public domain u<8>::Ready;"),
        ("west/defs.omg", "public domain u8::Ready;"),
        ("ops.omg", source),
    ]).unwrap_err();
    assert_eq!(error.file, "ops.omg");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"+".as_slice()));
    assert_eq!(error.message, concat!(
        r"competing declarations: game\east\u::Ready, game\west\u8::Ready",
        r" (game\east\u::Ready on u, import ops.omg@4..8) (game\west\u8::Ready on u8, import ops.omg@14..18)",
    ));
}

#[test]
fn an_applied_integer_root_selects_the_domain_on_its_normalized_width() {
    // The reported case: a single domain on u8 named through u<8>.
    let stage = check(&[("domains.omg", "domain u8::Ready;"), ("use.omg", "machine f(x:u8 in u<8>::Ready){}")]).unwrap();
    assert_eq!(width_domain_atoms(&stage, "f"), ["u8::Ready"]);

    let defs = "domain u8::Ready; domain u16::Ready; domain i8::Ready; domain i16::Ready;";
    for (source, selected) in [
        ("machine f(x:u8 in u<8>::Ready){}", "u8::Ready"),
        ("machine f(x:u16 in u<16>::Ready){}", "u16::Ready"),
        ("machine f(x:u8 in u<4 + 4>::Ready){}", "u8::Ready"),
        ("machine f(x:u<16> in u<16>::Ready){}", "u16::Ready"),
        ("machine f(x:i16 in i<16>::Ready){}", "i16::Ready"),
        // Implicit carrier: the subject's normalized width selects.
        ("machine f(x:u<8> in Ready){}", "u8::Ready"),
        ("machine f(x:u<16> in Ready){}", "u16::Ready"),
        ("machine f(x:i<8> in Ready){}", "i8::Ready"),
        // Unapplied decimal spellings keep the exact carrier lookup.
        ("machine f(x:u16 in u16::Ready){}", "u16::Ready"),
        ("machine f(x:u16 in Ready){}", "u16::Ready"),
        // A value parameter or static binder spelled u keeps the builtin carrier role.
        ("machine f(u:u16, x:u8 in u<8>::Ready){}", "u8::Ready"),
        ("machine f<const u:u64>(x:u16 in u<16>::Ready){}", "u16::Ready"),
    ] {
        let stage = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert_eq!(width_domain_atoms(&stage, "f"), [selected], "{source}");
    }
    // Written result and cast forms select through the same written carrier.
    let source = "machine f(x:u16)->u16 in u<16>::Ready {x as +u<16>::Ready}";
    check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
}

#[test]
fn an_applied_integer_root_without_a_matching_width_is_refused_by_typing() {
    let defs = "domain u8::Ready; domain u16::Ready;";
    let none = "no visible domain on the written carrier";
    for (source, site, message) in [
        ("machine f(x:u32 in u<32>::Ready){}", "u<32>::Ready", none),
        ("machine f(x:u<32> in Ready){}", "Ready", none),
        // A symbolic width is not 8 or 16. Typing refuses it; L2 did not need a literal.
        ("machine f<const N:u64>(x:u<N> in u<N>::Ready) requires N > 0 {}", "u<N>::Ready", none),
        ("machine f(x:u8 in u<16>::Ready){}", "u<16>::Ready", "domain qualifier names u16, found u8"),
        ("machine f(x:u8 in u<0>::Ready){}", "0", "integer width must be positive"),
        ("machine f(x:u8 in u8<1>::Ready){}", "<1>", "builtin type u8 does not take arguments"),
    ] {
        let error = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_err();
        assert_eq!(error.file, "use.omg", "{source}: {error:?}");
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()), "{source}: {error:?}");
        assert_eq!(error.message, message, "{source}");
    }
}

#[test]
fn a_family_found_parent_domain_is_the_selected_domains_ancestor() {
    let defs = "domain<const K:u8> u8::Limit<K>; domain<const K:u16> u16::Limit<K>; domain u8::Limit<2>::Pair; domain u16::Limit<2>::Pair;";
    for (source, selected) in [
        ("machine f(x:u8 in u<8>::Limit<2>::Pair){}", "u8::Limit::Pair"),
        ("machine f(x:u16 in u<16>::Limit<2>::Pair){}", "u16::Limit::Pair"),
        ("machine f(x:u<16> in Limit<2>::Pair){}", "u16::Limit::Pair"),
    ] {
        let stage = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert!(width_domain_atoms(&stage, "f").iter().any(|path| path == selected), "{source}");
    }
    let source = "machine f(x:u8 in u<8>::Limit<3>::Pair){}";
    let error = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"3".as_slice()), "{error:?}");
    assert!(error.message.starts_with("parent domain u8::Limit expects "), "{error:?}");
}

#[test]
fn applied_and_decimal_width_spellings_form_one_overload_identity() {
    let defs = "domain u8::Ready; domain u16::Ready;";
    let duplicate = "machine r(x:u8 in u8::Ready){} machine r(x:u8 in u<8>::Ready){}";
    let error = check(&[("domains.omg", defs), ("use.omg", duplicate)]).unwrap_err();
    assert_eq!(error.file, "use.omg", "{error:?}");
    assert_eq!(error.message, "duplicate typed operand signature for r");
    let implicit = "machine r(x:u<8> in Ready){} machine r(x:u8 in u8::Ready){}";
    let error = check(&[("domains.omg", defs), ("use.omg", implicit)]).unwrap_err();
    assert_eq!(error.message, "duplicate typed operand signature for r");
    let distinct = "machine r(x:u8 in u<8>::Ready){} machine r(x:u16 in u<16>::Ready){}";
    check(&[("domains.omg", defs), ("use.omg", distinct)]).unwrap();
}

#[test]
fn width_domain_candidates_keep_package_visibility_and_public_signature_authority() {
    let edge = omega_resolver::Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    };
    let library = ("lib", "units/defs.omg", "public domain u16::Ready; domain u32::Ready;");
    let type_app = |source: &'static str| {
        let resolved = support::resolve_packages(&[library, ("app", "main.omg", source)], std::slice::from_ref(&edge));
        omega_typer::type_stage(&resolved).map(|trees| trees.into_facts())
    };
    let stage = type_app("use dep\\units; domain u8::Ready; machine f(x:u16 in u<16>::Ready){}").unwrap();
    let signature = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
    let [domain] = signature.parameters[0].1.domains() else {
        panic!("expected one domain")
    };
    assert_eq!((domain.declaration.package.as_str(), domain.declaration.path.as_str()), ("lib", "units\\u16::Ready"));
    // The private foreign domain is not a candidate, so no visible domain carries u32.
    let source = "use dep\\units; domain u8::Ready; machine f(x:u32 in u<32>::Ready){}";
    let error = type_app(source).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"u<32>::Ready".as_slice()), "{error:?}");
    assert_eq!(error.message, "no visible domain on the written carrier");

    // A public signature may see a private same-family candidate, but must not select it.
    let defs = "public domain u8::Ready; domain u16::Ready;";
    check(&[("domains.omg", defs), ("use.omg", "public machine f(x:u8 in u<8>::Ready){}")]).unwrap();
    let source = "public machine f(x:u16 in u<16>::Ready){}";
    let error = check(&[("domains.omg", defs), ("use.omg", source)]).unwrap_err();
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"u<16>::Ready".as_slice()), "{error:?}");
    assert_eq!(error.message, "public signature exposes private declaration u::Ready");
}

/// Every ordering of `items`, each item exactly once.
fn width_domain_permutations<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for (i, &first) in items.iter().enumerate() {
        let mut rest = items.to_vec();
        rest.remove(i);
        for mut tail in width_domain_permutations(&rest) {
            tail.insert(0, first);
            all.push(tail);
        }
    }
    all
}

#[test]
fn a_width_domain_alias_depends_only_on_its_selected_candidate() {
    // Two independent aliases on different carriers. Each alias's candidate set holds the
    // other alias's domain, but typing selects by carrier, so neither is a dependency.
    let declarations = [
        "domain u8::Base;",
        "domain u16::Ready;",
        "domain u8::Ready = u<8>::Base;",
        "domain u16::Base = u<16>::Ready;",
        // An inferred carrier crosses the same pair from a data alias.
        "data Id<T> = T; data Narrow = Id<u<8> in Ready>;",
    ];
    let user = "machine f(x:u8 in Ready){} machine g(x:u16 in Base){} machine h(x:u16 in u<16>::Base){} machine k(x:Narrow){}";
    fn check_order(files: &[(&str, &str)]) {
        let stage = check(files).unwrap_or_else(|error| panic!("{files:?}: {error:?}"));
        assert_eq!(width_domain_atoms(&stage, "f"), ["u8::Base"], "{files:?}");
        assert_eq!(width_domain_atoms(&stage, "g"), ["u16::Ready"], "{files:?}");
        assert_eq!(width_domain_atoms(&stage, "h"), ["u16::Ready"], "{files:?}");
        assert_eq!(width_domain_atoms(&stage, "k"), ["u8::Base"], "{files:?}");
    }
    // The reported input (target/diagnostic_application_owner/alias_order.omg), extended.
    check_order(&[(
        "alias_order.omg",
        "domain u8::Base; domain u16::Ready; domain u8::Ready = u<8>::Base; domain u16::Base = u<16>::Ready; machine f(x:u8 in Ready){} machine g(x:u16 in Base){} machine h(x:u16 in u<16>::Base){} machine k(x:Narrow){} data Id<T> = T; data Narrow = Id<u<8> in Ready>;",
    )]);
    let names = ["a.omg", "b.omg", "c.omg", "d.omg", "e.omg"];
    for order in width_domain_permutations(&[0, 1, 2, 3, 4]) {
        // Within one file, and with each declaration in its own file in that order.
        let joined = order.iter().map(|&i| declarations[i]).collect::<Vec<_>>().join(" ");
        check_order(&[("domains.omg", joined.as_str()), ("use.omg", user)]);
        let mut files: Vec<_> = order.iter().zip(names).map(|(&i, name)| (name, declarations[i])).collect();
        files.push(("use.omg", user));
        check_order(&files);
    }
}

#[test]
fn a_genuine_width_domain_alias_cycle_is_still_refused() {
    let cycles: [&[&str]; 3] = [
        // Through family selection on both sides; the u16 domains are unselected candidates.
        &["domain u8::A = u<8>::B;", "domain u8::B = u<8>::A;", "domain u16::A;", "domain u16::B;"],
        // A selected self alias.
        &["domain u8::A = u<8>::A;", "domain u16::A;"],
        // A mixed cycle: exact on one side, family-selected on the other.
        &["domain u8::A = u8::B;", "domain u8::B = u<8>::A;", "domain u16::A = u<16>::B;", "domain u16::B;"],
    ];
    for declarations in cycles {
        for order in width_domain_permutations(&(0..declarations.len()).collect::<Vec<_>>()) {
            let source = order.iter().map(|&i| declarations[i]).collect::<Vec<_>>().join(" ");
            let error = check(&[("domains.omg", source.as_str())]).unwrap_err();
            assert_eq!(error.message, "cyclic domain alias", "{source}: {error:?}");
            let site = error.span.slice(source.as_bytes()).map(|s| String::from_utf8_lossy(s).into_owned());
            assert!(matches!(site.as_deref(), Some("u8::A" | "u8::B")), "{source}: {error:?}");
        }
    }
}
