use std::collections::BTreeMap;

use layout_plans::EntryStubId;
use target::{ProgramEntrySlotDeclaration, TargetProfile, TargetRequiredRootSlotDeclaration};

use crate::{
    ArtifactId, ExternalRootDiagnostic, ExternalRootId, InstallationScopeId, InstalledCodeId,
    InstalledExternalRoot, InstalledRootEvidence, InstalledRootLedger, RootSlotAuthority,
    RootSlotId, RootSlotOwnerId,
};

/// Compiler-selected realization of one target-owned required root slot.
///
/// This is input to closure verification, not evidence by itself. Its private
/// fields prevent later consumers from confusing an unchecked row with the
/// verified closed set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRequiredRootSlotSelection {
    target_slot: TargetRequiredRootSlotDeclaration,
    selected_entry: EntryStubId,
    requirement_identity: String,
}

impl TargetRequiredRootSlotSelection {
    pub fn for_program_entry(
        target_slot: ProgramEntrySlotDeclaration,
        selected_entry: EntryStubId,
        requirement_identity: impl Into<String>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let target_slot = TargetRequiredRootSlotDeclaration::ProgramEntry(target_slot);
        if target_slot
            .owner()
            .required_root_slot(target_slot.slot_name())
            != Some(target_slot)
        {
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
    target_slot: TargetRequiredRootSlotDeclaration,
    slot: RootSlotId,
    owner: RootSlotOwnerId,
    selected_entry: EntryStubId,
    requirement_identity: String,
}

impl VerifiedRequiredRootSlot {
    pub const fn target_slot(&self) -> TargetRequiredRootSlotDeclaration {
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
}

/// One target-required slot sealed to the exact installed root that occupies
/// it. The target closure remains descriptive; this row only proves that the
/// complete selected set was replayed against one installation ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRequiredRootSlot {
    required: VerifiedRequiredRootSlot,
    root: ExternalRootId,
    evidence: InstalledRootEvidence,
}

impl InstalledRequiredRootSlot {
    pub const fn required(&self) -> &VerifiedRequiredRootSlot {
        &self.required
    }

    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub(super) fn matches_root(&self, root: &InstalledExternalRoot<'_>) -> bool {
        self.root == root.root
            && self.required.slot == root.slot
            && self.required.owner == root.owner
            && self.evidence == root.evidence
    }
}

/// Exact target-required root-slot closure retained by one installed artifact.
///
/// Exact member evidence is retained privately and replayed by the
/// program-local cohort verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRequiredRootSlotClosure {
    profile: TargetProfile,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    installation_scope: InstallationScopeId,
    slots: BTreeMap<RootSlotId, InstalledRequiredRootSlot>,
}

impl InstalledRequiredRootSlotClosure {
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn installation_scope(&self) -> InstallationScopeId {
        self.installation_scope
    }

    pub fn slots(&self) -> impl ExactSizeIterator<Item = &InstalledRequiredRootSlot> {
        self.slots.values()
    }

