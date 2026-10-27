//! Canonical terminal-machine envelope wire format.
//!
//! This module owns machine identity, parameters/results, structural places,
//! entry/content claims, service ceilings, ordered block envelopes, and the
//! retained contract. Block bodies and their operation vocabulary remain in
//! the parent codec.

#[cfg(test)]
mod tests;

use terminal_psi::{
    EntryClaim, StructuralPlaceDeclaration, TerminalBlockNaturalRank, TerminalMachine,
    TerminalMachineResult, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc, ValueDeclaration,
};

use super::CodecError;
use super::content_wire::{
    decode_content_entry_claim, decode_content_identity_reshuffle,
    decode_content_partition_composition, encode_content_entry_claim,
    encode_content_identity_reshuffle, encode_content_partition_composition,
};
use super::contract_wire::{decode_contract, encode_contract};
use super::scalar_wire::{
    decode_integer_type, decode_scalar_type, encode_integer_type, encode_scalar_type,
};
use super::structural_result_wire::{decode_function_result, encode_function_result};
use super::structural_signature_wire::{
    decode_structural_parameters, encode_service_ceiling, encode_structural_parameters,
};
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, decode_structural_place_kind, encode_structural_path,
    encode_structural_place_kind,
};
use crate::sections::semantic_module::wire::{
    decode_counted, decode_ids, decode_optional_id, encode_optional_id,
};

