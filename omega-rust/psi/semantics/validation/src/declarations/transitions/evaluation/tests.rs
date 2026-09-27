mod returns;

fn check_typed(typed: &typed_trees::TypedTrees, source: &str, accepted: bool) {
    let result = crate::validate_program(typed);
    if accepted {
        assert!(result.is_ok(), "{source}: {result:#?}");
    } else {
        let diagnostics = result.expect_err("invalidated Exact range must reject");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("overflow")
                    || diagnostic.message.contains("range")
                    || diagnostic.message.contains("implicitly converts")
                    || diagnostic.message.contains("not provably representable")
            }),
            "{source}: {diagnostics:#?}"
        );
    }
}
