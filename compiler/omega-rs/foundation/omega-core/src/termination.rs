/// The published eventual-completion promise carried by a machine contract.
///
/// This identity is intentionally independent of the implementation's ranking
/// witness. Premises and terminal-outcome contracts join the payload in TPR4;
/// the v1 migration starts with the guarantee bit as an explicit semantic enum
/// so it can no longer drift together with proof evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TerminationGuarantee {
    #[default]
    None,
    EventualTerminal,
}

impl TerminationGuarantee {
    pub const fn is_eventual_terminal(self) -> bool {
        matches!(self, Self::EventualTerminal)
    }
}

impl From<bool> for TerminationGuarantee {
    fn from(value: bool) -> Self {
        if value {
            Self::EventualTerminal
        } else {
            Self::None
        }
    }
}
