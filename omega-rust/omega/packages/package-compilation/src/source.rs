//! The package source a compilation reads: `source_assembly` builds one source
//! forest with exact package, import and generated-source custody,
//! `source_imports` discovers imports in authored order, `source_snapshot`
//! binds neutral acquisition to Build's input metadata, `source_consumption`
//! records exactly which bytes were read, `anonymous_owner` owns an anonymous
//! invocation's source, `toolchain_library` locates the toolchain's own
//! library, and `canonical_fs` handles host-filesystem bytes and modes.

pub(crate) mod anonymous_owner;
pub(crate) mod canonical_fs;
pub mod source_assembly;
pub(crate) mod source_consumption;
pub mod source_imports;
pub(crate) mod source_snapshot;
pub mod toolchain_library;
