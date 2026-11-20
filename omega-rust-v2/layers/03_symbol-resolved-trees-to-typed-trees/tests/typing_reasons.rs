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
