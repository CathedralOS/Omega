#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStorageRegion {
    #[default]
    Machine,
    RuntimeFrame,
}
