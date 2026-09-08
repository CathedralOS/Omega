//! Optimizer module role: stage group.
//! Ordinary function contracts, outgoing memory, and ownership data.
mod argument_source;
mod call;
mod calling;
mod declarations;
mod function;
mod projected_qualifications;
mod provider;
mod settlements;
mod signature;
pub(super) use function::{decode_contracts, decode_local_slot, encode_contracts};