pub(crate) fn encode_machine(
    writer: &mut Writer,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    writer.id(machine.id);
    writer.u8(match machine.assembly_target_requirement {
        terminal_psi::RegisterTargetApplicability::Any => 0,
        terminal_psi::RegisterTargetApplicability::X86_64 => 1,
        terminal_psi::RegisterTargetApplicability::Aarch64 => 2,
    });
    writer.boolean(machine.declaration.is_some());
    if let Some(declaration) = &machine.declaration {
        super::machine_declaration_wire::encode(writer, declaration)?;
    }
    encode_optional_id(writer, machine.attachment);
    encode_optional_id(writer, machine.nominal_cleanup_subject);
    encode_declarations(writer, "machine parameters", &machine.parameters)?;
    writer.len("machine parameter order", machine.parameter_order.len())?;
    for kind in &machine.parameter_order {
        writer.u8(match kind {
            terminal_psi::BoundaryParameterKind::Scalar => 0,
            terminal_psi::BoundaryParameterKind::Structural => 1,
        });
    }
    encode_structural_parameters(writer, &machine.structural_parameters)?;
    encode_ranked_scc(writer, machine.ranked_scc.as_ref())?;
    match &machine.result {
        TerminalMachineResult::Unit => writer.u8(0),
        TerminalMachineResult::Scalar(result) => {
            writer.u8(1);
            encode_declaration(writer, *result);
        }
        TerminalMachineResult::Structural(result) => {
            writer.u8(2);
            encode_function_result(writer, result)?;
        }
    }
    writer.len("structural places", machine.structural_places.len())?;
    for place in &machine.structural_places {
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    writer.len("entry claims", machine.entry_claims.len())?;
    for claim in &machine.entry_claims {
        writer.id(claim.claim);
        writer.id(claim.input);
        encode_structural_path(writer, "entry claim path", &claim.path)?;
    }
    writer.len("block entry claims", machine.block_entry_claims.len())?;
    for claim in &machine.block_entry_claims {
        writer.id(claim.claim);
        writer.id(claim.input);
        encode_structural_path(writer, "block entry claim path", &claim.path)?;
    }
    writer.len("loan completions", machine.loan_completions.len())?;
    for completion in &machine.loan_completions {
        writer.id(completion.operation);
        writer.id(completion.place);
        encode_structural_path(writer, "loan completion region", &completion.path)?;
        super::structural_result_wire::encode_multiplicity(writer, completion.multiplicity);
        writer.len("loan completion claims", completion.claims.len())?;
        for claim in &completion.claims {
            writer.id(claim.claim);
            encode_structural_path(writer, "loan completion claim path", &claim.path)?;
        }
    }
    encode_service_ceiling(writer, &machine.declared_service_reach)?;
    encode_service_ceiling(writer, &machine.published_service_ceiling)?;
    super::reach_application_wire::encode(writer, machine.closed_reach_application.as_ref())?;
    writer.len("content entry claims", machine.content_entry_claims.len())?;
    for binding in &machine.content_entry_claims {
        encode_content_entry_claim(writer, binding)?;
    }
    writer.len(
        "content identity reshuffles",
        machine.content_identity_reshuffles.len(),
    )?;
    for reshuffle in &machine.content_identity_reshuffles {
        encode_content_identity_reshuffle(writer, reshuffle)?;
    }
    writer.len(
        "content partition compositions",
        machine.content_partition_compositions.len(),
    )?;
    for composition in &machine.content_partition_compositions {
        encode_content_partition_composition(writer, composition)?;
    }
    writer.id(machine.entry);
    writer.len("blocks", machine.blocks.len())?;
    for block in &machine.blocks {
        super::block_wire::encode_block(writer, block)?;
    }
    encode_contract(writer, &machine.contract)
}

pub(crate) fn encode_declarations(
    writer: &mut Writer,
    label: &'static str,
    declarations: &[ValueDeclaration],
) -> Result<(), CodecError> {
    writer.len(label, declarations.len())?;
    for declaration in declarations {
        encode_declaration(writer, *declaration);
    }
    Ok(())
}

pub(crate) fn encode_declaration(writer: &mut Writer, declaration: ValueDeclaration) {
    writer.id(declaration.id);
    encode_scalar_type(writer, declaration.scalar_type);
    writer.u64(declaration.qualifications.get());
}

pub(crate) fn decode_machine(reader: &mut Reader<'_>) -> Result<TerminalMachine, CodecError> {
    let id = reader.id("MachineId")?;
    let assembly_target_requirement = match reader.u8()? {
        0 => terminal_psi::RegisterTargetApplicability::Any,
        1 => terminal_psi::RegisterTargetApplicability::X86_64,
        2 => terminal_psi::RegisterTargetApplicability::Aarch64,
        _ => return crate::codec_error::malformed("unknown machine assembly target requirement"),
    };
    let declaration = if reader.boolean()? {
        Some(super::machine_declaration_wire::decode(reader)?)
    } else {
        None
    };
    let attachment = decode_optional_id(reader, "StructuralTypeId")?;
    let nominal_cleanup_subject = decode_optional_id(reader, "StructuralTypeId")?;
    let parameters = decode_declarations(reader)?;
    let parameter_order = decode_counted(reader, |reader| match reader.u8()? {
        0 => Ok(terminal_psi::BoundaryParameterKind::Scalar),
        1 => Ok(terminal_psi::BoundaryParameterKind::Structural),
        tag => Err(CodecError::InvalidTag("machine parameter kind", tag)),
    })?;
    let structural_parameters = decode_structural_parameters(reader)?;
    let ranked_scc = decode_ranked_scc(reader)?;
    let result = match reader.u8()? {
        0 => TerminalMachineResult::Unit,
        1 => TerminalMachineResult::Scalar(decode_declaration(reader)?),
        2 => TerminalMachineResult::Structural(decode_function_result(reader)?),
        tag => return Err(CodecError::InvalidTag("TerminalMachineResult", tag)),
    };
    let structural_places = decode_counted(reader, |reader| {
        Ok(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        })
    })?;
    let entry_claims = decode_counted(reader, |reader| {
        Ok(EntryClaim {
            claim: reader.id("ClaimId")?,
            input: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
        })
    })?;
    let block_entry_claims = decode_counted(reader, |reader| {
        Ok(EntryClaim {
            claim: reader.id("ClaimId")?,
            input: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
        })
    })?;
    let loan_completions = decode_counted(reader, |reader| {
        Ok(terminal_psi::LoanCompletion {
            operation: reader.id("OperationId")?,
            place: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
            multiplicity: super::structural_result_wire::decode_multiplicity(reader)?,
            claims: decode_counted(reader, |reader| {
                Ok(terminal_psi::StructuralResultClaimBinding {
                    claim: reader.id("ClaimId")?,
                    path: decode_structural_path(reader)?,
                })
            })?,
        })
    })?;
    let declared_service_reach = decode_ids(reader, "ServiceId")?;
    let published_service_ceiling = decode_ids(reader, "ServiceId")?;
    let closed_reach_application = super::reach_application_wire::decode(reader)?;
    let content_entry_claims = decode_counted(reader, decode_content_entry_claim)?;
    let content_identity_reshuffles = decode_counted(reader, decode_content_identity_reshuffle)?;
    let content_partition_compositions =
        decode_counted(reader, decode_content_partition_composition)?;
    let entry = reader.id("BlockId")?;
    let blocks = decode_counted(reader, super::block_wire::decode_block)?;
    let contract = decode_contract(reader)?;
    Ok(TerminalMachine {
        assembly_target_requirement,
        id,
        declaration,
        attachment,
        nominal_cleanup_subject,
        parameters,
        parameter_order,
        structural_parameters,
        ranked_scc,
        result,
        structural_places,
        entry_claims,
        block_entry_claims,
        loan_completions,
        declared_service_reach,
        published_service_ceiling,
        closed_reach_application,
        content_entry_claims,
        content_identity_reshuffles,
        content_partition_compositions,
        entry,
        blocks,
        contract,
    })
}

