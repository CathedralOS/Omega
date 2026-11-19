#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Access {
    Shared,
    Mut,
    Write,
}

impl Access {
    pub(crate) fn prefix(&self) -> &'static str {
        match self {
            Self::Shared => "@",
            Self::Mut => "@mut ",
            Self::Write => "@write ",
        }
    }
}
