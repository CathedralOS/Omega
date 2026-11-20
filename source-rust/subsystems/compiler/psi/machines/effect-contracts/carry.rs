//! Acquisition and provenance coordinates retained by a conditional carry requirement.

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, record_storage::Record,
)]
pub enum PermissionEventSource<Identity> {
    #[default]
    StateEntry,
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: Identity,
    },
    InstructionDisposition {
        statement_index: usize,
    },
    InstructionExitDisposition {
        statement_index: usize,
    },
    StateExit,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, record_storage::Record,
)]
pub enum PermissionProvenance<Identity> {
    #[default]
    Unknown,
    Established {
        machine_symbol: Identity,
        state_symbol: Identity,
        source: PermissionEventSource<Identity>,
    },
    Joined {
        machine_symbol: Identity,
        state_symbol: Identity,
        source: PermissionEventSource<Identity>,
        ordinal: u32,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, record_storage::Record,
)]
pub enum PermissionClaimIdentity<Identity> {
    #[default]
    Unknown,
    Established {
        machine_symbol: Identity,
        state_symbol: Identity,
        source: PermissionEventSource<Identity>,
        ordinal: u32,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
pub struct PermissionArrival<Identity> {
    pub claim_identity: PermissionClaimIdentity<Identity>,
    pub provenance: PermissionProvenance<Identity>,
}

impl<Identity> PermissionProvenance<Identity> {
    pub fn map_declarations<Output, Error>(
        self,
        mut map: impl FnMut(Identity) -> Result<Output, Error>,
    ) -> Result<PermissionProvenance<Output>, Error> {
        Ok(match self {
            Self::Unknown => PermissionProvenance::Unknown,
            Self::Established {
                machine_symbol,
                state_symbol,
                source,
            } => PermissionProvenance::Established {
                machine_symbol: map(machine_symbol)?,
                state_symbol: map(state_symbol)?,
                source: source.map_declarations(map)?,
            },
            Self::Joined {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } => PermissionProvenance::Joined {
                machine_symbol: map(machine_symbol)?,
                state_symbol: map(state_symbol)?,
                source: source.map_declarations(map)?,
                ordinal,
            },
        })
    }
}

impl<Identity> PermissionArrival<Identity> {
    pub fn map_declarations<Output, Error>(
        self,
        mut map: impl FnMut(Identity) -> Result<Output, Error>,
    ) -> Result<PermissionArrival<Output>, Error> {
        Ok(PermissionArrival {
            claim_identity: self.claim_identity.map_declarations(&mut map)?,
            provenance: self.provenance.map_declarations(map)?,
        })
    }
}

impl<Identity> PermissionEventSource<Identity> {
    pub fn map_declarations<Output, Error>(
        self,
        mut map: impl FnMut(Identity) -> Result<Output, Error>,
    ) -> Result<PermissionEventSource<Output>, Error> {
        Ok(match self {
            Self::StateEntry => PermissionEventSource::StateEntry,
            Self::Statement { statement_index } => {
                PermissionEventSource::Statement { statement_index }
            }
            Self::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            } => PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol: map(target_symbol)?,
            },
            Self::InstructionDisposition { statement_index } => {
                PermissionEventSource::InstructionDisposition { statement_index }
            }
            Self::InstructionExitDisposition { statement_index } => {
                PermissionEventSource::InstructionExitDisposition { statement_index }
            }
            Self::StateExit => PermissionEventSource::StateExit,
        })
    }
}

impl<Identity> PermissionClaimIdentity<Identity> {
    pub fn map_declarations<Output, Error>(
        self,
        mut map: impl FnMut(Identity) -> Result<Output, Error>,
    ) -> Result<PermissionClaimIdentity<Output>, Error> {
        Ok(match self {
            Self::Unknown => PermissionClaimIdentity::Unknown,
            Self::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } => PermissionClaimIdentity::Established {
                machine_symbol: map(machine_symbol)?,
                state_symbol: map(state_symbol)?,
                source: source.map_declarations(map)?,
                ordinal,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
pub enum SuspensionValueOrigin<Identity> {
    Parameter {
        symbol: Identity,
        position: usize,
    },
    Local {
        symbol: Identity,
        statement_index: usize,
        environment_position: usize,
    },
    CallArgument {
        position: usize,
    },
}

impl<Identity> SuspensionValueOrigin<Identity> {
    pub fn map_declarations<Output, Error>(
        self,
        mut map: impl FnMut(Identity) -> Result<Output, Error>,
    ) -> Result<SuspensionValueOrigin<Output>, Error> {
        Ok(match self {
            Self::Parameter { symbol, position } => SuspensionValueOrigin::Parameter {
                symbol: map(symbol)?,
                position,
            },
            Self::Local {
                symbol,
                statement_index,
                environment_position,
            } => SuspensionValueOrigin::Local {
                symbol: map(symbol)?,
                statement_index,
                environment_position,
            },
            Self::CallArgument { position } => SuspensionValueOrigin::CallArgument { position },
        })
    }
}
