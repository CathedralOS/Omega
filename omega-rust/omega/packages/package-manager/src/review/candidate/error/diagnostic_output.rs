//! Bounded compiler reasons for ordinary operation errors, never review decisions.

use crate::declarations::PackageKey;
use diagnostics::Diagnostic;
pub(crate) use diagnostics::write_bounded_diagnostics as render_diagnostics;
use std::fmt;

const MAXIMUM_PACKAGE_NAME_BYTES: usize = 256;

pub(super) fn render(
    formatter: &mut fmt::Formatter<'_>,
    phase: &str,
    package: &PackageKey,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    // PackageName admits only ASCII snake_case, not arbitrary terminal text.
    let name = package.name().as_str();
    let shown_name = name.len().min(MAXIMUM_PACKAGE_NAME_BYTES);
    write!(
        formatter,
        "{phase} failed for package `{0}",
        &name[..shown_name]
    )?;
    if shown_name != name.len() {
        write!(
            formatter,
            "...[{} name bytes omitted]",
            name.len() - shown_name
        )?;
    }
    write!(formatter, "` with {} diagnostic(s)", diagnostics.len())?;
    render_diagnostics(formatter, diagnostics)
}
