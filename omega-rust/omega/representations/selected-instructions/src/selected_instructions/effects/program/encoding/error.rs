#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAllocationMachineEffectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for PreAllocationMachineEffectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-allocation machine-effect artifact: {self:?}"
        )
    }
}

impl std::error::Error for PreAllocationMachineEffectDecodeError {}
