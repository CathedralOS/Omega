//! Established interrupt tables and their members.

use crate::interrupts::interrupt_table::{
    InterruptTableGateDescriptor, InterruptTableObligation, InterruptTableProfile,
};
use crate::{
    ExternalRootDiagnostic, ExternalRootId, InstalledExternalRoot, InterruptTableEstablishmentId,
    InterruptTableProfileId, RootSlotId,
};
use executable_installation::{ArtifactId, InstalledCodeId};
use extents::Extent;
use layout_plans::EntryStubId;
use std::collections::BTreeMap;

/// The member set one consumer-established table value claims to describe.
/// The root and entry identities are replayed against the admitted member
/// records before publication; a stale or substituted row rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableEstablishedMember {
    pub root: ExternalRootId,
    pub entry: EntryStubId,
}

/// The consumer-established table value: the descriptor-table bytes the
/// owner's semantic validator accepted for this exact profile, member set,
/// and destination.
///
/// Gate-kind, selector, privilege, IST, reserved-bit, and base/limit policy
/// belong to the consumer's validator; the compiler binds the established
/// identity, its claimed member rows, and the occupied destination so the
/// issued carrier cannot describe a different table. The destination extent
/// is linear resource evidence, so the value itself cannot be cloned.
#[derive(Debug, PartialEq, Eq)]
pub struct EstablishedInterruptTable {
    pub(crate) establishment: InterruptTableEstablishmentId,
    pub(crate) profile: InterruptTableProfileId,
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) artifact: ArtifactId,
    pub(crate) members: BTreeMap<u8, InterruptTableEstablishedMember>,
    pub(crate) destination: Extent,
}

impl EstablishedInterruptTable {
    pub fn from_consumer(
        establishment: InterruptTableEstablishmentId,
        profile: &InterruptTableProfile,
        installed_code: InstalledCodeId,
        artifact: ArtifactId,
        members: impl IntoIterator<Item = (u8, InterruptTableEstablishedMember)>,
        destination: Extent,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let members: BTreeMap<u8, InterruptTableEstablishedMember> = members.into_iter().collect();
        if members.is_empty() {
            return Err(ExternalRootDiagnostic(
                "established interrupt table describes no member rows".into(),
            ));
        }
        if destination.length() == 0 {
            return Err(ExternalRootDiagnostic(
                "established interrupt table occupies an empty destination extent".into(),
            ));
        }
        Ok(Self {
            establishment,
            profile: profile.identity(),
            installed_code,
            artifact,
            members,
            destination,
        })
    }

    pub const fn establishment(&self) -> InterruptTableEstablishmentId {
        self.establishment
    }

    pub const fn destination(&self) -> &Extent {
        &self.destination
    }
}

/// One admitted table member. The retained linear root handle is the custody
/// pin: removal consumes `InstalledExternalRoot`, so the member's entry
/// cannot leave the installed ledger while the table holds it.
#[derive(Debug)]
pub struct InterruptTableMember<'code> {
    pub(crate) vector: u8,
    pub(crate) dedicated_stack_class: u16,
    pub(crate) obligation: InterruptTableObligation,
    pub(crate) descriptor: InterruptTableGateDescriptor,
    pub(crate) entry: EntryStubId,
    pub(crate) slot: RootSlotId,
    pub(crate) root: InstalledExternalRoot<'code>,
}

impl<'code> InterruptTableMember<'code> {
    pub const fn vector(&self) -> u8 {
        self.vector
    }

    pub const fn dedicated_stack_class(&self) -> u16 {
        self.dedicated_stack_class
    }

    pub const fn obligation(&self) -> InterruptTableObligation {
        self.obligation
    }

    /// The member's declared descriptor constants — the staged content the
    /// produced gate bytes must carry.
    pub const fn descriptor(&self) -> InterruptTableGateDescriptor {
        self.descriptor
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    /// Borrow the retained member handle. The interrupt-entry path consumes
    /// only a shared borrow, so the timer member's provider receipts can be
    /// admitted without releasing the pin.
    pub const fn root(&self) -> &InstalledExternalRoot<'code> {
        &self.root
    }
}