fn encode_ranked_scc(
    writer: &mut Writer,
    ranked_scc: Option<&TerminalRankedScc>,
) -> Result<(), CodecError> {
    let Some(ranking) = ranked_scc else {
        writer.u8(0);
        return Ok(());
    };
    match ranking {
        TerminalRankedScc::Natural(components) => {
            writer.u8(2);
            writer.len("natural cycle components", components.len())?;
            for component in components {
                encode_integer_type(writer, component.rank_type);
                writer.len("natural block ranks", component.ranks.len())?;
                for rank in &component.ranks {
                    writer.id(rank.block);
                    writer.id(rank.value);
                }
                writer.len("natural rank edges", component.edges.len())?;
                for edge in &component.edges {
                    writer.id(edge.edge);
                    writer.id(edge.source);
                    writer.id(edge.target);
                    writer.id(edge.successor_rank);
                    writer.u8(match edge.comparison {
                        TerminalNaturalRankComparison::Preserving => 1,
                        TerminalNaturalRankComparison::Strict => 2,
                    });
                }
            }
        }
    }
    Ok(())
}

fn decode_ranked_scc(reader: &mut Reader<'_>) -> Result<Option<TerminalRankedScc>, CodecError> {
    match reader.u8()? {
        0 => Ok(None),
        2 => Ok(Some(TerminalRankedScc::Natural(decode_counted(
            reader,
            |reader| {
                Ok(TerminalNaturalCycle {
                    rank_type: decode_integer_type(reader)?,
                    ranks: decode_counted(reader, |reader| {
                        Ok(TerminalBlockNaturalRank {
                            block: reader.id("BlockId")?,
                            value: reader.id("ValueId")?,
                        })
                    })?,
                    edges: decode_counted(reader, |reader| {
                        Ok(TerminalNaturalRankEdge {
                            edge: reader.id("EdgeId")?,
                            source: reader.id("BlockId")?,
                            target: reader.id("BlockId")?,
                            successor_rank: reader.id("ValueId")?,
                            comparison: match reader.u8()? {
                                1 => TerminalNaturalRankComparison::Preserving,
                                2 => TerminalNaturalRankComparison::Strict,
                                tag => {
                                    return Err(CodecError::InvalidTag(
                                        "TerminalNaturalRankComparison",
                                        tag,
                                    ));
                                }
                            },
                        })
                    })?,
                })
            },
        )?))),
        tag => Err(CodecError::InvalidTag("TerminalRankedScc", tag)),
    }
}

pub(crate) fn decode_declarations(
    reader: &mut Reader<'_>,
) -> Result<Vec<ValueDeclaration>, CodecError> {
    decode_counted(reader, decode_declaration)
}

pub(crate) fn decode_declaration(reader: &mut Reader<'_>) -> Result<ValueDeclaration, CodecError> {
    Ok(ValueDeclaration {
        id: reader.id("ValueId")?,
        scalar_type: decode_scalar_type(reader)?,
        qualifications: semantic_vocabulary::ScalarQualificationSetId::new(reader.u64()?),
    })
}
