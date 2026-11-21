//! L3 types one resolved Omega stage. It checks declarations across the admitted files,
//! selects calls and operators, and records the facts later layers must prove.
//!
//! The package CLI calls [`type_source_package`]. Preparation calls [`type_source_package_for_preparation`]
//! for completed trees or a pending value request. Both entries live in `toolchain_declarations/source_package.rs`.
//! For a computed width, follow `tests/expected.rs` and `tests/fixtures/computed_width/`.
//!
//! Start with `typing/type_stage.rs` for pass order. `declarations/` indexes the
//! names supplied by L2. `type_system/` interprets annotations and specializes types.
//! `expressions/` checks machine bodies. `contracts/` composes effects and evidence
//! across declarations. `evaluation/` forms static indices and checks closed results.
//! `toolchain_declarations/` admits the toolchain declarations used by the source command.
//!
//! `omega-typed` defines the result and its component types.
//! The language rules and their executable examples are in the crate's CONTRACT.md.
#![forbid(unsafe_code)]

mod contracts;
mod declarations;
mod evaluation;
mod expressions;
mod toolchain_declarations;
mod type_error;
mod type_relations;
mod type_system;
mod typing;

pub use omega_toolchain::{SourcePackage, resolve_source_package, resolve_source_package_with_assembly};
pub use omega_typed::*;
pub use toolchain_declarations::core_interface::standard_library;
pub use toolchain_declarations::source_package::{type_source_package, type_source_package_for_preparation};
pub use typing::entry_points::{type_stage, type_stage_for_target, type_stage_in_environment};
pub use typing::typing_environment::TypingEnvironment;
pub use typing::typing_progress::{PendingTyping, TypingProgress};
