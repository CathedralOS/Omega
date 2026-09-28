//! Scalar block invariants on the wire: the header block, predicate and
//! arrivals of one invariant.

use super::super::CodecError;
use super::super::proposition_wire::{decode_proposition, encode_proposition};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;

pub(super) fn encode_scalar_block_invariant(
    writer: &mut Writer,
    invariant: &terminal_psi::ScalarBlockInvariant,
) -> Result<(), CodecError> {
    writer.id(invariant.machine);
    writer.id(invariant.header);
    encode_proposition(writer, &invariant.predicate, 0)?;
    writer.len("scalar block invariant arrivals", invariant.arrivals.len())?;
    for arrival in &invariant.arrivals {
        writer.id(arrival.edge);
        writer.id(arrival.obligation);
    }
    Ok(())
}

pub(super) fn decode_scalar_block_invariant(
    reader: &mut Reader<'_>,
) -> Result<terminal_psi::ScalarBlockInvariant, CodecError> {
    Ok(terminal_psi::ScalarBlockInvariant {
        machine: reader.id("MachineId")?,
        header: reader.id("BlockId")?,
        predicate: decode_proposition(reader, 0)?,
        arrivals: decode_counted(reader, |reader| {
            Ok(terminal_psi::ScalarBlockInvariantArrival {
                edge: reader.id("EdgeId")?,
                obligation: reader.id("ObligationId")?,
            })
        })?,
    })
}
