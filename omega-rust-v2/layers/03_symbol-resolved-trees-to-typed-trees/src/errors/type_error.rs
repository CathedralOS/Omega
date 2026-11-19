use omega_lexer::SourceSpan;
use omega_resolver::IndexedSource;

#[derive(Debug, Eq, PartialEq)]
pub struct TypeError {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub message: String,
}
impl TypeError {
    pub(crate) fn for_site(
        site: &crate::Site,
        message: impl Into<String>,
    ) -> Self {
        Self {
            package: site.package.clone(),
            file: site.file.clone(),
            span: site.span,
            message: message.into(),
        }
    }
    pub(crate) fn at(
        file: &IndexedSource<'_>,
        n: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            package: file.package.into(),
            file: file.path.into(),
            span: file.span(n),
            message: message.into(),
        }
    }
    pub(crate) fn unsupported(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self {
        Self::at(file, n, format!("unsupported L3 form {:?}", file.nodes[n].kind))
    }
}
