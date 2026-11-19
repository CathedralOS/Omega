use omega_lexer::SourceSpan;
use omega_resolver::IndexedSource;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Site {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
}
impl Site {
    pub(crate) fn at(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self {
        Self {
            package: file.package.into(),
            file: file.path.into(),
            span: file.span(n),
        }
    }
}
