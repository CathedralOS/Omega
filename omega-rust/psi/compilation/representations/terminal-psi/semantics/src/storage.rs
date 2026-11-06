//! Storage: activation, arrival, borrowed and calling storage, storage
//! dependencies and regions, collection backing, representation regions,
//! element correspondence and zero-valid shapes.

pub mod activation_storage;
pub(crate) mod arrival_storage;
pub mod borrowed_storage;
pub(crate) mod calling_storage;
pub(crate) mod collection_backing;
pub(crate) mod element_representation;
pub(crate) mod representation_regions;
pub mod run_custody;
pub(crate) mod storage_dependencies;
pub(crate) mod storage_regions;
pub(crate) mod structural_locals;
pub(crate) mod zero_valid_storage;
