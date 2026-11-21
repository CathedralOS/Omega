#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Access {
    Shared,
    Mut,
    Write,
}

impl Access {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Shared => "@",
            Self::Mut => "@mutable ",
            Self::Write => "@write ",
        }
    }
}
