// Target preparation used the deleted checker's provider application custody.
// Keep the request boundary unavailable until the accepted Terminal module
// supplies genuine provider and calling plans; typed declarations alone do not.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

pub(crate) struct PreparedProductTarget {
    _unserved: (),
}

impl PreparedProductTarget {
    pub(crate) fn capture(
        _source: &CheckedProductSource,
        _profile: Option<target::TargetProfile>,
        _package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        Err(vec![Diagnostic::error(
            "no product route yet: target preparation requires route-owned provider and calling plans",
        )])
    }
}
