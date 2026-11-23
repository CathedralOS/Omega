mod support;

#[test]
fn trait_arity_and_universe_formation_have_distinct_refusal_sites() {
    let schema = ("schema.omg", r"public boundary trait Marker<T:core\Type<1>> {}");
    let check = |source: &str| support::check(&[schema, ("use.omg", source)]);
    check(r"Supplied satisfies Marker<core\Type<0>> {}").unwrap();

    let refuse = |selection: &str, message: &str| {
        let source = format!("Supplied satisfies {selection} {{}}");
        let error = check(&source).unwrap_err();
        let start = "Supplied satisfies ".len();
        let end = start + selection.len();
        assert_eq!((&*error.package, &*error.file), ("game", "use.omg"));
        assert_eq!(error.span, omega_lexer::SourceSpan::new(start, end));
        assert_eq!(error.span.slice(source.as_bytes()), Some(selection.as_bytes()));
        assert_eq!(error.message, message);
        assert!(!error.waiting_for_static_value());
        #[cfg(feature = "diagnostic-origins")]
        support::report_return(&error, "", Some(("game", "use.omg", start, end, message)));
        error
    };
    let _arity = refuse("Marker", "Marker expects 1 trait arguments, found 0");
    let _formation = refuse(r"Marker<core\Type<1>>", "universe argument requires its exact successor universe");
    #[cfg(feature = "diagnostic-origins")]
    assert_ne!(_arity.diagnostic_origin(), _formation.diagnostic_origin());
}
