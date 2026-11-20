use omega_typed::{Site, TypedTree};

#[derive(Debug, Eq, PartialEq)]
pub struct LoweringError {
    pub site: Site,
    pub message: String,
}

impl LoweringError {
    pub(crate) fn new(
        site: &Site,
        message: impl Into<String>,
    ) -> Self {
        Self {
            site: site.clone(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LoweringError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}: {}", self.site.file, self.message)
    }
}

impl std::error::Error for LoweringError {}

pub(crate) fn site_at(
    tree: &TypedTree<'_>,
    node: usize,
) -> Site {
    let file = tree.resolved().file;
    Site {
        package: file.package.into(),
        file: file.path.into(),
        span: file.span(node),
    }
}
