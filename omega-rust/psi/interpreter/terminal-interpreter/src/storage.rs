//! The storage the loop mutates. Primitive, scalar, structural, provider and
//! activation storage are the homes; `reference`, `record`, `scalar_array`,
//! `byte_sequences` and `element_views` are the carriers and views over them;
//! `runtime_rows` reserves live cells, `runtime_indexes` and
//! `storage_addresses` expose positions and addresses, and
//! `structural_identities` allocates execution-wide identities.

pub(crate) mod activation_storage;
pub(crate) mod byte_sequences;
pub(crate) mod element_views;
pub(crate) mod primitive_storage;
pub(crate) mod provider_storage;
pub(crate) mod record;
pub(crate) mod reference;
pub(crate) mod runtime_indexes;
pub(crate) mod runtime_rows;
pub(crate) mod scalar_array;
pub(crate) mod scalar_storage;
pub(crate) mod storage_addresses;
pub(crate) mod structural_identities;
pub(crate) mod structural_storage;
