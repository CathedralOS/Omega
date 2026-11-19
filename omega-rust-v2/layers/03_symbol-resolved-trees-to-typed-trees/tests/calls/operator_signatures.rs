// Three type positions have five distinct patterns of binder reuse. Two declarations
// are duplicates exactly when they use the same pattern, even through an alias.
#[test]
fn generated_named_and_operator_signatures_agree_on_binder_reuse() {
    let patterns = [
        ("A", ["A", "A", "A"], "X", ["X", "X", "X"]),
        ("A,B", ["A", "A", "B"], "Y,X", ["X", "X", "Y"]),
        ("A,B", ["A", "B", "A"], "Y,X", ["X", "Y", "X"]),
        ("A,B", ["A", "B", "B"], "Y,X", ["X", "Y", "Y"]),
        ("A,B,C", ["A", "B", "C"], "Z,Y,X", ["X", "Y", "Z"]),
    ];
    let alias = "data Id<T> = T;";
    let mut distinct = vec![("alias.omg".to_owned(), alias.to_owned())];
    for (i, (left_binders, [a, b, c], _, _)) in patterns.iter().enumerate() {
        for (j, (_, _, right_binders, [x, y, z])) in patterns.iter().enumerate() {
            for operator in [false, true] {
                // A fresh nominal pair keeps independent operator families apart in the batch.
                let family = format!("pair_{i}_{j}_{}", if operator { "token" } else { "named" });
                let first = format!("first_{family}");
                let second = if operator { format!("second_{family}") } else { first.clone() };
                let token = if operator { "+ " } else { "" };
                let source = format!(
                    "data {family}<L,R>{{}} machine {token}{first}<{left_binders}>(p:{family}<{a},{b}>,v:{c}){{}} machine {token}{second}<{right_binders}>(p:Id<{family}<{x},{y}>>,v:{z}){{}}"
                );
                let file = format!("{family}.omg");
                if i == j {
                    let error = check(&[("alias.omg", alias), (&file, &source)])
                        .err()
                        .unwrap_or_else(|| panic!("accepted duplicate in {file}: {source}"));
                    let expected = if operator {
                        "duplicate normalized operator binding".to_owned()
                    } else {
                        format!("duplicate typed operand signature for {first}")
                    };
                    assert_eq!(error.file, file, "{source}");
                    assert_eq!(error.message, expected, "{source}");
                } else {
                    distinct.push((file, source));
                }
            }
        }
    }
    let files = distinct
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect::<Vec<_>>();
    let stage = check(&files).unwrap_or_else(|e| {
        let source = files.iter().find(|(p, _)| *p == e.file).unwrap();
        panic!("{e:?}\n{}", source.1);
    });
    assert_eq!(stage.signatures.len(), 80); // Both declarations survived in each of the 40 distinct pairs.
}

#[test]
fn an_unqualified_operator_uses_the_normalized_operand_domains() {
    let source =
        "domain u8::Small; data Id<T> = T; data Count = Id<u8 in Small>; machine + add(x:Count,y:Count)->Count{x} machine sum(x:Count,y:Count)->Count{x+y}";
    let stage = check(&[("units.omg", source)]).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|call| call.machine.path == "add" && call.site.span.slice(source.as_bytes()) == Some(b"x+y"))
    );
    let duplicate = format!("{source} machine + plus(x:u8 in Small,y:u8 in Small)->Count{{x}}");
    assert_eq!(
        check(&[("units.omg", &duplicate)]).unwrap_err().message,
        "duplicate normalized operator binding"
    );
    let nested = "domain u8::Small; data Box<T>{v:T;} machine + join(x:Box<u8 in Small>,y:Box<u8 in Small>)->Box<u8 in Small>{x} machine sum(x:Box<u8 in Small>,y:Box<u8 in Small>)->Box<u8 in Small>{x+y}";
    assert!(
        check(&[("nested.omg", nested)])
            .unwrap()
            .calls
            .iter()
            .any(|c| c.machine.path == "join")
    );
}

