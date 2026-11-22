//! Owned Psi product consumed independently of the source compiler.
#![forbid(unsafe_code)]
pub use omega_program_contracts::{CrashCause, DeclaredPauseBounds, Multiplicity, NumberedMember, PauseEffects, SchemaNumbering, Site, SourceSpan};
pub use omega_target_contract::{
    AddressSpaceIdentity, Endianness, EntryProfileIdentity, EntryStackGuarantee, FloatFormat, FloatOperation, FloatRealization, FloatSemantics, LayoutExtent,
    LayoutGeometry, LayoutQuery, TargetIdentity,
};
mod assembly;
pub use assembly::*;
mod body;
pub use body::*;
mod suspension;
pub use suspension::*;
mod blocks;
pub use blocks::*;
mod external_exit;
pub use external_exit::*;
mod machine_templates;
pub use machine_templates::*;
mod package;
pub use package::*;
mod data;
pub use data::*;
mod traits;
pub use traits::*;
mod type_expression;
pub use type_expression::*;
mod mathematics;
pub use mathematics::*;
mod policy_construction;
pub use policy_construction::*;
mod domains;
pub use domains::*;
mod contracts;
pub use contracts::*;
mod arithmetic;
pub use arithmetic::*;
mod access;
pub use access::*;
mod types;
pub use types::*;
mod identity;
pub use identity::*;
mod target;
pub use target::*;
mod ranking;
pub use ranking::*;
