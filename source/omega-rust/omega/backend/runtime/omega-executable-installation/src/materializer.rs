use super::*;

/// Inert provider-side result of resolving one admitted artifact at one exact
/// placement. The bytes are not executable authority; only the installation
/// ladder can consume the corresponding placement and establish execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedArtifactBytes {
    admission: AdmittedArtifact,
    placement: CodePlacementEvidence,
    placement_plan: PlacementPlanId,
    base_address: u64,
    bytes: Vec<u8>,
    final_bytes: FinalBytesDigest,
}

impl MaterializedArtifactBytes {
    pub fn artifact(&self) -> ArtifactId {
        self.admission.artifact.0.identity
    }

    pub(super) const fn admission_evidence(&self) -> &AdmittedArtifact {
        &self.admission
    }

    pub const fn admission(&self) -> AdmissionReceiptId {
        self.admission.admission
    }

    pub const fn placement(&self) -> CodePlacementId {
        self.placement.placement
    }

    pub(super) const fn placement_evidence(&self) -> &CodePlacementEvidence {
        &self.placement
    }

    pub const fn placement_plan(&self) -> PlacementPlanId {
        self.placement_plan
    }

    pub const fn base_address(&self) -> u64 {
        self.base_address
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn final_bytes(&self) -> FinalBytesDigest {
        self.final_bytes
    }
}

/// Resolve and patch one admitted artifact without granting write or execute
/// authority. Resolution is atomic: a missing/out-of-range target returns an
/// error and no partially materialized result.
pub fn materialize_admitted_artifact(
    artifact: &AdmittedArtifact,
    placement: &CodePlacement,
    mut resolve: impl FnMut(RelocationTarget) -> Option<u64>,
) -> Result<MaterializedArtifactBytes, InstallationDiagnostic> {
    let record = &artifact.artifact.0;
    if placement.extent.length() < record.byte_length {
        return Err(InstallationDiagnostic(
            "code placement is smaller than the admitted artifact".into(),
        ));
    }
    if placement.constraints != record.placement_constraints {
        return Err(InstallationDiagnostic(
            "code placement constraints do not match the admitted artifact".into(),
        ));
    }

    let mut bytes = record.code.clone();
    for relocation in &record.relocations {
        let target = resolve(relocation.target).ok_or_else(|| {
            InstallationDiagnostic(format!(
                "artifact relocation target {:?} has no admitted realization",
                relocation.target
            ))
        })?;
        let target = target
            .checked_add_signed(relocation.addend)
            .ok_or_else(|| {
                InstallationDiagnostic(format!(
                    "artifact relocation target overflows after addend {}",
                    relocation.addend
                ))
            })?;
        apply_relocation(
            &mut bytes,
            record.architecture,
            placement.extent.base(),
            *relocation,
            target,
        )?;
    }

    let final_bytes = normalized_final_bytes_identity(
        record.content,
        placement.extent.base(),
        record.architecture,
        &bytes,
    )?;
    Ok(MaterializedArtifactBytes {
        admission: artifact.clone(),
        placement: CodePlacementEvidence::from_placement(placement),
        placement_plan: record.placement_plan,
        base_address: placement.extent.base(),
        bytes,
        final_bytes,
    })
}

fn apply_relocation(
    bytes: &mut [u8],
    architecture: Architecture,
    base_address: u64,
    relocation: DecodedArtifactRelocation,
    target: u64,
) -> Result<(), InstallationDiagnostic> {
    let offset = usize::try_from(relocation.destination_offset).map_err(|_| {
        InstallationDiagnostic("artifact relocation offset does not fit the provider host".into())
    })?;
    match (architecture, relocation.kind) {
        (_, ArtifactRelocationKind::Absolute64) => write_u64(bytes, offset, target),
        (Architecture::X86_64, ArtifactRelocationKind::X86Relative32) => {
            let next_instruction = base_address
                .checked_add(relocation.destination_offset)
                .and_then(|address| address.checked_add(4))
                .ok_or_else(|| {
                    InstallationDiagnostic(
                        "x86 relative relocation instruction address overflows".into(),
                    )
                })?;
            let delta = i128::from(target) - i128::from(next_instruction);
            let delta = i32::try_from(delta).map_err(|_| {
                InstallationDiagnostic(format!(
                    "x86 relative relocation is out of range: {delta} byte(s)"
                ))
            })?;
            write_i32(bytes, offset, delta)
        }
        (Architecture::Aarch64, ArtifactRelocationKind::Aarch64Page21) => {
            let instruction_address = base_address
                .checked_add(relocation.destination_offset)
                .ok_or_else(|| {
                    InstallationDiagnostic("AArch64 instruction address overflows".into())
                })?;
            patch_aarch64_adrp(bytes, offset, instruction_address, target)
        }
        (Architecture::Aarch64, ArtifactRelocationKind::Aarch64PageOffset12) => {
            patch_aarch64_add_page_offset(bytes, offset, target)
        }
        (Architecture::Aarch64, ArtifactRelocationKind::Aarch64Branch26) => {
            let instruction_address = base_address
                .checked_add(relocation.destination_offset)
                .ok_or_else(|| {
                    InstallationDiagnostic("AArch64 instruction address overflows".into())
                })?;
            patch_aarch64_branch26(bytes, offset, instruction_address, target)
        }
        (actual, authored) => Err(InstallationDiagnostic(format!(
            "artifact relocation {authored:?} is incompatible with architecture {actual:?}"
        ))),
    }
}

fn patch_aarch64_adrp(
    bytes: &mut [u8],
    offset: usize,
    instruction_address: u64,
    target: u64,
) -> Result<(), InstallationDiagnostic> {
    let mut instruction = read_u32(bytes, offset)?;
    if instruction & 0x9f00_0000 != 0x9000_0000 {
        return Err(InstallationDiagnostic(
            "AArch64 page relocation does not target an ADRP instruction".into(),
        ));
    }
    let instruction_page = instruction_address & !0xfff;
    let target_page = target & !0xfff;
    let page_delta = i128::from(target_page) - i128::from(instruction_page);
    let page_delta = page_delta / 4096;
    if !(-(1_i128 << 20)..(1_i128 << 20)).contains(&page_delta) {
        return Err(InstallationDiagnostic(format!(
            "AArch64 ADRP relocation is out of range: {page_delta} page(s)"
        )));
    }
    let immediate = (page_delta as u32) & 0x1f_ffff;
    instruction &= !((0b11 << 29) | (0x7ffff << 5));
    instruction |= ((immediate & 0b11) << 29) | (((immediate >> 2) & 0x7ffff) << 5);
    write_u32(bytes, offset, instruction)
}

fn patch_aarch64_add_page_offset(
    bytes: &mut [u8],
    offset: usize,
    target: u64,
) -> Result<(), InstallationDiagnostic> {
    let mut instruction = read_u32(bytes, offset)?;
    if instruction & 0x7f00_0000 != 0x1100_0000 {
        return Err(InstallationDiagnostic(
            "AArch64 page-offset relocation does not target an ADD-immediate instruction".into(),
        ));
    }
    instruction &= !(0xfff << 10);
    instruction |= ((target & 0xfff) as u32) << 10;
    write_u32(bytes, offset, instruction)
}

fn patch_aarch64_branch26(
    bytes: &mut [u8],
    offset: usize,
    instruction_address: u64,
    target: u64,
) -> Result<(), InstallationDiagnostic> {
    let mut instruction = read_u32(bytes, offset)?;
    if instruction & 0x7c00_0000 != 0x1400_0000 {
        return Err(InstallationDiagnostic(
            "AArch64 branch relocation does not target a B/BL instruction".into(),
        ));
    }
    let delta = i128::from(target) - i128::from(instruction_address);
    if delta % 4 != 0 {
        return Err(InstallationDiagnostic(
            "AArch64 branch relocation target is not instruction-aligned".into(),
        ));
    }
    let immediate = delta / 4;
    if !(-(1_i128 << 25)..(1_i128 << 25)).contains(&immediate) {
        return Err(InstallationDiagnostic(format!(
            "AArch64 branch relocation is out of range: {immediate} instruction(s)"
        )));
    }
    instruction &= !0x03ff_ffff;
    instruction |= (immediate as u32) & 0x03ff_ffff;
    write_u32(bytes, offset, instruction)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InstallationDiagnostic> {
    let range = bytes.get(offset..offset.saturating_add(4)).ok_or_else(|| {
        InstallationDiagnostic("artifact relocation lies outside executable bytes".into())
    })?;
    Ok(u32::from_le_bytes(
        range
            .try_into()
            .expect("validated four-byte relocation slice"),
    ))
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) -> Result<(), InstallationDiagnostic> {
    write_bytes(bytes, offset, &value.to_le_bytes())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), InstallationDiagnostic> {
    write_bytes(bytes, offset, &value.to_le_bytes())
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), InstallationDiagnostic> {
    write_bytes(bytes, offset, &value.to_le_bytes())
}

fn write_bytes(
    bytes: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), InstallationDiagnostic> {
    let end = offset
        .checked_add(value.len())
        .ok_or_else(|| InstallationDiagnostic("artifact relocation range overflows".into()))?;
    let destination = bytes.get_mut(offset..end).ok_or_else(|| {
        InstallationDiagnostic("artifact relocation lies outside executable bytes".into())
    })?;
    destination.copy_from_slice(value);
    Ok(())
}

fn normalized_final_bytes_identity(
    content: ArtifactContentDigest,
    base_address: u64,
    architecture: Architecture,
    bytes: &[u8],
) -> Result<FinalBytesDigest, InstallationDiagnostic> {
    let mut digest = Sha256::new();
    digest.update(b"omega.materialized-final-bytes.sha256.v1\0");
    digest.update(content.digest());
    digest.update(base_address.to_le_bytes());
    digest.update([match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    Ok(FinalBytesDigest::from_digest(digest.finalize().into()))
}
