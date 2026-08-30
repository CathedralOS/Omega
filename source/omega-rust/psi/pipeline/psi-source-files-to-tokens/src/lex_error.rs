use psi_source::Span;

pub(crate) const OUTSIDE_LEXICAL_PROFILE_MESSAGE: &str =
    "spelling is outside the current language profile";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl LexError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub(crate) fn outside_lexical_profile(span: Span) -> Self {
        Self::new(OUTSIDE_LEXICAL_PROFILE_MESSAGE, span)
    }
}
