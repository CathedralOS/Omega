use omega_executable_installation::{
    ArtifactRelocationKind, DecodedArtifactRelocation, ValidatedArtifactContainer,
};
use omega_object_file::{
    ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
    SectionKind,
};
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;
use psi_layout_plans::RelocationTarget;

/// Translate a semantically validated Omega artifact relocation set into the
/// existing target-object relocation plan. Symbol resolution stays behind a
/// provider/compiler callback; sealed entry and data identities never become
/// numeric addresses here.
pub fn append_validated_artifact_relocations(
    artifact: &ValidatedArtifactContainer,
    section: SectionKind,
    section_base_offset: usize,
    owner_symbol_handle: ObjectSymbolHandle,
    relocations: &mut RelocationPlan,
    mut target_symbol: impl FnMut(RelocationTarget) -> Option<ObjectSymbolHandle>,
) -> Result<usize, Diagnostic> {
    if section == SectionKind::Bss {
        return Err(Diagnostic::error(
            "artifact code relocations require an initialized text or data section",
        ));
    }
    if !owner_symbol_handle.is_valid() {
        return Err(Diagnostic::error(
            "artifact relocation translation requires a valid owner object symbol",
        ));
    }
    if artifact.artifact().architecture() != relocations.target.architecture {
        return Err(Diagnostic::error(format!(
            "artifact architecture {:?} is incompatible with target architecture {:?}",
            artifact.artifact().architecture(),
            relocations.target.architecture
        )));
    }

    let mut translated = Vec::with_capacity(artifact.relocations().len());
    for relocation in artifact.relocations() {
        translated.push(translate_relocation(
            *relocation,
            relocations.target.architecture,
            section,
            section_base_offset,
            owner_symbol_handle,
            &mut target_symbol,
        )?);
    }
    let appended = translated.len();
    for relocation in translated {
        relocations.push_record(relocation);
    }
    Ok(appended)
}

