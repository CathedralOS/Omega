//! Optimizer module role: stage group.
//! Ordinary function contracts, outgoing memory, and ownership data.
mod call;
mod calling;
mod declarations;
mod function;
mod projected_qualifications;
mod provider;
mod settlements;
mod signature;
pub(super) use function::{decode_contracts, encode_contracts};
