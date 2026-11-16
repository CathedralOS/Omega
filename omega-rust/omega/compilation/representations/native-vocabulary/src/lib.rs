#![forbid(unsafe_code)]

/*
Leaf vocabulary the native program forms share.

Abstract operations, the optimization unit, target operations, the prepared
execution plan and selected instructions all carry the same small set of
value-level types: where a row came from and what it is charged, how a
structural store or copy names its destination, what backs an access
reference, and which storage a structural path selects. None of these is a
program: they hold identities, structural declarations and paths, never a
function body or a fact table.

They used to be defined inside the abstract-operations and optimization-unit
crates, so any form that carried one depended on those earlier program
containers. Defined here, below every form, a later stage can name them
without being able to read an earlier program.
*/

pub mod access_lifecycles;
pub mod access_reference_validation;
pub mod access_references;
pub mod address_joins;
pub mod calls;
pub mod completion_claims;
pub mod identity_encoding;
pub mod integer_arithmetic;
pub mod owner_loans;
pub mod ownership_frontier;
pub mod provenance;
pub mod storage_projection;
pub mod structural_stores;
pub mod structural_type_catalog;
pub mod values;
