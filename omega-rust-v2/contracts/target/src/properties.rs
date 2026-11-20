//! Closed target properties supplied by the compiler's target and plan owners.

use super::{LayoutSubject, SelectedLayout, TargetSemantics};
use num_bigint::BigUint;

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
pub enum TargetProperty<S> {
    ExclusiveAddressBound {
        address_space: AddressSpaceIdentity,
        exclusive_bound: BigUint,
    },
    LayoutEndianness {
        layout: Arc<SelectedLayout<S>>,
        endianness: Endianness,
    },
    EntryStack {
        profile: EntryProfileIdentity,
        guarantee: EntryStackGuarantee,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SelectedTargetProperty<S> {
    pub projection_version: u32,
    pub value: TargetProperty<S>,
}

/// Compiler queries identify their subject in the capsule's closed vocabulary.
#[derive(Debug, Eq, PartialEq)]
pub enum TargetPropertyQuery<'a, S> {
    ExclusiveAddressBound(&'a AddressSpaceIdentity),
    LayoutEndianness(&'a S),
    EntryStack(&'a EntryProfileIdentity),
}

impl<S> Copy for TargetPropertyQuery<'_, S> {}
impl<S> Clone for TargetPropertyQuery<'_, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: LayoutSubject> TargetProperty<S> {
    pub(super) fn query(&self) -> TargetPropertyQuery<'_, S> {
        match self {
            Self::ExclusiveAddressBound { address_space, .. } => TargetPropertyQuery::ExclusiveAddressBound(address_space),
            Self::LayoutEndianness { layout, .. } => TargetPropertyQuery::LayoutEndianness(&layout.subject),
            Self::EntryStack { profile, .. } => TargetPropertyQuery::EntryStack(profile),
        }
    }

    pub(super) fn is_valid<D: Clone + Eq>(
        &self,
        capsule: &TargetSemantics<S, D>,
    ) -> bool {
        match self {
            Self::ExclusiveAddressBound { address_space, .. } => !address_space.0.is_empty(),
            Self::LayoutEndianness { layout, .. } => capsule.layout_for(&layout.subject) == Some(layout.as_ref()),
            Self::EntryStack { profile, guarantee } => !profile.0.is_empty() && guarantee.alignment.is_power_of_two(),
        }
    }
}
