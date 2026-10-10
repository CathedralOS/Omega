use semantic_vocabulary::ServiceId;
use crate::terminal_module::TerminalIdentityIndex;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalRootServiceReach {
    pub concrete: Vec<ServiceId>,
    pub installation_dependencies: Vec<InstallationReachDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallationReachDependency {
    pub requirement_identity: TerminalIdentityIndex,
    pub upper_bound: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceDeclaration {
    pub id: ServiceId,
    pub identity: TerminalIdentityIndex,
    /// Strictly ordered canonical parent closure.
    pub parents: Vec<ServiceId>,
}
