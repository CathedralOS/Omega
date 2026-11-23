use omega_lexer::SourceSpan;
use omega_resolver::{NameError, NameErrorKind};

pub fn assert_refusal(error: &NameError, kind: NameErrorKind, package: &str, file: &str, span: SourceSpan, reason: &str) {
    spec_corpus::assert_resolver_refusal!(error, kind, package, file, span, reason);
}
