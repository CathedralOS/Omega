use std::collections::BTreeMap;

use omega_target::{ProgramEntrySlotDeclaration, TargetProfile};
use psi_layout_plans::EntryStubId;

use crate::{
    ExternalRootDiagnostic, RootSlotAuthority, RootSlotId, RootSlotOwnerId, fnv1a_identity,
};

/// Compiler-selected realization of one target-owned required root slot.
///
/// This is input to closure verification, not evidence by itself. Its private
/// fields prevent later consumers from confusing an unchecked row with the
/// verified closed set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRequiredRootSlotSelection {
    target_slot: ProgramEntrySlotDeclaration,
    selected_entry: EntryStubId,
    requirement_identity: String,
}

impl TargetRequiredRootSlotSelection {
    pub fn for_program_entry(
        target_slot: ProgramEntrySlotDeclaration,
        selected_entry: EntryStubId,
        requirement_identity: impl Into<String>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if target_slot != target_slot.owner.program_entry_slot() {
            return Err(ExternalRootDiagnostic(
                "required root-slot selection names a program-entry declaration from another profile"
                    .into(),
            ));
        }
        let requirement_identity = requirement_identity.into();
        if requirement_identity.is_empty() {
            return Err(ExternalRootDiagnostic(
                "required root-slot selection has no exact requirement identity".into(),
            ));
        }
        Ok(Self {
            target_slot,
            selected_entry,
            requirement_identity,
        })
    }
}

/// One exact member of the target-derived required-slot closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRequiredRootSlot {
    target_slot: ProgramEntrySlotDeclaration,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    selected_entry: EntryStubId,
    requirement_identity: String,
}

impl VerifiedRequiredRootSlot {
    pub const fn target_slot(&self) -> ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    pub const fn owner(&self) -> RootSlotOwnerId {
        self.owner
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }
}

/// Opaque, exact, target-derived closure of every build-bound root slot that
/// one selected profile requires.
///
/// The closure is descriptive installation evidence, not root authority. It
/// can only be produced by comparing the supplied selections with the target's
/// complete required-slot declaration set. Runtime-installed open slots do not
/// belong to this set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRequiredRootSlotClosure {
    profile: TargetProfile,
    slots: BTreeMap<RootSlotId, VerifiedRequiredRootSlot>,
    fingerprint: u64,
}

impl VerifiedRequiredRootSlotClosure {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }

    pub fn slots(&self) -> impl ExactSizeIterator<Item = &VerifiedRequiredRootSlot> {
        self.slots.values()
    }

    pub fn slot(&self, identity: RootSlotId) -> Option<&VerifiedRequiredRootSlot> {
        self.slots.get(&identity)
    }

    /// Reporting/cache identity only. Consumers establish authority by exact
    /// member comparison, never by comparing this compact fingerprint.
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

/// Verify the complete required root-slot selection for one target profile.
/// Missing, duplicate, extra, and cross-profile rows all reject before an
/// opaque closure is returned.
pub fn verify_target_required_root_slot_closure(
    profile: TargetProfile,
    selections: impl IntoIterator<Item = TargetRequiredRootSlotSelection>,
) -> Result<VerifiedRequiredRootSlotClosure, ExternalRootDiagnostic> {
    // The current target vocabulary owns exactly one build-bound root slot.
    // Keeping the expected set explicit here means adding a target slot cannot
    // silently turn an absent selection into an open runtime slot.
    let expected = profile.program_entry_slot();
    let expected_authority = RootSlotAuthority::for_target_program_entry(expected)?;
    let mut slots = BTreeMap::new();

    for selection in selections {
        if selection.target_slot.owner != profile {
            return Err(ExternalRootDiagnostic(format!(
                "required root slot `{}::{}` belongs to a different target profile",
                selection.target_slot.owner.root_slot_owner_name(),
                selection.target_slot.slot_name
            )));
        }
        if selection.target_slot != expected {
            return Err(ExternalRootDiagnostic(format!(
                "target profile `{}` does not require root slot `{}::{}`",
                profile.target_name(),
                selection.target_slot.owner.root_slot_owner_name(),
                selection.target_slot.slot_name
            )));
        }
        let verified = VerifiedRequiredRootSlot {
            target_slot: selection.target_slot,
            slot: expected_authority.slot(),
            owner: expected_authority.owner(),
            selected_entry: selection.selected_entry,
            requirement_identity: selection.requirement_identity,
        };
        if slots.insert(verified.slot, verified).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "required root slot `{}::{}` is selected more than once",
                expected.owner.root_slot_owner_name(),
                expected.slot_name
            )));
        }
    }

    if !slots.contains_key(&expected_authority.slot()) {
        return Err(ExternalRootDiagnostic(format!(
            "selected target `{}` has no bound required root slot `{}::{}`",
            profile.target_name(),
            expected.owner.root_slot_owner_name(),
            expected.slot_name
        )));
    }

    let mut canonical = format!("required-root-slot-closure\n{}", profile.target_name());
    for slot in slots.values() {
        canonical.push_str(&format!(
            "\n{}\n{}\n{}\n{}",
            slot.slot.normalized_identity(),
            slot.owner.normalized_identity(),
            slot.selected_entry.normalized_identity(),
            slot.requirement_identity
        ));
    }
    Ok(VerifiedRequiredRootSlotClosure {
        profile,
        slots,
        fingerprint: fnv1a_identity(&canonical),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(identity: u64) -> EntryStubId {
        EntryStubId::from_normalized_identity(identity).expect("entry identity")
    }

    fn selection(profile: TargetProfile) -> TargetRequiredRootSlotSelection {
        TargetRequiredRootSlotSelection::for_program_entry(
            profile.program_entry_slot(),
            entry(91),
            "ProgramStorageEntry::enter/v1",
        )
        .expect("selection")
    }

    #[test]
    fn exact_target_required_slot_set_verifies() {
        let closure = verify_target_required_root_slot_closure(
            TargetProfile::UefiX64,
            [selection(TargetProfile::UefiX64)],
        )
        .expect("exact closure");
        assert_eq!(closure.slots().len(), 1);
        let slot = closure.slots().next().expect("required slot");
        assert_eq!(slot.selected_entry(), entry(91));
        assert_eq!(slot.requirement_identity(), "ProgramStorageEntry::enter/v1");
    }

    #[test]
    fn omitted_required_slot_rejects() {
        let error =
            verify_target_required_root_slot_closure(TargetProfile::UefiX64, std::iter::empty())
                .expect_err("omitted required slot");
        assert!(error.0.contains("no bound required root slot"));
    }

    #[test]
    fn duplicate_required_slot_rejects() {
        let error = verify_target_required_root_slot_closure(
            TargetProfile::UefiX64,
            [
                selection(TargetProfile::UefiX64),
                selection(TargetProfile::UefiX64),
            ],
        )
        .expect_err("duplicate required slot");
        assert!(error.0.contains("more than once"));
    }

    #[test]
    fn cross_profile_required_slot_rejects() {
        let error = verify_target_required_root_slot_closure(
            TargetProfile::UefiX64,
            [selection(TargetProfile::WindowsX64)],
        )
        .expect_err("cross-profile required slot");
        assert!(error.0.contains("different target profile"));
    }
}
