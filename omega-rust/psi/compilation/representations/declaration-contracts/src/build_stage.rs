/// Which stage of how many the Build entry of a typed frontier is. A Build
/// entry is divided at its top-level dependency preparations
/// (`wiki/spec/build/scoped_execution.md`, "Checked Build stages"), and a
/// frontier holds the statements of one stage, or the whole entry when it was
/// not divided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildStage {
    pub index: usize,
    pub count: usize,
}

impl BuildStage {
    /// The whole entry: no preparation divides it, or nothing divided it.
    pub const WHOLE: Self = Self { index: 0, count: 1 };

    pub fn is_last(self) -> bool {
        self.index + 1 == self.count
    }
}
