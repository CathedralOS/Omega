use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMathematicalCarrier {
    pub type_identity: TerminalIdentityIndex,
    pub carrier_key: Vec<u8>,
}

/// Source-free formation of an Id family over checked value subjects.
/// These rows grant no identity witness and allocate no runtime value copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalIdentityFormation {
    pub source_owner: TerminalIdentityIndex,
    pub source_carrier: TerminalIdentityIndex,
    pub subject_carriers: Vec<TerminalIdentityIndex>,
    pub subject_origins: Vec<(u32, u32)>,
    pub declaration: u32,
}

/// One shared checked signature avoids repeating it for every Id occurrence.
/// Each row names a transparent type-valued definition in the certificate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalIdentityFormations {
    pub rows: Vec<TerminalIdentityFormation>,
    pub certificate: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalIndexedTheorem {
    pub source_contract: CheckedMachineDeclaration,
    pub ensure_ordinal: u32,
    pub evidence_source_position: u32,
    pub type_identity: TerminalIdentityIndex,
    pub certificate: Vec<u8>,
    pub assumption_closure: Vec<u32>,
}
