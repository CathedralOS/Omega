//! Proof recursive components on the wire.

use crate::sections::semantic_module::CodecError;
use crate::sections::semantic_module::wire::{Reader, Writer, decode_counted};
use terminal_psi::TerminalIdentityIndex;
use terminal_psi::{
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
};

pub(crate) fn encode_proof_recursive_component(
    writer: &mut Writer,
    component: &TerminalProofRecursiveComponent,
) -> Result<(), CodecError> {
    writer.u8(match component.ranking_relation {
        TerminalProofRankingRelation::StructuralSubterm => 1,
        TerminalProofRankingRelation::StructuralSubtermCycles => 2,
    });
    writer.u32(component.rank_type_identity.0);
    writer.len("proof recursive types", component.types.len())?;
    for proof_type in &component.types {
        writer.u32(proof_type.identity.0);
        writer.len("proof recursive type fields", proof_type.fields.len())?;
        for field in &proof_type.fields {
            writer.u32(field.identity.0);
            writer.u32(field.type_identity.0);
        }
    }
    writer.len("proof recursive members", component.members.len())?;
    for member in &component.members {
        writer.id(member.contract);
        writer.u32(member.machine_identity.0);
        writer.u32(member.rank_parameter_identity.0);
    }
    writer.len("proof recursive edges", component.edges.len())?;
    for edge in &component.edges {
        writer.id(edge.caller);
        writer.id(edge.callee);
        match &edge.site {
            TerminalProofRecursiveCallSite::Statement {
                state_identity,
                statement_index,
            } => {
                writer.u8(1);
                writer.u32(state_identity.0);
                writer.u64(*statement_index);
            }
            TerminalProofRecursiveCallSite::Expression {
                state_identity,
                statement_index,
                expression_ordinal,
            } => {
                writer.u8(2);
                writer.u32(state_identity.0);
                writer.u64(*statement_index);
                writer.u64(*expression_ordinal);
            }
            TerminalProofRecursiveCallSite::Transition {
                state_identity,
                statement_index,
                lane,
            } => {
                writer.u8(3);
                writer.u32(state_identity.0);
                writer.u64(*statement_index);
                writer.u8(match lane {
                    TerminalProofRecursiveTransitionLane::Target => 1,
                    TerminalProofRecursiveTransitionLane::Continuation => 2,
                });
            }
        }
        writer.strings(
            "proof recursive strict member path",
            &edge.strict_member_path,
        )?;
    }
    Ok(())
}

pub(crate) fn decode_proof_recursive_component(
    reader: &mut Reader<'_>,
) -> Result<TerminalProofRecursiveComponent, CodecError> {
    let ranking_relation = match reader.u8()? {
        1 => TerminalProofRankingRelation::StructuralSubterm,
        2 => TerminalProofRankingRelation::StructuralSubtermCycles,
        tag => return Err(CodecError::InvalidTag("TerminalProofRankingRelation", tag)),
    };
    let rank_type_identity = TerminalIdentityIndex(reader.u32()?);
    let types = decode_counted(reader, |reader| {
        Ok(TerminalProofRecursiveType {
            identity: TerminalIdentityIndex(reader.u32()?),
            fields: decode_counted(reader, |reader| {
                Ok(TerminalProofRecursiveField {
                    identity: TerminalIdentityIndex(reader.u32()?),
                    type_identity: TerminalIdentityIndex(reader.u32()?),
                })
            })?,
        })
    })?;
    let members = decode_counted(reader, |reader| {
        Ok(TerminalProofRecursiveMember {
            contract: reader.id("ContractId")?,
            machine_identity: TerminalIdentityIndex(reader.u32()?),
            rank_parameter_identity: TerminalIdentityIndex(reader.u32()?),
        })
    })?;
    let edges = decode_counted(reader, |reader| {
        let caller = reader.id("ContractId")?;
        let callee = reader.id("ContractId")?;
        let site = match reader.u8()? {
            1 => TerminalProofRecursiveCallSite::Statement {
                state_identity: TerminalIdentityIndex(reader.u32()?),
                statement_index: reader.u64()?,
            },
            2 => TerminalProofRecursiveCallSite::Expression {
                state_identity: TerminalIdentityIndex(reader.u32()?),
                statement_index: reader.u64()?,
                expression_ordinal: reader.u64()?,
            },
            3 => TerminalProofRecursiveCallSite::Transition {
                state_identity: TerminalIdentityIndex(reader.u32()?),
                statement_index: reader.u64()?,
                lane: match reader.u8()? {
                    1 => TerminalProofRecursiveTransitionLane::Target,
                    2 => TerminalProofRecursiveTransitionLane::Continuation,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "TerminalProofRecursiveTransitionLane",
                            tag,
                        ));
                    }
                },
            },
            tag => {
                return Err(CodecError::InvalidTag(
                    "TerminalProofRecursiveCallSite",
                    tag,
                ));
            }
        };
        Ok(TerminalProofRecursiveEdge {
            caller,
            callee,
            site,
            strict_member_path: reader.strings("proof recursive strict member path")?,
        })
    })?;
    Ok(TerminalProofRecursiveComponent {
        ranking_relation,
        rank_type_identity,
        types,
        members,
        edges,
    })
}
