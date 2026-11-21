//! Callable declarations shared by native program identities and the complete Psi artifact.
#![forbid(unsafe_code)]

pub mod catalogs;
pub mod certificates;
pub mod contracts;
pub mod evidence;
pub mod machine_declarations;
pub mod proof_declarations;
pub mod reach;
pub mod results;
pub mod signatures;

pub use catalogs::{decode_callable_catalog, encode_callable_catalog};
pub use psi_declaration_encoding::{CodecError, fields, wire};
