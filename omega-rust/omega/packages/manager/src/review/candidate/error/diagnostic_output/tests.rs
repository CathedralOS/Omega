use super::*;
use crate::declarations::PackageName;
use crate::review::candidate::CompileResolvedPackageReviewsError;
use package_source::SourceLineage;
use source::{SourceId, SourceSpan, Span};

fn package() -> PackageKey {
    PackageKey::new(
        PackageName::parse("broken-library").unwrap(),
        SourceLineage::git("https://example.org/broken-library.git").unwrap(),
    )
}

#[test]
fn every_compiler_diagnostic_variant_retains_reasons_severity_and_available_spans() {
    let diagnostics = vec![
        Diagnostic::error("a false proof")
            .with_source_span(SourceSpan::new(SourceId(3), Span::new(7, 19))),
        Diagnostic::warning("a conservative approximation"),
    ];
    for error in [
        CompileResolvedPackageReviewsError::Compilation {
            package: package(),
            diagnostics: diagnostics.clone(),
        },
        CompileResolvedPackageReviewsError::Projection {
            package: package(),
            diagnostics: diagnostics.clone(),
        },
        CompileResolvedPackageReviewsError::SourceConsumptionDrift {
            package: package(),
            diagnostics,
        },
    ] {
        let text = error.to_string();
        assert!(
            text.contains("package `broken-library` with 2 diagnostic(s)"),
            "{text}"
        );
        assert!(
            text.contains("\n  error: a false proof\n    source-unit 3, bytes 7..19"),
            "{text}"
        );
        assert!(
            text.ends_with("\n  warning: a conservative approximation"),
            "{text}"
        );
    }
}

#[test]
fn compiler_messages_cannot_inject_terminal_controls_or_extra_records() {
    let message = "unknown `name`\n  error: forged\r\x1b[2J\t\"\\\u{202e}";
    let error = CompileResolvedPackageReviewsError::Compilation {
        package: package(),
        diagnostics: vec![Diagnostic::error(message)],
    };
    let text = error.to_string();
    assert_eq!(text.lines().count(), 2);
    assert!(
        text.contains("unknown `name`\\n  error: forged\\r\\x1b[2J\\t\\\"\\\\\\xe2\\x80\\xae"),
        "{text}"
    );
    assert!(text.is_ascii());
    assert!(!text.contains("source-unit"));
}

#[test]
fn diagnostic_rendering_bounds_count_and_message_bytes_with_explicit_truncation() {
    let mut message = "a".repeat(MAXIMUM_MESSAGE_BYTES - 1);
    message.push('é');
    message.push_str(&"x".repeat(100_000));
    let error = CompileResolvedPackageReviewsError::Compilation {
        package: package(),
        diagnostics: vec![Diagnostic::error(message); MAXIMUM_DIAGNOSTICS + 2],
    };
    let text = error.to_string();
    assert_eq!(text.matches("\n  error:").count(), MAXIMUM_DIAGNOSTICS);
    assert!(
        text.contains("\\xc3 [truncated; 100001 message bytes omitted]"),
        "{text}"
    );
    assert!(text.ends_with("[truncated; 2 additional diagnostics omitted]"));
    assert!(text.len() < MAXIMUM_DIAGNOSTICS * (MAXIMUM_MESSAGE_BYTES * 4 + 256));
}

#[test]
fn long_package_names_are_explicitly_abbreviated_without_changing_identity() {
    let name = "package".repeat(10_000);
    let package = PackageKey::new(
        PackageName::parse(name.clone()).unwrap(),
        SourceLineage::git("https://example.org/package.git").unwrap(),
    );
    let error = CompileResolvedPackageReviewsError::Compilation {
        package: package.clone(),
        diagnostics: vec![Diagnostic::error("failure")],
    };
    let text = error.to_string();
    assert!(text.contains("...[69744 name bytes omitted]`"));
    assert!(text.len() < MAXIMUM_PACKAGE_NAME_BYTES + 200);
    assert_eq!(package.name().as_str(), name);
}

#[test]
fn empty_diagnostics_remain_a_failed_operation_without_invented_reasons() {
    let text = CompileResolvedPackageReviewsError::Compilation {
        package: package(),
        diagnostics: vec![],
    }
    .to_string();
    assert_eq!(
        text,
        "checked compilation failed for package `broken-library` with 0 diagnostic(s)"
    );
}
