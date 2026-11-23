pub use omega_program_contracts::Site;
use omega_resolved::IndexedSource;
pub fn site_at(
    file: &IndexedSource<'_>,
    n: usize,
) -> Site {
    Site {
        package: file.package.into(),
        file: file.path.into(),
        span: file.span(n),
    }
}
