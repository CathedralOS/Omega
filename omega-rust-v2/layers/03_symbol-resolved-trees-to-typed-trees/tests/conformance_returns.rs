mod support;

use omega_typer::{TypeError, TypedStage};

const FILE: &str = "conformance.omg";

fn check(source: &str) -> Result<TypedStage, TypeError> {
    support::check(&[(FILE, source)])
}

// Use the last occurrence so a duplicate row is blamed on the repetition, not the original.
fn refusal(
    source: &str,
    blamed: &str,
    message: &str,
) -> TypeError {
    let error = check(source).unwrap_err();
    let start = source.rfind(blamed).expect("expected source phrase");
    let end = start + blamed.len();
    assert_eq!((&*error.package, &*error.file), ("game", FILE));
    assert_eq!(error.message, message);
    assert_eq!(error.span, omega_lexer::SourceSpan::new(start, end));
    assert!(!error.waiting_for_static_value());
    #[cfg(feature = "diagnostic-origins")]
    support::report_return(&error, "", Some(("game", FILE, start, end, message)));
    error
}

#[test]
fn missing_row_names_the_declaring_trait_not_the_conformance_target() {
    let traits = "trait Parent<A>{machine get(x:A)->A;} trait T:Parent<u8>{machine f();}";
    let run = |body: &str| format!("{traits} Impl satisfies T{{{body}}}");
    let get = "machine get(x:u8)->u8{x}";
    let f = "machine f(){}";
    let stage = check(&run(&format!("{get} {f}"))).unwrap();
    let [parent, direct] = stage.conformance_rows.as_slice() else {
        panic!("expected both rows")
    };
    assert_eq!(parent.requirement.declaration.path, "Parent::get");
    assert_eq!(parent.requirement.trait_arguments[0].to_string(), "u8");
    assert_eq!(direct.requirement.declaration.path, "T::f");
    for (row, message) in [
        (get, "missing conformance row for T::f"),
        (f, "missing conformance row for Parent::get"),
    ] {
        refusal(&run(row), &format!("Impl satisfies T{{{row}}}"), message);
    }
}

#[test]
fn row_selection_counts_compatible_pairs_before_checking_completeness() {
    let row = "T::f=g;";
    for (first, second) in [("u8", "bool"), ("bool", "u8")] {
        let schema = format!("trait T{{machine f(x:{first}); machine f(x:{second});}}");
        let machines = "machine g(x:u8){} machine g(x:bool){}";
        let assemble = |machines: &str, rows: &str| format!("{schema} {machines} Impl satisfies T{{{rows}}}");
        let _ambiguous = refusal(&assemble(machines, row), row, "ambiguous conformance row");
        let _none = refusal(&assemble("machine g(x:u32){}", row), row, "conformance row has no matching signature");
        #[cfg(feature = "diagnostic-origins")]
        assert_ne!(_ambiguous.diagnostic_origin(), _none.diagnostic_origin());

        let machines = "machine g(x:u8){} machine h(x:bool){}";
        let complete = format!("{row} T::f=h;");
        let stage = check(&assemble(machines, &complete)).unwrap();
        let [g, h] = stage.conformance_rows.as_slice() else {
            panic!("expected two selected overloads")
        };
        for (selected, implementation, operand) in [(g, "g", "u8"), (h, "h", "bool")] {
            assert_eq!(selected.implementation.path, implementation);
            let signature = stage
                .signatures
                .iter()
                .find(|s| s.machine == selected.requirement.declaration)
                .unwrap();
            assert_eq!(signature.parameters[0].1.to_string(), operand);
        }
        refusal(&assemble(machines, &format!("{complete} {row}")), row, "requirement is supplied twice");
    }
}

#[test]
fn a_value_signature_match_does_not_erase_generic_binder_kinds() {
    let schema = "trait Trait{machine f<T,const N:u64>(v:T)->T;}";
    let source = |member: &str| format!("{schema} Impl satisfies Trait{{{member}}}");
    check(&source("machine f<U,const X:u64>(v:U)->U{v}")).unwrap();
    let wrong = "machine f<U,V>(v:U)->U{v}";
    refusal(&source(wrong), wrong, "conformance row has no matching signature");
}

#[test]
fn declaration_clause_and_binding_expression_blame_their_own_signature_check() {
    let schema = "trait Base<T>{machine read(x:T)->T;} trait Child:Base<bool>{}";
    let clause = "satisfies Child::read";
    let binding = "(apply satisfies Child::read)";
    let source = |ty: &str, clause: &str| format!("{schema} machine apply(x:{ty})->{ty} {clause} {{x}} const action={binding};");
    let stage = check(&source("bool", clause)).unwrap();
    let [row] = stage.conformance_rows.as_slice() else {
        panic!("expected an expression binding")
    };
    assert!(row.conformance.is_none());
    assert_eq!(row.requirement.declaration.path, "Base::read");
    assert_eq!(row.implementation.path, "apply");
    let wrong_clause = format!("{schema} machine apply(x:u8)->u8 {clause} {{x}}");
    refusal(&wrong_clause, clause, "machine has incompatible requirement signature");
    refusal(&source("u8", ""), binding, "machine has no matching requirement signature");
}

#[test]
fn a_refinement_checks_base_rows_at_evidence_use_not_at_conformance_declaration() {
    let schema = "trait Log{machine put() blocks;} trait Local=Log{machine * blocks false;}";
    let run = |target: &str, effect: &str, caller: &str| {
        format!("{schema} Busy satisfies {target}{{machine put() {effect}{{}}}} machine use_it<const E:Local>(){{E::put()}} {caller}")
    };
    let caller = "machine f(){use_it::<Busy>()}";
    check(&run("Log", "", caller)).unwrap();
    check(&run("Log", "blocks", "")).unwrap();
    refusal(
        &run("Log", "blocks", caller),
        "use_it::<Busy>()",
        "Busy::put exceeds the refined contract of Local",
    );
    refusal(
        &run("Local", "", ""),
        "Busy satisfies Local{machine put() {}}",
        "a transparent refinement cannot be a conformance target",
    );
}
