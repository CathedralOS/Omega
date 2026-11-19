#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallUse {
    Execution,
    Fact,
    StaticArgument,
}
