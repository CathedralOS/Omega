//! Canonical terminal-machine envelope wire format.
//!
//! This module owns machine identity, parameters/results, structural places,
//! entry/content claims, service ceilings, ordered block envelopes, and the
//! retained contract. Block bodies and their operation vocabulary remain in
//! the parent codec.

#[cfg(test)]
mod tests;

use terminal_psi::{
    BoundaryParameterKind, EntryClaim, LoanCompletion, RegisterTargetApplicability,
    StructuralPlaceDeclaration, TerminalBlockNaturalRank, TerminalIntegerRankValue,
    TerminalMachine, TerminalMachineResult, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc, ValueDeclaration,
};

use super::CodecError;
use super::scalar_wire::{
    decode_integer_type, decode_scalar_type, encode_integer_type, encode_scalar_type,
};
use super::structural_place_wire::{decode_structural_place_kind, encode_structural_place_kind};
use super::structural_result_wire::{decode_function_result, encode_function_result};
use super::structural_signature_wire::StructuralParameter;
use super::wire::{Reader, Writer};
use super::wire_fields::{
    ClaimBinding, Field, Id, Ids, List, MachineDeclaration, Multiplicity, Optional, OptionalId,
    Path, QualificationSet, Scalar, delegated_fields, wire_enum, wire_record,
};
use crate::sections::semantic_module::wire::decode_counted;

wire_record! {
    Machine: TerminalMachine {
        id: Id,
        assembly_target_requirement: AssemblyTarget,
        declaration: Optional<MachineDeclaration>,
        attachment: OptionalId,
        nominal_cleanup_subject: OptionalId,
        parameters: List<Declaration>,
        parameter_order: List<ParameterKind>,
        structural_parameters: List<StructuralParameter>,
        ranked_scc: RankedScc,
        result: MachineResult,
        structural_places: List<Place>,
        entry_claims: List<Claim>,
        block_entry_claims: List<Claim>,
        loan_completions: List<LoanCompletionRow>,
        declared_service_reach: Ids,
        published_service_ceiling: Ids,
        closed_reach_application: ReachApplication,
        content_entry_claims: List<super::content_wire::EntryClaim>,
        content_identity_reshuffles: List<super::content_wire::Reshuffle>,
        content_partition_compositions: List<super::content_wire::PartitionComposition>,
        entry: Id,
        blocks: List<BlockRow>,
        contract: super::contract_wire::Contract,
    }
    Declaration: ValueDeclaration { id: Id, scalar_type: Scalar, qualifications: QualificationSet }
    Place: StructuralPlaceDeclaration { id: Id, kind: PlaceKind }
    Claim: EntryClaim { claim: Id, input: Id, path: Path }
    LoanCompletionRow: LoanCompletion {
        operation: Id,
        place: Id,
        path: Path,
        multiplicity: Multiplicity,
        claims: List<ClaimBinding>,
    }
}

wire_enum! {
    ParameterKind: BoundaryParameterKind, "machine parameter kind" { 0 => Scalar, 1 => Structural }
    MachineResult: TerminalMachineResult, "TerminalMachineResult" {
        0 => Unit,
        1 => Scalar(declaration: Declaration),
        2 => Structural(result: FunctionResult),
    }
}

delegated_fields! {
    FunctionResult: terminal_psi::StructuralResultDeclaration
        => encode_function_result, decode_function_result;
    BlockRow: terminal_psi::Block => super::block_wire::encode_block, super::block_wire::decode_block;
}

struct PlaceKind;
impl Field<semantic_vocabulary::StructuralPlaceKind> for PlaceKind {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::StructuralPlaceKind,
    ) -> Result<(), CodecError> {
        encode_structural_place_kind(writer, *value);
        Ok(())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<semantic_vocabulary::StructuralPlaceKind, CodecError> {
        decode_structural_place_kind(reader)
    }
}

