/// Target-realization storage roots shared by lowering, object planning, and
/// artifact reporting.
///
/// This is not a source place or terminal-Psi place identity. It only
/// distinguishes the two native storage roots already selected by Omega.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStorageRegion {
    #[default]
    Machine,
    RuntimeFrame,
}
