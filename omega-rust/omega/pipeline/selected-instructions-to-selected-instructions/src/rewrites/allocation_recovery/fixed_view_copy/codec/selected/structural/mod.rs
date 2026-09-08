//! Optimizer module role: stage group.
//! Ordinary function contracts, outgoing memory, and ownership data.
mod argument_source;
mod call;
mod calling;
mod declarations;
mod function;
mod projected_qualifications;
mod provider;
mod read_result;
mod settlements;
mod signature;
pub(super) use declarations::{decode_semantic_argument, encode_semantic_argument};
pub(super) use function::{decode_contracts, decode_local_slot, encode_contracts};