    pub fn slot(&self, identity: RootSlotId) -> Option<&InstalledRequiredRootSlot> {
        self.slots.get(&identity)
    }
}

impl InstalledRootLedger {
    /// Replay and retain the complete target-required closure against this
    /// exact installed root ledger. Every required member must already be
    /// occupied by the selected entry and requirement; sealing is one-shot.
    pub fn seal_required_root_slot_closure(
        &mut self,
        closure: VerifiedRequiredRootSlotClosure,
    ) -> Result<&InstalledRequiredRootSlotClosure, ExternalRootDiagnostic> {
        if self.required_root_slots.is_some() {
            return Err(ExternalRootDiagnostic(
                "required root-slot closure was already sealed for this installation".into(),
            ));
        }

        let mut slots = BTreeMap::new();
        for required in closure.slots.values() {
            let record = self
                .roots
                .values()
                .find(|record| record.slot == required.slot)
                .ok_or_else(|| {
                    ExternalRootDiagnostic(format!(
                        "required root slot `{}` has no installed root",
                        required.slot.normalized_identity()
                    ))
                })?;
            if record.installed_code != self.installed_code
                || record.artifact != self.artifact
                || record.owner != required.owner
                || record.entry != required.selected_entry
                || record.requirement_identity != required.requirement_identity
            {
                return Err(ExternalRootDiagnostic(
                    "installed root does not match the exact required slot owner, entry, requirement, code, and artifact"
                        .into(),
                ));
            }
            let evidence = self
                .root_evidence
                .get(&record.root)
                .cloned()
                .ok_or_else(|| {
                    ExternalRootDiagnostic(
                        "required root slot has no exact installed-root evidence".into(),
                    )
                })?;
            slots.insert(
                required.slot,
                InstalledRequiredRootSlot {
                    required: required.clone(),
                    root: record.root,
                    evidence,
                },
            );
        }

        self.required_root_slots = Some(InstalledRequiredRootSlotClosure {
            profile: closure.profile,
            installed_code: self.installed_code,
            artifact: self.artifact,
            installation_scope: self.installation_scope,
            slots,
        });
        Ok(self
            .required_root_slots
            .as_ref()
            .expect("required root-slot closure was just retained"))
    }
}

/// Verify the complete required root-slot selection for one target profile.
/// Missing, duplicate, extra, and cross-profile rows all reject before an
/// opaque closure is returned.
pub fn verify_target_required_root_slot_closure(
    profile: TargetProfile,
    selections: impl IntoIterator<Item = TargetRequiredRootSlotSelection>,
) -> Result<VerifiedRequiredRootSlotClosure, ExternalRootDiagnostic> {
    let mut expected = BTreeMap::new();
    for declaration in profile.required_root_slots() {
        let authority = RootSlotAuthority::for_target_required_root_slot(declaration)?;
        if let Some((existing, _)) = expected.insert(authority.slot(), (declaration, authority)) {
            let diagnostic = if existing == declaration {
                format!(
                    "target profile `{}` declares required root slot `{}::{}` more than once",
                    profile.target_name(),
                    declaration.owner().root_slot_owner_name(),
                    declaration.slot_name()
                )
            } else {
                format!(
                    "distinct target-required root slots `{}::{}` and `{}::{}` collide on one compact slot identity",
                    existing.owner().root_slot_owner_name(),
                    existing.slot_name(),
                    declaration.owner().root_slot_owner_name(),
                    declaration.slot_name()
                )
            };
            return Err(ExternalRootDiagnostic(diagnostic));
        }
    }
    let mut slots = BTreeMap::new();

    for selection in selections {
        if selection.target_slot.owner() != profile {
            return Err(ExternalRootDiagnostic(format!(
                "required root slot `{}::{}` belongs to a different target profile",
                selection.target_slot.owner().root_slot_owner_name(),
                selection.target_slot.slot_name()
            )));
        }
        let authority = RootSlotAuthority::for_target_required_root_slot(selection.target_slot)?;
        let Some((declaration, expected_authority)) = expected.get(&authority.slot()) else {
            return Err(ExternalRootDiagnostic(format!(
                "target profile `{}` does not require root slot `{}::{}`",
                profile.target_name(),
                selection.target_slot.owner().root_slot_owner_name(),
                selection.target_slot.slot_name()
            )));
        };
        if declaration != &selection.target_slot || expected_authority != &authority {
            return Err(ExternalRootDiagnostic(format!(
                "required root slot `{}::{}` does not match its target catalog declaration",
                selection.target_slot.owner().root_slot_owner_name(),
                selection.target_slot.slot_name()
            )));
        }
        let verified = VerifiedRequiredRootSlot {
            target_slot: selection.target_slot,
            slot: authority.slot(),
            owner: authority.owner(),
            selected_entry: selection.selected_entry,
            requirement_identity: selection.requirement_identity,
        };
        if slots.insert(verified.slot, verified).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "required root slot `{}::{}` is selected more than once",
                selection.target_slot.owner().root_slot_owner_name(),
                selection.target_slot.slot_name()
            )));
        }
    }

    if let Some((_, (missing, _))) = expected
        .iter()
        .find(|(identity, _)| !slots.contains_key(identity))
    {
        return Err(ExternalRootDiagnostic(format!(
            "selected target `{}` has no bound required root slot `{}::{}`",
            profile.target_name(),
            missing.owner().root_slot_owner_name(),
            missing.slot_name()
        )));
    }

    Ok(VerifiedRequiredRootSlotClosure { profile, slots })
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
        assert_eq!(
            slot.target_slot(),
            TargetProfile::UefiX64
                .required_root_slot("ProgramEntry")
                .expect("catalogued ProgramEntry")
        );
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
