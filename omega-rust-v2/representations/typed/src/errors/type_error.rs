use omega_tokens::SourceSpan;

#[derive(Debug)]
#[cfg_attr(not(feature = "diagnostic-origins"), derive(Eq, PartialEq))]
pub struct TypeError {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub message: String,
    reason: Option<crate::TypingReason>,
    waiting_for_static_value: bool,
    #[cfg(feature = "diagnostic-origins")]
    origin: &'static std::panic::Location<'static>,
}

// Readiness tests compare the same diagnostic across compiler interfaces. The Rust
// site where it was constructed is test evidence and does not change its meaning.
#[cfg(feature = "diagnostic-origins")]
impl PartialEq for TypeError {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        (
            self.package.as_str(),
            self.file.as_str(),
            self.span,
            self.message.as_str(),
            self.reason,
            self.waiting_for_static_value,
        ) == (
            other.package.as_str(),
            other.file.as_str(),
            other.span,
            other.message.as_str(),
            other.reason,
            other.waiting_for_static_value,
        )
    }
}

#[cfg(feature = "diagnostic-origins")]
impl Eq for TypeError {}

impl TypeError {
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    pub fn new(
        package: impl Into<String>,
        file: impl Into<String>,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> Self {
        Self {
            package: package.into(),
            file: file.into(),
            span,
            message: message.into(),
            reason: None,
            waiting_for_static_value: false,
            #[cfg(feature = "diagnostic-origins")]
            origin: std::panic::Location::caller(),
        }
    }

    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    pub fn for_reason(
        site: &crate::Site,
        cause: impl Into<crate::TypingReason>,
    ) -> Self {
        let cause = cause.into();
        let mut error = Self::new(&site.package, &site.file, site.span, cause.message());
        error.reason = Some(cause);
        error
    }

    /// The decision returned by an inventoried L3 check, never inferred from diagnostic text.
    pub fn reason(&self) -> Option<crate::TypingReason> {
        self.reason
    }

    #[cfg(feature = "diagnostic-origins")]
    pub fn diagnostic_origin(&self) -> &'static std::panic::Location<'static> {
        self.origin
    }

    pub fn waiting_for_static_value(&self) -> bool {
        self.waiting_for_static_value
    }

    pub fn mark_waiting_for_static_value(mut self) -> Self {
        self.waiting_for_static_value = true;
        self
    }

    pub fn with_span(
        mut self,
        span: SourceSpan,
    ) -> Self {
        self.span = span;
        self
    }
}

#[cfg(test)]
#[test]
fn direct_call_pause_identity_is_not_derived_from_text_or_span() {
    let site = crate::Site { package: "app".into(), file: "main.omg".into(), span: SourceSpan::new(2, 6) };
    let cause = crate::DirectCallPauseError::ExceedsSuspensionCeiling;
    let typed = TypeError::for_reason(&site, cause);
    let plain = TypeError::new(&site.package, &site.file, site.span, &typed.message);
    assert_eq!(plain.reason(), None);
    assert_ne!(typed, plain);
    assert_eq!(typed.with_span(SourceSpan::new(9, 13)).reason(), Some(cause.into()));
}

#[cfg(all(test, feature = "diagnostic-origins"))]
#[test]
fn observing_the_constructor_preserves_diagnostic_equality_and_span_changes() {
    let span = SourceSpan::new(1, 3);
    let first = TypeError::new("app", "main.omg", span, "bad type");
    let second = TypeError::new("app", "main.omg", span, "bad type");
    assert_ne!(first.diagnostic_origin(), second.diagnostic_origin());
    assert_eq!(first, second);
    let origin = first.diagnostic_origin();
    let moved = first.with_span(SourceSpan::new(2, 3));
    assert_eq!(moved.diagnostic_origin(), origin);
    assert_ne!(moved, second);
    assert_ne!(second.mark_waiting_for_static_value(), TypeError::new("app", "main.omg", span, "bad type"));
}
