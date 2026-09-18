//! Linkage candidates, derived contents and their validation.

use crate::dynamic_executable::file_assembly::dynamic_file_envelope::{ValidatedElfDynamicFileEnvelope};
use crate::dynamic_executable::section_headers::section_payload_roster::{ElfIndexedProcedureFixup};
use crate::dynamic_executable::section_headers::section_roster::{ElfDynamicRosterSectionKind};
use diagnostics::{Diagnostic};
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::{ElfAppliedProcedureLinkageFixup, ElfAppliedProcedureLinkageStorage, ValidatedElfResolvedProcedureLinkage};
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::compatibility_fingerprint::{non_authoritative_resolved_linkage_compatibility_fingerprint};
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::storage_fields::{checked_sum_usize, decode_rejoins_target, encode_field, indexed_payloads, indexed_row_bytes, load_layout, public_kind, public_storage, public_target, read_field, semantic_target_address, storage_address, storage_bytes, storage_bytes_mut, upstream_storage_bytes, write_field};
use crate::dynamic_executable::checked::{checked_u32, require};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfResolvedProcedureLinkageContents {
    pub(crate) source_text_bytes: Vec<u8>,
    pub(crate) procedure_linkage_bytes: Vec<u8>,
    pub(crate) procedure_got_bytes: Vec<u8>,
    pub(crate) procedure_relocation_bytes: Vec<u8>,
    pub(crate) applications: Vec<ElfAppliedProcedureLinkageFixup>,
}

pub(crate) struct Candidate {
    pub(super) envelope: ValidatedElfDynamicFileEnvelope,
    pub(super) contents: ElfResolvedProcedureLinkageContents,
    pub(super) non_authoritative_resolved_linkage_compatibility_fingerprint: u64,
}

pub(super) struct CandidateValidationError {
    pub(super) candidate: Candidate,
    pub(super) diagnostic: Diagnostic,
}

pub(crate) fn derive_contents(
    envelope: &ValidatedElfDynamicFileEnvelope,
) -> Result<ElfResolvedProcedureLinkageContents, Diagnostic> {
    let payloads = indexed_payloads(envelope);
    let mut contents = ElfResolvedProcedureLinkageContents {
        source_text_bytes: load_layout(envelope).retained_image().memory.text.clone(),
        procedure_linkage_bytes: indexed_row_bytes(
            payloads,
            8,
            ElfDynamicRosterSectionKind::ProcedureLinkage,
        )?
        .to_vec(),
        procedure_got_bytes: indexed_row_bytes(
            payloads,
            9,
            ElfDynamicRosterSectionKind::ProcedureGot,
        )?
        .to_vec(),
        procedure_relocation_bytes: indexed_row_bytes(
            payloads,
            10,
            ElfDynamicRosterSectionKind::ProcedureRelocation,
        )?
        .to_vec(),
        applications: Vec::with_capacity(payloads.procedure_fixups.len()),
    };
    for fixup in &payloads.procedure_fixups {
        let application = derive_application(envelope, &contents, fixup)?;
        write_field(
            storage_bytes_mut(&mut contents, application.storage),
            application.byte_offset,
            application.byte_width,
            application.encoded_field,
        )?;
        contents.applications.push(application);
    }
    Ok(contents)
}

fn derive_application(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
    fixup: &ElfIndexedProcedureFixup,
) -> Result<ElfAppliedProcedureLinkageFixup, Diagnostic> {
    let storage = public_storage(fixup.storage)?;
    let source_address = storage_address(load_layout(envelope), fixup)?;
    let target_address = semantic_target_address(load_layout(envelope), fixup)?;
    let original = read_field(
        storage_bytes(contents, storage),
        fixup.byte_offset,
        fixup.byte_width,
    )?;
    require(
        original & fixup.mutable_mask == 0,
        "procedure-linkage fixup source is not an exact zero placeholder",
    )?;
    let encoded_field = encode_field(
        fixup.kind,
        original,
        fixup.mutable_mask,
        source_address,
        target_address,
    )?;
    Ok(ElfAppliedProcedureLinkageFixup {
        ordinal: fixup.upstream_ordinal,
        storage,
        byte_offset: fixup.byte_offset,
        byte_width: fixup.byte_width,
        mutable_mask: fixup.mutable_mask,
        kind: public_kind(fixup.kind),
        target: public_target(fixup.target),
        source_address,
        target_address,
        encoded_field,
    })
}

pub(crate) fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfResolvedProcedureLinkage, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.envelope, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    let expected = non_authoritative_resolved_linkage_compatibility_fingerprint(
        &candidate.envelope,
        &candidate.contents,
    );
    if candidate.non_authoritative_resolved_linkage_compatibility_fingerprint == 0
        || candidate.non_authoritative_resolved_linkage_compatibility_fingerprint != expected
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "resolved procedure-linkage compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfResolvedProcedureLinkage {
        envelope: candidate.envelope,
        contents: candidate.contents,
        non_authoritative_resolved_linkage_compatibility_fingerprint: candidate
            .non_authoritative_resolved_linkage_compatibility_fingerprint,
    })
}

