#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Access {
    Shared,
    Mut,
    Write,
}

impl Access {
    /// Following a stored reference reads its carrier under the outer loan.
    pub(crate) fn through(
        &self,
        outer: Option<&Access>,
    ) -> Result<Self, &'static str> {
        match (outer, self) {
            (Some(Self::Write), _) => Err("cannot read through a write-only reference"),
            (Some(Self::Shared), Self::Write) => Err("place requires writable authority"),
            (Some(Self::Shared), _) => Ok(Self::Shared),
            _ => Ok(self.clone()),
        }
    }

    pub(crate) fn prefix(&self) -> &'static str {
        match self {
            Self::Shared => "@",
            Self::Mut => "@mut ",
            Self::Write => "@write ",
        }
    }
}
