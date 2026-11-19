#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct PauseEffects {
    pub suspend: bool,
    pub block: bool,
}