fn validate_contents(
    envelope: &ValidatedElfDynamicFileEnvelope,
    contents: &ElfResolvedProcedureLinkageContents,
) -> Result<(), Diagnostic> {
    let payloads = indexed_payloads(envelope);
    require(
        contents.source_text_bytes.len()
            == load_layout(envelope).retained_image().memory.text.len()
            && contents.procedure_linkage_bytes.len()
                == indexed_row_bytes(payloads, 8, ElfDynamicRosterSectionKind::ProcedureLinkage)?
                    .len()
            && contents.procedure_got_bytes.len()
                == indexed_row_bytes(payloads, 9, ElfDynamicRosterSectionKind::ProcedureGot)?.len()
            && contents.procedure_relocation_bytes.len()
                == indexed_row_bytes(
                    payloads,
                    10,
                    ElfDynamicRosterSectionKind::ProcedureRelocation,
                )?
                .len(),
        "resolved procedure-linkage fragment length drifted from exact upstream storage",
    )?;
    require(
        contents.applications.len() == payloads.procedure_fixups.len(),
        "resolved procedure-linkage application coverage is incomplete or duplicated",
    )?;
    validate_nonoverlapping_applications(&contents.applications)?;

    for (ordinal, (application, fixup)) in contents
        .applications
        .iter()
        .zip(&payloads.procedure_fixups)
        .enumerate()
    {
        require(
            application.ordinal == checked_u32(ordinal, "procedure application ordinal")?
                && application.ordinal == fixup.upstream_ordinal
                && application.storage == public_storage(fixup.storage)?
                && application.byte_offset == fixup.byte_offset
                && application.byte_width == fixup.byte_width
                && application.mutable_mask == fixup.mutable_mask
                && application.kind == public_kind(fixup.kind)
                && application.target == public_target(fixup.target),
            "resolved procedure-linkage application drifted from its indexed fixup",
        )?;
        let expected_source = storage_address(load_layout(envelope), fixup)?;
        let expected_target = semantic_target_address(load_layout(envelope), fixup)?;
        require(
            application.source_address == expected_source
                && application.target_address == expected_target,
            "resolved procedure-linkage source or target address drifted from absolute layout",
        )?;
        let original = read_field(
            upstream_storage_bytes(envelope, application.storage)?,
            application.byte_offset,
            application.byte_width,
        )?;
        require(
            original & application.mutable_mask == 0,
            "retained procedure-linkage template no longer has an exact zero placeholder",
        )?;
        let expected_field = encode_field(
            fixup.kind,
            original,
            application.mutable_mask,
            application.source_address,
            application.target_address,
        )?;
        let actual_field = read_field(
            storage_bytes(contents, application.storage),
            application.byte_offset,
            application.byte_width,
        )?;
        require(
            application.encoded_field == expected_field && actual_field == expected_field,
            "resolved procedure-linkage encoded field does not replay",
        )?;
        require(
            (actual_field & !application.mutable_mask) == (original & !application.mutable_mask),
            "resolved procedure-linkage application changed fixed opcode bits",
        )?;
        decode_rejoins_target(
            fixup.kind,
            actual_field,
            application.source_address,
            application.target_address,
        )?;
    }

    for storage in [
        ElfAppliedProcedureLinkageStorage::SourceText,
        ElfAppliedProcedureLinkageStorage::ProcedureLinkage,
        ElfAppliedProcedureLinkageStorage::ProcedureGot,
        ElfAppliedProcedureLinkageStorage::ProcedureRelocation,
    ] {
        validate_unchanged_bytes(
            upstream_storage_bytes(envelope, storage)?,
            storage_bytes(contents, storage),
            storage,
            &contents.applications,
        )?;
    }
    Ok(())
}

pub(crate) fn validate_nonoverlapping_applications(
    applications: &[ElfAppliedProcedureLinkageFixup],
) -> Result<(), Diagnostic> {
    for (index, application) in applications.iter().enumerate() {
        let end = checked_sum_usize(
            application.byte_offset,
            usize::from(application.byte_width),
            "procedure application end",
        )?;
        for other in &applications[index + 1..] {
            if application.storage != other.storage {
                continue;
            }
            let other_end = checked_sum_usize(
                other.byte_offset,
                usize::from(other.byte_width),
                "procedure application end",
            )?;
            require(
                end <= other.byte_offset || other_end <= application.byte_offset,
                "resolved procedure-linkage applications overlap or duplicate one field",
            )?;
        }
    }
    Ok(())
}

fn validate_unchanged_bytes(
    original: &[u8],
    resolved: &[u8],
    storage: ElfAppliedProcedureLinkageStorage,
    applications: &[ElfAppliedProcedureLinkageFixup],
) -> Result<(), Diagnostic> {
    require(
        original.len() == resolved.len(),
        "resolved procedure-linkage storage length drifted",
    )?;
    for (offset, (before, after)) in original.iter().zip(resolved).enumerate() {
        let mutable = applications.iter().any(|application| {
            application.storage == storage
                && offset >= application.byte_offset
                && offset
                    < application
                        .byte_offset
                        .saturating_add(usize::from(application.byte_width))
        });
        require(
            mutable || before == after,
            "resolved procedure-linkage application changed an unowned byte",
        )?;
    }
    Ok(())
}