#[test]
fn inferred_operator_home_distinguishes_inherited_and_independent_domains() {
    let declarations = "domain u8::P; domain u8::P::Child; domain u8::Q;";
    let child = "machine + add(x:u8 in P::Child,y:u8 in P::Child)->u8 in P::Child{x} machine sum(x:u8 in P::Child,y:u8 in P::Child)->u8 in P::Child{x+y}";
    let stage = check(&[("domains.omg", declarations), ("child.omg", child)]).unwrap();
    assert!(stage.calls.iter().any(|c| c.machine.path == "add"));
    let competing = "machine + add(x:u8 in P,y:u8 in Q)->u8 in P{x}";
    assert_eq!(
        check(&[("domains.omg", declarations), ("ambiguous.omg", competing)])
            .unwrap_err()
            .message,
        "operator declaration has multiple possible semantic homes"
    );
}

#[test]
fn cross_package_operator_selection_uses_the_resolved_visibility() {
    let caller = "use dep::Point; machine combine(a:Point,b:Point)->Point{a+b}";
    for visibility in ["public ", ""] {
        let library = format!("public data Point{{}} {visibility}machine + add(a:Point,b:Point)->Point{{a}}");
        let resolved = resolve_operator_packages("main.omg", &library, caller);
        let visible = !visibility.is_empty();
        assert_eq!(resolved.operator_declarations[0].public, visible);
        let typed = omega_typer::type_stage(&resolved);
        if visible {
            assert!(
                typed
                    .unwrap()
                    .calls
                    .iter()
                    .any(|call| call.machine.package == "lib" && call.machine.path == "add")
            );
        } else {
            let error = typed.unwrap_err();
            assert_eq!(error.package, "app");
            assert_eq!(error.span.slice(caller.as_bytes()), Some(&b"a+b"[..]));
        }
    }
}

fn resolve_operator_packages<'a>(
    library_path: &'a str,
    library: &'a str,
    caller: &'a str,
) -> omega_resolver::SymbolResolvedTrees<'a> {
    support::resolve_packages(
        &[("lib", library_path, library), ("app", "main.omg", caller)],
        &[omega_resolver::Dependency {
            requester: "app",
            alias: "dep",
            package: "lib",
        }],
    )
}

#[test]
fn explicit_and_inferred_foreign_operator_homes_are_rejected_at_the_same_boundary() {
    let declaration = "public domain i32::Km;";
    for name in ["Km::add", "add"] {
        let code = format!("use dep::units; machine + {name}(x:i32 in Km,y:i32 in Km)->i32 in Km {{x}}");
        let resolved = resolve_operator_packages("units/u.omg", declaration, &code);
        assert_eq!(resolved.operator_declarations[0].declared_home.is_some(), name == "Km::add");
        let error = omega_typer::type_stage(&resolved).unwrap_err();
        assert_eq!(error.package, "app");
        assert_eq!(error.span.slice(code.as_bytes()), Some(b"+".as_slice()));
        assert_eq!(error.message, "only the declaring package may bind an operator for this home");
    }
    let duplicate_token = "trait Add<T>{machine + add(x:T,y:T)->T;} Impl satisfies Add<u8>{machine + add(x:u8,y:u8)->u8{x}}";
    let error = check(&[("trait.omg", duplicate_token)]).unwrap_err();
    assert_eq!(error.span.get_start_offset(), duplicate_token.rfind('+').unwrap());
    assert_eq!(error.message, "a conformance supplies the trait's token and cannot bind one");
}

#[test]
fn a_directory_qualifies_an_operator_name_without_becoming_an_operand_type() {
    let source = "data Point{} machine + drawing::add(x:Point,y:Point)->Point{x} machine sum(x:Point,y:Point)->Point{x+y}";
    let typed = check(&[
        ("op.omg", source),
        ("drawing/helpers.omg", "machine helper(){}"),
    ])
    .unwrap();
    let [call] = typed.calls.as_slice() else {
        panic!("expected one operator use")
    };
    assert_eq!(call.machine.path, "drawing::add");
    assert_eq!(call.result.to_string(), "game::Point");
}

