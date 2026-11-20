use omega_tokens::SourceSpan;

#[derive(Debug, Eq, PartialEq)]
pub struct TypeError {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub message: String,
    waiting_for_static_value: bool,
}
impl TypeError {
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
            waiting_for_static_value: false,
        }
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