fn translate_relocation(
    relocation: DecodedArtifactRelocation,
    architecture: Architecture,
    section: SectionKind,
    section_base_offset: usize,
    owner_symbol_handle: ObjectSymbolHandle,
    target_symbol: &mut impl FnMut(RelocationTarget) -> Option<ObjectSymbolHandle>,
) -> Result<RelocationRecord, Diagnostic> {
    let kind = match (architecture, relocation.kind) {
        (_, ArtifactRelocationKind::Absolute64) => RelocationKind::Absolute64,
        (Architecture::X86_64, ArtifactRelocationKind::X86Relative32) => {
            RelocationKind::X86_64Relative32
        }
        (Architecture::Aarch64, ArtifactRelocationKind::Aarch64Page21) => {
            RelocationKind::Aarch64Page21
        }
        (Architecture::Aarch64, ArtifactRelocationKind::Aarch64PageOffset12) => {
            RelocationKind::Aarch64PageOffset12
        }
        (Architecture::Aarch64, ArtifactRelocationKind::Aarch64Branch26) => {
            RelocationKind::Aarch64Branch26
        }
        (actual, authored) => {
            return Err(Diagnostic::error(format!(
                "artifact relocation {authored:?} is incompatible with target architecture {actual:?}"
            )));
        }
    };
    let local_offset = usize::try_from(relocation.destination_offset).map_err(|_| {
        Diagnostic::error("artifact relocation offset does not fit the compiler host")
    })?;
    let offset = section_base_offset
        .checked_add(local_offset)
        .ok_or_else(|| Diagnostic::error("artifact relocation section offset overflows"))?;
    let symbol_handle = target_symbol(relocation.target)
        .filter(|handle| handle.is_valid())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "artifact relocation target {:?} has no object symbol",
                relocation.target
            ))
        })?;
    let byte_width = match kind {
        RelocationKind::Absolute64 => 8,
        RelocationKind::X86_64Relative32
        | RelocationKind::Aarch64Page21
        | RelocationKind::Aarch64PageOffset12
        | RelocationKind::Aarch64Branch26 => 4,
    };
    Ok(RelocationRecord {
        origin: RelocationOrigin::Materialization {
            object_symbol_handle: owner_symbol_handle,
        },
        section,
        offset,
        byte_width,
        symbol_handle,
        addend: relocation.addend,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_executable_installation::{
        ArtifactContentId, ArtifactEntry, ArtifactId, ContainerLimits, ContainerSection,
        ContainerSectionKind, DecodedArtifactContainer, EntrySetId, MachineContractSetId,
        MachineFootprintId, OMEGA_EXECUTABLE_CONTAINER_MARKER, PlacementPlanId, RelocationSetId,
        normalized_decoded_content_identity, normalized_proof_payload_identity,
        validate_decoded_container,
    };
    use omega_target::NativeTarget;
    use psi_arena::Handle;
    use psi_layout_plans::{EntryStubId, PlacementConstraints, PlacementPhase};

    fn id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn validated(
        kind: ArtifactRelocationKind,
        addend: i64,
    ) -> (ValidatedArtifactContainer, RelocationTarget) {
        let entry = EntryStubId::from_normalized_identity(9).expect("normalized entry identity");
        let target = RelocationTarget::Entry(entry);
        let relocations = id(6, RelocationSetId::from_normalized_identity);
        let proof = vec![0xa5; 64];
        let proof_payload = normalized_proof_payload_identity(&proof);
        let mut decoded = DecodedArtifactContainer {
            format_marker: OMEGA_EXECUTABLE_CONTAINER_MARKER,
            total_length: 400,
            artifact: id(1, ArtifactId::from_normalized_identity),
            content: id(2, ArtifactContentId::from_normalized_identity),
            architecture: Architecture::X86_64,
            code_length: 64,
            code: vec![0x90; 64],
            contracts: id(3, MachineContractSetId::from_normalized_identity),
            declared_footprint: id(4, MachineFootprintId::from_normalized_identity),
            placement_plan: id(5, PlacementPlanId::from_normalized_identity),
            placement_constraints: PlacementConstraints::unconstrained(PlacementPhase::Load),
            entry_set: id(8, EntrySetId::from_normalized_identity),
            entries: vec![ArtifactEntry::from_canonical_decode(entry, 16)],
            relocation_set: relocations,
            relocations: vec![DecodedArtifactRelocation {
                kind,
                destination_offset: 24,
                target,
                addend,
            }],
            proof_payload,
            proof,
            sections: vec![
                ContainerSection {
                    kind: ContainerSectionKind::Code,
                    offset: 0,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Relocations(relocations),
                    offset: 64,
                    length: 40,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Contracts(id(
                        3,
                        MachineContractSetId::from_normalized_identity,
                    )),
                    offset: 128,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Footprint(id(
                        4,
                        MachineFootprintId::from_normalized_identity,
                    )),
                    offset: 192,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Placement(id(
                        5,
                        PlacementPlanId::from_normalized_identity,
                    )),
                    offset: 256,
                    length: 64,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Entries(id(
                        8,
                        EntrySetId::from_normalized_identity,
                    )),
                    offset: 320,
                    length: 16,
                },
                ContainerSection {
                    kind: ContainerSectionKind::Proof(proof_payload),
                    offset: 336,
                    length: 64,
                },
            ],
        };
        decoded.content = normalized_decoded_content_identity(&decoded).expect("content identity");
        let limits = ContainerLimits {
            max_total_bytes: 4096,
            max_sections: 16,
            max_section_bytes: 1024,
            max_relocations: 16,
        };
        (
            validate_decoded_container(decoded, limits).expect("validated artifact"),
            target,
        )
    }

    #[test]
    fn validated_artifact_relocations_translate_atomically() {
        let (artifact, target) = validated(ArtifactRelocationKind::X86Relative32, 0);
        let owner = Handle::from_arena_index(2);
        let destination = Handle::from_arena_index(3);
        let mut plan = RelocationPlan::with_target(NativeTarget::linux_x64());

        let count = append_validated_artifact_relocations(
            &artifact,
            SectionKind::Text,
            100,
            owner,
            &mut plan,
            |candidate| (candidate == target).then_some(destination),
        )
        .expect("validated relocation translation");

        assert_eq!(count, 1);
        let record = plan.records().next().expect("relocation record").1;
        assert_eq!(record.offset, 124);
        assert_eq!(record.byte_width, 4);
        assert_eq!(record.kind, RelocationKind::X86_64Relative32);
        assert_eq!(record.symbol_handle, destination);
        assert_eq!(record.addend, 0);
    }

    #[test]
    fn preserves_addends_while_architecture_and_symbol_failures_append_nothing() {
        let owner = Handle::from_arena_index(2);
        let destination = Handle::from_arena_index(3);

        let (wrong_artifact_architecture, target) =
            validated(ArtifactRelocationKind::Absolute64, 0);
        let mut aarch64 = RelocationPlan::with_target(NativeTarget::linux_arm64());
        let error = append_validated_artifact_relocations(
            &wrong_artifact_architecture,
            SectionKind::Text,
            0,
            owner,
            &mut aarch64,
            |candidate| (candidate == target).then_some(destination),
        )
        .expect_err("artifact architecture mismatch rejects");
        assert!(error.message.contains("artifact architecture"));
        assert_eq!(aarch64.record_count(), 0);

        let mut x86 = RelocationPlan::with_target(NativeTarget::linux_x64());
        let (addend, target) = validated(ArtifactRelocationKind::X86Relative32, 4);
        append_validated_artifact_relocations(
            &addend,
            SectionKind::Text,
            0,
            owner,
            &mut x86,
            |candidate| (candidate == target).then_some(destination),
        )
        .expect("semantic addend translates");
        assert_eq!(x86.record_count(), 1);
        assert_eq!(x86.records().next().expect("addend record").1.addend, 4);

        let (missing, _) = validated(ArtifactRelocationKind::X86Relative32, 0);
        let error = append_validated_artifact_relocations(
            &missing,
            SectionKind::Text,
            0,
            owner,
            &mut x86,
            |_| None,
        )
        .expect_err("missing symbol rejects");
        assert!(error.message.contains("no object symbol"));
        assert_eq!(x86.record_count(), 1);
    }
}