#[test]
fn named_and_token_calls_enforce_the_same_machine_application() {
    let schema = "data Box<T>{v:T;} data Handle [linear]{} machine + combine<T [copy]>(b:Box<T>,v:T)->T requires true {v}";
    for (expression, blame) in [("combine(b,v)", "combine"), ("b+v", "b+v")] {
        let bad = format!("{schema} machine f(b:Box<Handle>,v:Handle)->Handle{{{expression}}}");
        let error = check(&[("bound.omg", &bad)]).expect_err("a linear argument cannot satisfy a copy parameter");
        assert!(error.message.contains("does not satisfy Copy"), "{error:?}");
        assert_eq!(error.span.slice(bad.as_bytes()), Some(blame.as_bytes()));

        let good = format!("{schema} machine f(b:Box<u8>,v:u8)->u8{{{expression}}}");
        let stage = check(&[("bound.omg", &good)]).unwrap();
        let [call] = stage.calls.as_slice() else {
            panic!("expected exactly one selected machine")
        };
        assert_eq!(call.machine.path, "combine");
        assert_eq!(call.type_arguments.as_slice(), std::slice::from_ref(&call.result));
        assert_eq!(call.result.to_string(), "u8");
        assert_eq!(call.operands.len(), 2);
        assert!(stage.obligations.iter().any(|p| p.site == call.site
            && matches!(&p.kind,
            omega_typer::ObligationKind::CallPrecondition(machine) if machine == &call.machine)));
    }
}

#[test]
fn token_calls_substitute_operand_subjects_into_parameters_and_results() {
    let prefix = "domain<K:u8> u8::Label<K>; machine + tag(n:u8,x:u8 in Label<n>)->u8 in Label<n>{x}";
    for (args, expr, index) in [
        ("k:u8,x:u8 in Label<k>", "k+x", "k"),
        ("x:u8 in Label<2>", "2+x", "2"),
    ] {
        let good = format!("{prefix} machine f({args})->u8 in Label<{index}>{{{expr}}}");
        let stage = check(&[("subject.omg", &good)]).unwrap();
        let call = stage.calls.iter().find(|c| c.machine.path == "tag").unwrap();
        let f = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
        assert_eq!(call.result, f.result);
        let declaration = stage
            .signatures
            .iter()
            .find(|s| s.machine == call.machine)
            .unwrap();
        assert_eq!(call.instantiate(&declaration.result), f.result);
    }
    let bad = format!("{prefix} machine f(k:u8,x:u8 in Label<2>)->u8 in Label<2>{{k+x}}");
    assert!(
        check(&[("subject.omg", &bad)])
            .unwrap_err()
            .message
            .contains("no matching operator")
    );
}

#[test]
fn a_later_operand_call_preserves_the_first_operands_storage_version() {
    let schema = "data Frame{} domain<K:u8> u8::Label<K>; machine frame()->Frame{Frame{}}
        machine + mark(n:u8,b:Frame)->u8 in Label<n>{0u8 as +Label<n>}";
    let code = format!("{schema} machine f(){{let mut k:u8=1; k=2; _=k+frame();}}");
    let stage = check(&[("order.omg", &code)]).unwrap();
    let mark = stage.calls.iter().find(|c| c.machine.path == "mark").unwrap();
    let first = &mark.operands[0];
    let omega_typer::ValueType::Subject { version: Some(write), .. } = &first.subject else {
        panic!("missing captured write")
    };
    assert_eq!(write.span.slice(code.as_bytes()), Some(b"k=2;".as_slice()));
    assert_eq!(mark.result.domains()[0].arguments[0], first.subject);
}
