//! Program identities, diagnostics and semantic bounds shared by compiler stages and their products.
#![forbid(unsafe_code)]

mod source;
pub use source::{Site, SourceSpan};
mod effects;
pub use effects::*;
mod storage;
pub use storage::*;

mod identity;
pub use identity::{DeclarationId, DeclarationName, ResolvedSignature};
mod schema;
pub use schema::{NumberedMember, SchemaNumbering};
mod collection;
pub use collection::CollectionPrimitive;
