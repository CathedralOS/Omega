use super::validate_pure_discards;

fn discard_warnings(source: &str) -> Vec<String> {
    let program = crate::front_end::typed_program(source);
    let operational = crate::infer_operational_may(&program);
    let service_reaches = crate::infer_service_reaches(&program, &operational);
    let mut diagnostics = Vec::new();
    validate_pure_discards(&program, &operational, &service_reaches, &mut diagnostics);
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .filter(|message| message.contains("discards a provably pure call"))
        .collect()
}

#[test]
fn discarding_a_requirement_call_keeps_its_published_reach() {
    // The requirement has no body to infer from; its published `Clock`
    // ceiling is the reach every call to it carries.
    let warnings = discard_warnings(
        r#"
        boundary trait Clock {}
        data Timed {}
        boundary requirement Timed::poll(timeout_milliseconds: i32) -> i32
        reaches
            Clock;
        machine wait() reaches Clock {
            _ = Timed::poll(1);
        }
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:#?}");
}

#[test]
fn discarding_a_pure_helper_still_warns() {
    let warnings = discard_warnings(
        r#"
        machine twice(value: i32 in Wrapping) -> i32 in Wrapping {
            value + value
        }
        machine caller() {
            _ = twice(1);
        }
        "#,
    );
    assert_eq!(warnings.len(), 1, "{warnings:#?}");
    assert!(warnings[0].contains("`twice`"), "{warnings:#?}");
}
