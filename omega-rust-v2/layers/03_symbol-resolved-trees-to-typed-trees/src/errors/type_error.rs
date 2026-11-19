use omega_lexer::SourceSpan;
use omega_resolver::IndexedSource;

#[derive(Debug, Eq, PartialEq)]
pub struct TypeError {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub message: String,
    pub(crate) waiting_for_static_value: bool,
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

    pub(crate) fn waiting(
        file: &IndexedSource<'_>,
        node: usize,
    ) -> Self {
        Self {
            waiting_for_static_value: true,
            ..Self::at(file, node, "typing needs the evaluated static argument")
        }
    }
    pub(crate) fn for_site(
        site: &crate::Site,
        message: impl Into<String>,
    ) -> Self {
        Self::new(&site.package, &site.file, site.span, message)
    }
    pub(crate) fn at(
        file: &IndexedSource<'_>,
        n: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::new(file.package, file.path, file.span(n), message)
    }
    pub(crate) fn unsupported(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self {
        Self::at(file, n, format!("unsupported L3 form {:?}", file.nodes[n].kind))
    }
}
