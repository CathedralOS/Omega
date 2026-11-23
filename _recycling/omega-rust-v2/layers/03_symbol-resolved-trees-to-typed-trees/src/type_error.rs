use crate::TypeError;
use omega_resolved::IndexedSource;

pub(crate) trait SourceTypeError: Sized {
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    fn at(
        file: &IndexedSource<'_>,
        n: usize,
        message: impl Into<String>,
    ) -> Self;
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    fn for_site(
        site: &crate::Site,
        message: impl Into<String>,
    ) -> Self;
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    fn unsupported(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self;
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    fn waiting(
        file: &IndexedSource<'_>,
        node: usize,
    ) -> Self;
}

impl SourceTypeError for TypeError {
    fn at(
        file: &IndexedSource<'_>,
        n: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::new(file.package, file.path, file.span(n), message)
    }
    fn for_site(
        site: &crate::Site,
        message: impl Into<String>,
    ) -> Self {
        Self::new(&site.package, &site.file, site.span, message)
    }
    fn unsupported(
        file: &IndexedSource<'_>,
        n: usize,
    ) -> Self {
        Self::at(file, n, format!("unsupported L3 form {:?}", file.nodes[n].kind))
    }
    fn waiting(
        file: &IndexedSource<'_>,
        node: usize,
    ) -> Self {
        Self::at(file, node, "typing needs the evaluated static argument").mark_waiting_for_static_value()
    }
}
