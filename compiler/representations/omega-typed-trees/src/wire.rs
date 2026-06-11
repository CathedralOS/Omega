use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

/// A `wire data` protocol schema carried through the typed stage: stable field
/// numbers, reserved (retired) numbers, and historical version eras. Wire
/// schemas are external-representation contracts, kept separate from runtime
/// `data` definitions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireSchema {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub encoding: Option<Identifier>,
    pub members: HandleSpan<WireMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireMember {
    Field(WireField),
    Reserved(WireReserved),
    Version(WireVersion),
}

impl Default for WireMember {
    fn default() -> Self {
        Self::Reserved(WireReserved::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireField {
    pub number: i64,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
}

impl Default for WireField {
    fn default() -> Self {
        Self {
            number: 0,
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireReserved {
    pub number: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireVersion {
    pub name: Identifier,
    pub members: HandleSpan<WireMember>,
}
