#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceDimension {
    Borrow,
    Proof,
    ServiceReach,
    Suspension,
    Blocking,
    Boundaries,
    Termination,
}

impl AcceptanceDimension {
    pub const ALL: [Self; 7] = [
        Self::Borrow,
        Self::Proof,
        Self::ServiceReach,
        Self::Suspension,
        Self::Blocking,
        Self::Boundaries,
        Self::Termination,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Borrow => "borrow",
            Self::Proof => "proof",
            Self::ServiceReach => "service_reach",
            Self::Suspension => "suspension",
            Self::Blocking => "blocking",
            Self::Boundaries => "boundaries",
            Self::Termination => "termination",
        }
    }
}