/// The register family an assembly body requires; an unknown byte is malformed.
struct AssemblyTarget;
impl Field<RegisterTargetApplicability> for AssemblyTarget {
    fn write(writer: &mut Writer, value: &RegisterTargetApplicability) -> Result<(), CodecError> {
        writer.u8(match value {
            RegisterTargetApplicability::Any => 0,
            RegisterTargetApplicability::X86_64 => 1,
            RegisterTargetApplicability::Aarch64 => 2,
        });
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<RegisterTargetApplicability, CodecError> {
        Ok(match reader.u8()? {
            0 => RegisterTargetApplicability::Any,
            1 => RegisterTargetApplicability::X86_64,
            2 => RegisterTargetApplicability::Aarch64,
            _ => {
                return crate::codec_error::malformed(
                    "unknown machine assembly target requirement",
                );
            }
        })
    }
}

struct ReachApplication;
impl Field<Option<terminal_psi::ClosedReachApplication>> for ReachApplication {
    fn write(
        writer: &mut Writer,
        value: &Option<terminal_psi::ClosedReachApplication>,
    ) -> Result<(), CodecError> {
        super::reach_application_wire::encode(writer, value.as_ref())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<Option<terminal_psi::ClosedReachApplication>, CodecError> {
        super::reach_application_wire::decode(reader)
    }
}

struct RankedScc;
impl Field<Option<TerminalRankedScc>> for RankedScc {
    fn write(writer: &mut Writer, value: &Option<TerminalRankedScc>) -> Result<(), CodecError> {
        encode_ranked_scc(writer, value.as_ref())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Option<TerminalRankedScc>, CodecError> {
        decode_ranked_scc(reader)
    }
}

pub(crate) fn encode_machine(
    writer: &mut Writer,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    Machine::write(writer, machine)
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
    Machine::read(reader)
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
            // Tag 3 is the uniform typed-tuple record, including singletons.
            // The retired tag 2 encoded a single carrier/value per row.
            if components.is_empty() {
                return Err(CodecError::MalformedRanking(
                    "empty ranked component roster",
                ));
            }
            writer.u8(3);
            writer.len("integer cycle components", components.len())?;
            for component in components {
                if component.ranks.is_empty() {
                    return Err(CodecError::MalformedRanking("empty cyclic rank roster"));
                }
                writer.len("integer block ranks", component.ranks.len())?;
                for rank in &component.ranks {
                    writer.id(rank.block);
                    encode_rank_values(writer, component, rank.values)?;
                }
                writer.len("integer rank edges", component.edges.len())?;
                for edge in &component.edges {
                    writer.id(edge.edge);
                    writer.id(edge.source);
                    writer.id(edge.target);
                    encode_rank_values(writer, component, edge.successor_ranks)?;
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
        3 => {
            let components = decode_counted(reader, |reader| {
                let mut observations = arena::Arena::new();
                let ranks = decode_counted(reader, |reader| {
                    let block = reader.id("BlockId")?;
                    let values = decode_rank_values(reader, &mut observations)?;
                    Ok(TerminalBlockNaturalRank { block, values })
                })?;
                let edges = decode_counted(reader, |reader| {
                    let edge = reader.id("EdgeId")?;
                    let source = reader.id("BlockId")?;
                    let target = reader.id("BlockId")?;
                    let successor_ranks = decode_rank_values(reader, &mut observations)?;
                    let comparison = match reader.u8()? {
                        1 => TerminalNaturalRankComparison::Preserving,
                        2 => TerminalNaturalRankComparison::Strict,
                        tag => {
                            return Err(CodecError::InvalidTag(
                                "TerminalNaturalRankComparison",
                                tag,
                            ));
                        }
                    };
                    Ok(TerminalNaturalRankEdge {
                        edge,
                        source,
                        target,
                        successor_ranks,
                        comparison,
                    })
                })?;
                let component = TerminalNaturalCycle {
                    observations,
                    ranks,
                    edges,
                };
                if component.ranks.is_empty()
                    || component
                        .ranks
                        .iter()
                        .any(|rank| component.rank_values(rank.values).is_none())
                    || component
                        .edges
                        .iter()
                        .any(|edge| component.rank_values(edge.successor_ranks).is_none())
                {
                    return Err(CodecError::MalformedRanking(
                        "incomplete or differently typed rank tuple",
                    ));
                }
                Ok(component)
            })?;
            if components.is_empty() {
                return Err(CodecError::MalformedRanking(
                    "empty ranked component roster",
                ));
            }
            Ok(Some(TerminalRankedScc::Natural(components)))
        }
        tag => Err(CodecError::InvalidTag("TerminalRankedScc", tag)),
    }
}

fn encode_rank_values(
    writer: &mut Writer,
    component: &TerminalNaturalCycle,
    span: arena::HandleSpan<TerminalIntegerRankValue>,
) -> Result<(), CodecError> {
    let values = component
        .rank_values(span)
        .ok_or(CodecError::MalformedRanking(
            "incomplete or differently typed rank tuple",
        ))?;
    writer.len("integer rank tuple", values.len())?;
    for value in values {
        encode_integer_type(writer, value.integer_type);
        writer.id(value.value);
    }
    Ok(())
}

fn decode_rank_values(
    reader: &mut Reader<'_>,
    observations: &mut arena::Arena<TerminalIntegerRankValue>,
) -> Result<arena::HandleSpan<TerminalIntegerRankValue>, CodecError> {
    let values = decode_counted(reader, |reader| {
        Ok(TerminalIntegerRankValue {
            integer_type: decode_integer_type(reader)?,
            value: reader.id("ValueId")?,
        })
    })?;
    if values.is_empty() {
        return Err(CodecError::MalformedRanking("empty rank tuple"));
    }
    Ok(observations.insert_many(values))
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
