//! Optimizer module role: stage group.
//! Ordinary function contracts, outgoing memory, and ownership data.
mod argument_source;
mod call;
mod calling;
mod declarations;
mod function;
mod normalized_foreign;
mod placed_bindings;
mod projected_qualifications;
mod provider;
mod read_result;
mod settlements;
mod signature;
pub(super) use call::{decode_runtime_indices, encode_runtime_indices};
pub(super) use declarations::{decode_semantic_argument, encode_semantic_argument};
pub(super) use function::{
    decode_contracts, decode_invocation_site, decode_local_slot, encode_contracts,
};
pub(super) use settlements::decode_cleanup;
