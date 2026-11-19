//! Closed target properties supplied by the compiler's target and plan owners.

use super::{SelectedLayout, TargetSemantics};
use num_bigint::BigUint;
use omega_typer::ValueType;
use std::sync::Arc;

/// The normalized identity of one address space in the selected target model.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AddressSpaceIdentity(pub Arc<[u8]>);

/// The normalized identity of one selected target entry contract.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntryProfileIdentity(pub Arc<[u8]>);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Endianness {
    Little,
    Big,
}

/// The minimum stack geometry promised by an entry profile. Physical arrival
/// needs independent evidence that the incoming stack meets the promise.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EntryStackGuarantee {
    pub minimum_available_bytes: u64,
    pub alignment: u64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TargetProperty {
    ExclusiveAddressBound {
        address_space: AddressSpaceIdentity,
        exclusive_bound: BigUint,
    },
    LayoutEndianness {
        layout: Arc<SelectedLayout>,
        endianness: Endianness,
    },
    EntryStack {
        profile: EntryProfileIdentity,
        guarantee: EntryStackGuarantee,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SelectedTargetProperty {
    pub projection_version: u32,
    pub value: TargetProperty,
}

/// Compiler queries identify their subject in the capsule's closed vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetPropertyQuery<'a> {
    ExclusiveAddressBound(&'a AddressSpaceIdentity),
    LayoutEndianness(&'a ValueType),
    EntryStack(&'a EntryProfileIdentity),
}

impl TargetProperty {
    pub(super) fn query(&self) -> TargetPropertyQuery<'_> {
        match self {
            Self::ExclusiveAddressBound { address_space, .. } => TargetPropertyQuery::ExclusiveAddressBound(address_space),
            Self::LayoutEndianness { layout, .. } => TargetPropertyQuery::LayoutEndianness(&layout.subject),
            Self::EntryStack { profile, .. } => TargetPropertyQuery::EntryStack(profile),
        }
    }

    pub(super) fn is_valid(
        &self,
        capsule: &TargetSemantics,
    ) -> bool {
        match self {
            Self::ExclusiveAddressBound { address_space, .. } => !address_space.0.is_empty(),
            Self::LayoutEndianness { layout, .. } => capsule.layout_for(&layout.subject) == Some(layout.as_ref()),
            Self::EntryStack { profile, guarantee } => !profile.0.is_empty() && guarantee.alignment.is_power_of_two(),
        }
    }
}

#[cfg(test)]
mod tests;
