mod support;

use omega_typer::{CrashDeclarationError::*, TypingReason};

#[test]
fn declared_typing_reasons() {
    let mut names = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for cause in TypingReason::ALL {
        assert!(names.insert(cause.identity()));
        assert!(!cause.message().is_empty());
        rows.push(format!("{}\t{}", cause.identity(), spec_corpus::diagnostic_field(cause.message())));
    }
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_TYPING_REASON_INVENTORY\t{}\t{}", rows.len(), rows.join("\t"));
    }
}

#[test]
fn cleanup_effects_report_the_selection_not_the_machine() {
    use omega_typer::CleanupEffectError::{MayPause, PublishesCrashRoute};

    let owner = "data Lease dropped by finish { slot: u8; }";
    let check = |effect: &str| {
        let machine = format!("machine finish(value: Lease) {effect} {{ let Lease {{ slot as _ }} = value; }}");
        support::check(&[("lease.omg", owner), ("finish.omg", &machine)])
    };
    let stage = check("crashes none").unwrap();
    assert_eq!(stage.cleanups.len(), 1);
    assert_eq!(stage.cleanups[0].machine.path, "finish");

    for (effect, cause, message) in [
        ("crashes Trap", PublishesCrashRoute, "cleanup cannot publish a crash route"),
        ("crashes Abort", PublishesCrashRoute, "cleanup cannot publish a crash route"),
        ("blocks", MayPause, "cleanup cannot block or suspend"),
        ("suspends", MayPause, "cleanup cannot block or suspend"),
    ] {
        let error = check(effect).unwrap_err();
        assert!(!error.waiting_for_static_value());
        assert_eq!((&*error.package, &*error.file), ("game", "lease.omg"));
        assert_eq!(error.span.slice(owner.as_bytes()), Some(b"dropped by finish".as_slice()));
        assert_eq!(error.message, message);
        assert_eq!(error.reason(), Some(cause.into()));
    }
}

mod cleanup_selection {
    use omega_typer::{CleanupSelectionError::{self, *}, TypeError};
    use super::support;

    fn assert_site(error: TypeError, package: &str, file: &str, source: &str, occurrence: &str, cause: CleanupSelectionError) {
        let start = source.find(occurrence).unwrap();
        assert!(!error.waiting_for_static_value());
        assert_eq!((&*error.package, &*error.file), (package, file));
        assert_eq!(error.span, omega_lexer::SourceSpan::new(start, start + occurrence.len()));
        assert_eq!(error.reason(), Some(cause.into()));
    }

    #[test]
    fn target_and_signature_failures_name_the_head_and_clause_respectively() {
        let owner = "data R dropped by end {}";
        let check = |hook: &str| support::check(&[("owner.omg", owner), ("hook.omg", hook)]);
        for hook in ["machine end(){}", "machine end(r:R,s:R){}", "machine end(r:R)->bool{true}", "machine end(r:u8){}"] {
            assert_site(check(hook).unwrap_err(), "game", "owner.omg", owner, "dropped by end", InvalidSignature);
        }
        assert_site(check("const end=1u8;").unwrap_err(), "game", "owner.omg", owner, "end", NotExactMachine);
        let stage = check("machine end(r:R){}").unwrap();
        let [selection] = stage.cleanups.as_slice() else { panic!("expected one cleanup") };
        assert_eq!(selection.owner.path, "R");
        assert_eq!(selection.machine.path, "end");
        assert!(stage.obligations.iter().any(|o| o.site == selection.site &&
            matches!(&o.kind, omega_typer::ObligationKind::CleanupContract(id) if id == &selection.machine)));
    }

    #[test]
    fn domain_cleanup_belongs_to_the_package_of_the_carrier() {
        let policy = r"use dep\R; domain R::Live dropped by end; machine end(r:R){}";
        let resolved = support::resolve_packages(
            &[("lib", "resource.omg", "public data R {}"), ("client", "policy.omg", policy)],
            &[omega_resolver::Dependency { requester: "client", alias: "dep", package: "lib" }],
        );
        assert_site(omega_typer::type_stage(&resolved).unwrap_err(), "client", "policy.omg", policy, "dropped by end", ForeignCarrier);
    }

    #[test]
    fn compatibility_errors_name_the_typed_subject_not_the_selected_hook() {
        let defs = "data R dropped by end<u8> {} domain R::Live dropped by end<u8>; machine end<T>(r:R){}";
        for (source, occurrence) in [
            ("machine f(r:R in Live){}", "R in Live"),
            ("machine f(r:R){let owned=r as +Live;}", "r as +Live"),
        ] {
            let check = |schema: &str| support::check(&[("schema.omg", schema), ("use.omg", source)]);
            let stage = check(defs).unwrap();
            let [a, b] = stage.cleanups.as_slice() else { panic!("expected carrier and domain plans") };
            assert_ne!(a.owner, b.owner);
            assert_eq!(a.machine, b.machine);
            assert_eq!(a.arguments, b.arguments);
            assert_eq!(a.arguments, [omega_typer::ValueType::Integer { signed: false, width: Some(8) }]);
            let conflict = defs.replace("Live dropped by end<u8>", "Live dropped by end<u16>");
            assert_site(check(&conflict).unwrap_err(), "game", "use.omg", source, occurrence, ConflictingSelections);
        }
    }

    #[test]
    fn indexed_cleanup_uses_the_bound_carrier_and_index() {
        let defs = "domain<T,const N:u8> T::Level<N> dropped by end<T>; machine end<U>(x:U){}";
        let check = |source: &str| support::check(&[("defs.omg", defs), ("use.omg", source)]);
        let missing_index = "machine read(x:u8 in u8::Level){}";
        let error = check(missing_index).unwrap_err();
        assert_eq!(error.file, "use.omg");
        assert_eq!(error.span.slice(missing_index.as_bytes()), Some(b"u8::Level".as_slice()));
        assert_eq!(error.message, "incorrect indexed domain arity");
        assert_eq!(error.reason(), None);
        let source = "machine read(x:u8 in u8::Level<2>){}";
        let stage = check(source).expect("the domain owns the index and infers the carrier");
        let [selection] = stage.cleanups.as_slice() else { panic!("expected the domain cleanup") };
        assert_eq!((selection.owner.path.as_str(), selection.machine.path.as_str()), ("T::Level", "end"));
    }
}

#[test]
fn resolved_crash_declarations_keep_the_clause_head_or_declaration_site() {
    let check = |source: &str| support::check(&[("crashes.omg", source)]);
    let source = |clause: &str| format!("data Other {{}} machine f() {clause} {{}}");
    for clause in ["crashes none", "crashes Trap true", "crashes Abort"] {
        check(&source(clause)).unwrap();
    }
    for (clause, occurrence, cause, text) in [
        ("crashes none true", "crashes none true", GuardOnNone, "crashes none cannot carry a route guard"),
        ("crashes Other", "Other", UnknownCause, "crash clause requires Trap, Abort or none"),
        ("crashes Abort crashes none", "machine f() crashes Abort crashes none {}", NoneWithDeclaredCause, "crashes none conflicts with a declared cause"),
    ] {
        let source = source(clause);
        let error = check(&source).unwrap_err();
        let start = source.rfind(occurrence).unwrap();
        assert!(!error.waiting_for_static_value());
        assert_eq!((&*error.package, &*error.file), ("game", "crashes.omg"));
        assert_eq!(error.span, omega_lexer::SourceSpan::new(start, start + occurrence.len()));
        assert_eq!(error.message, text);
        assert_eq!(error.reason(), Some(cause.into()));
    }
}
