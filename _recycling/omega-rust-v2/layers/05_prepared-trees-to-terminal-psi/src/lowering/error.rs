use omega_typed::{Site, TypedTree};

#[derive(Debug)]
#[cfg_attr(not(feature = "diagnostic-origins"), derive(Eq, PartialEq))]
pub struct LoweringError {
    pub site: Site,
    pub message: String,
    #[cfg(feature = "diagnostic-origins")]
    origin: &'static std::panic::Location<'static>,
}

impl LoweringError {
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    pub fn new(
        site: &Site,
        message: impl Into<String>,
    ) -> Self {
        Self {
            site: site.clone(),
            message: message.into(),
            #[cfg(feature = "diagnostic-origins")]
            origin: std::panic::Location::caller(),
        }
    }

    /// Compiler construction site for this returned diagnostic in the measured revision.
    #[cfg(feature = "diagnostic-origins")]
    pub fn diagnostic_origin(&self) -> &'static std::panic::Location<'static> {
        self.origin
    }
}

#[cfg(feature = "diagnostic-origins")]
impl PartialEq for LoweringError {
    fn eq(&self, other: &Self) -> bool {
        self.site == other.site && self.message == other.message
    }
}

#[cfg(feature = "diagnostic-origins")]
impl Eq for LoweringError {}

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
