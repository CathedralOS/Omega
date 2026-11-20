//! Structural types, domains, qualifications and quotients.

mod declarations;
mod opaque_storage;
mod queries;
pub mod quotients;
mod structural;
pub mod structural_path_encoding;
pub mod values;

pub use declarations::TypeDeclarations;
pub use opaque_storage::OpaqueStorageShapes;
pub use structural::*;
