//! Canonical raw ranked custody transport. Decoding grants no verification authority.
use super::*;
use sha2::{Digest, Sha256};
use terminal_psi::{TerminalRankedGuard, TerminalRankedSccEdge, TerminalRankedSuccessorArgument};
mod values;
mod wire;
use wire::{Reader, Wire};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankedCustodyCodecError {
    Semantic(terminal_codec::CodecError),
    InvalidEncoding,
}
impl From<terminal_codec::CodecError> for RankedCustodyCodecError {
    fn from(error: terminal_codec::CodecError) -> Self {
        Self::Semantic(error)
    }
}
pub fn encode_ranked_u32_countdown_custody(
    custody: &RankedU32CountdownCustody,
) -> Result<Vec<u8>, RankedCustodyCodecError> {
    let mut bytes = b"omega.ranked-custody.v1\0".to_vec();
    custody.write(&mut bytes)?;
    Ok(bytes)
}
pub fn decode_ranked_u32_countdown_custody(
    bytes: &[u8],
) -> Result<RankedU32CountdownCustody, RankedCustodyCodecError> {
    let domain = b"omega.ranked-custody.v1\0";
    let mut reader = Reader(
        bytes
            .strip_prefix(domain)
            .ok_or(RankedCustodyCodecError::InvalidEncoding)?,
    );
    let custody = RankedU32CountdownCustody::read(&mut reader)?;
    if !reader.0.is_empty() || encode_ranked_u32_countdown_custody(&custody)? != bytes {
        return Err(RankedCustodyCodecError::InvalidEncoding);
    }
    Ok(custody)
}
pub fn ranked_u32_countdown_custody_identity(
    custody: &RankedU32CountdownCustody,
) -> Result<[u8; 32], RankedCustodyCodecError> {
    Ok(Sha256::digest(encode_ranked_u32_countdown_custody(custody)?).into())
}
macro_rules! record {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl Wire for $name {
            fn write(&self, bytes: &mut Vec<u8>) -> Result<(), RankedCustodyCodecError> { $( self.$field.write(bytes)?; )* Ok(()) }
            fn read(reader: &mut Reader<'_>) -> Result<Self, RankedCustodyCodecError> { Ok(Self { $( $field: Wire::read(reader)?, )* }) }
        }
    };
}
record!(RankedU32CountdownCustody {
    semantic_replay,
    proof_replay,
    ranked_scc,
    fixed_fuel,
    graph,
    structural_frontiers
});
record!(RankedFixedEntryFuel {
    terminal_psi,
    schedule,
    entry,
    relevant_preconditions,
    ceiling_units
});
record!(RankedU32CountdownGraph {
    entry,
    preheader_edge,
    initial_value,
    zero_operation,
    zero_value,
    compare_operation,
    false_exit_edge,
    done_block,
    one_operation,
    one_value,
    subtract_operation,
    subtract_obligation,
    return_edge
});
record!(RankedLiveClaim {
    claim,
    input,
    path,
    multiplicity
});
record!(RankedOwnedStructuralPlace {
    place,
    multiplicity
});
record!(RankedPartialStructuralCustody { place, moved_paths });
record!(RankedStructuralOwnershipFrontier {
    claims,
    owned_places,
    partial_custody
});
record!(RankedMachineStructuralFrontiers {
    machine,
    header,
    backedge,
    header_entry,
    backedge_exit
});
record!(TerminalRankedScc {
    header,
    rank_parameter,
    rank_type,
    lower_bound,
    upper_bound,
    covered_cyclic_edges
});
record!(TerminalRankedSccEdge {
    edge,
    source,
    target,
    guard,
    successor_argument
});
impl Wire for TerminalRankedGuard {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), RankedCustodyCodecError> {
        let Self::UnsignedParameterPositive {
            block,
            edge,
            condition,
            parameter,
        } = self;
        0u8.write(bytes)?;
        block.write(bytes)?;
        edge.write(bytes)?;
        condition.write(bytes)?;
        parameter.write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, RankedCustodyCodecError> {
        reader.tag(0)?;
        Ok(Self::UnsignedParameterPositive {
            block: Wire::read(reader)?,
            edge: Wire::read(reader)?,
            condition: Wire::read(reader)?,
            parameter: Wire::read(reader)?,
        })
    }
}
impl Wire for TerminalRankedSuccessorArgument {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), RankedCustodyCodecError> {
        let Self::UnsignedParameterMinusOne {
            argument_index,
            argument,
            source_parameter,
            target_parameter,
        } = self;
        0u8.write(bytes)?;
        argument_index.write(bytes)?;
        argument.write(bytes)?;
        source_parameter.write(bytes)?;
        target_parameter.write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, RankedCustodyCodecError> {
        reader.tag(0)?;
        Ok(Self::UnsignedParameterMinusOne {
            argument_index: Wire::read(reader)?,
            argument: Wire::read(reader)?,
            source_parameter: Wire::read(reader)?,
            target_parameter: Wire::read(reader)?,
        })
    }
}
