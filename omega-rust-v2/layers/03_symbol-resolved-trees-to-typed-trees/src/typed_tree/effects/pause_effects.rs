#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct PauseEffects {
    pub suspend: bool,
    pub block: bool,
}

/// A missing bound can be inferred when the declaration permits it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredPauseBounds {
    pub suspend: Option<bool>,
    pub block: Option<bool>,
}
impl DeclaredPauseBounds {
    pub fn declared(self) -> PauseEffects {
        PauseEffects {
            suspend: self.suspend == Some(true),
            block: self.block == Some(true),
        }
    }
}
