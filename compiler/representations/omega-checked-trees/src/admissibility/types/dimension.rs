#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceDimension {
    Borrow,
    Proof,
    Effects,
    Boundaries,
    Termination,
}

impl AcceptanceDimension {
    pub const ALL: [Self; 5] = [
        Self::Borrow,
        Self::Proof,
        Self::Effects,
        Self::Boundaries,
        Self::Termination,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Borrow => "borrow",
            Self::Proof => "proof",
            Self::Effects => "effects",
            Self::Boundaries => "boundaries",
            Self::Termination => "termination",
        }
    }
}
