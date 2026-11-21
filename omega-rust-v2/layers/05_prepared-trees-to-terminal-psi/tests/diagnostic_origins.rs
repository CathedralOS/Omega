#![cfg(feature = "diagnostic-origins")]

#[allow(dead_code)]
#[path = "support/source.rs"]
mod source;

/// Return the origin only after checking the public refusal at its authored occurrence.
fn refusal(
    text: &str,
    start: usize,
    end: usize,
    reason: &str,
) -> &'static std::panic::Location<'static> {
    let files = [("main.omg", text)];
    let package = source::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&package).expect("must be an L5 input");
    let error = omega_lowering::lower_package(&prepared).unwrap_err();
    assert_eq!(error.site.package, "app");
    assert_eq!(error.site.file, "main.omg");
    assert_eq!(error.site.span, omega_lexer::SourceSpan::new(start, end));
    assert_eq!(error.message, reason);
    assert_eq!(error.to_string(), format!("main.omg: {reason}"));
    error.diagnostic_origin()
}

#[test]
fn identical_open_template_messages_retain_distinct_decisions() {
    // A type binder's formal has no scalar template operand; an array const
    // binder has no template binder form and no callable bound for its slot.
    // These branches used to share one `bad` closure.
    let reason = "open generic machine requires a closed application before lowering";
    // Equal byte extents in the same logical file make even Site equality uninformative.
    let type_only = "machine relay<T>(x:T)->T{               x}";
    let array = "machine relay<const N:[u8;2]>(x:u8)->u8{x}";
    assert_eq!(type_only.len(), array.len());
    let a = refusal(type_only, 0, type_only.len(), reason);
    let b = refusal(array, 0, array.len(), reason);

    for origin in [a, b] {
        assert!(
            origin
                .file()
                .replace('\\', "/")
                .ends_with("src/declarations/machine_templates.rs")
        );
        assert!(origin.line() > 0 && origin.column() > 0);
    }
    assert_ne!(a, b);
}

#[test]
fn a_carrier_failure_is_located_where_it_becomes_a_lowering_refusal() {
    // Mutable byte views have a separate carrier; scalar element reads now lower.
    let text = "machine get(items:@mutable [u8],i:u64)->u8 { items[i] }";
    let origin = refusal(text, 12, 17, "no Psi lowering for carrier @mutable [u8]");
    assert!(
        origin
            .file()
            .replace('\\', "/")
            .ends_with("src/lowering/package.rs")
    );
}

#[test]
fn unsupported_node_preserves_its_callers_origin_through_package_lowering() {
    // Multiple literal tests are still refused; a compound binding use is not.
    let repaired = "data Choice {case A(code:u8, value:u8); case B;} machine first(x:@Choice)->u8 {match x {Choice::A {code:1u8, value} -> value +% 1u8, _ -> 0u8,}}";
    let text = repaired.replace("value} -> value +% 1u8", "value:2u8} -> 4u8");
    let field = source::unique_span(&text, "value:2u8");
    let origin = refusal(&text, field.start, field.end, "payload requires one literal test and direct value bindings: FieldPattern");
    assert!(
        origin
            .file()
            .replace('\\', "/")
            .ends_with("src/machines/branches/value_patterns.rs")
    );
    source::lower(&[("main.omg", repaired)]).expect("the compound payload binding lowers");
}

#[test]
fn construction_location_does_not_change_the_returned_diagnostic() {
    let site = omega_typed::Site {
        package: "app".into(),
        file: "main.omg".into(),
        span: omega_lexer::SourceSpan::new(4, 8),
    };
    let first = omega_lowering::LoweringError::new(&site, "missing predicate");
    let second = omega_lowering::LoweringError::new(&site, "missing predicate");
    assert_ne!(first.diagnostic_origin(), second.diagnostic_origin());
    assert_eq!(first, second);
}
