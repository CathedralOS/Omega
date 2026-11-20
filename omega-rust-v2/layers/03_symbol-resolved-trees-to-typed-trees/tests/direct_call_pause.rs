mod support;

use omega_typer::DirectCallPauseError;

#[test]
fn same_call_and_constructor_distinguish_the_callee_effect() {
    let refuse = |effect| {
        let text = format!("machine work() {effect:8} {{}} machine f(){{work()}}");
        let error = support::check(&[("pause.omg", &text)]).unwrap_err();
        assert!(!error.waiting_for_static_value());
        assert_eq!((&*error.package, &*error.file), ("game", "pause.omg"));
        let start = text.rfind("work()").unwrap();
        assert_eq!(error.span, omega_lexer::SourceSpan::new(start, start + 6));
        error
    };
    let block = refuse("blocks");
    let suspend = refuse("suspends");
    assert_eq!(block.span, suspend.span);
    #[cfg(feature = "diagnostic-origins")]
    assert_eq!(block.diagnostic_origin(), suspend.diagnostic_origin());
    assert_eq!(block.reason(), Some(DirectCallPauseError::MissingBlockAcknowledgement.into()));
    assert_eq!(suspend.reason(), Some(DirectCallPauseError::MissingSuspendAcknowledgement.into()));
    assert_eq!(block.message, "call requires a block acknowledgement");
    assert_eq!(suspend.message, "call requires a suspend acknowledgement");
}
